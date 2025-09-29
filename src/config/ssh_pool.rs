use crossbeam_queue::ArrayQueue;
use ssh2::Session;
use std::io::Read;
use std::io::{Error, ErrorKind};
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpStream;
use tokio::sync::{OwnedSemaphorePermit, Semaphore};
use tokio::task;

// Connection options for SSH
#[derive(Clone, Debug)]
pub struct ConnectOptions {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
    pub connect_timeout: Duration,
}

impl ConnectOptions {
    pub fn new(host: &str, port: u16, username: &str, password: &str) -> Self {
        Self {
            host: host.to_string(),
            port,
            username: username.to_string(),
            password: password.to_string(),
            connect_timeout: Duration::from_secs(10),
        }
    }

    async fn connect(&self) -> Result<Session, Error> {
        let addr = format!("{}:{}", self.host, self.port);
        let stream = tokio::time::timeout(self.connect_timeout, TcpStream::connect(&addr))
            .await
            .map_err(|_| Error::new(ErrorKind::TimedOut, "Connection timed out"))?
            .map_err(|e| Error::new(ErrorKind::Other, e))?;

        // Clone username and password to avoid borrowing self in the closure
        let username = self.username.clone();
        let password = self.password.clone();

        let sess = task::spawn_blocking(move || {
            let mut sess = Session::new().map_err(|e| Error::new(ErrorKind::Other, e))?;
            sess.set_tcp_stream(stream);
            sess.handshake().map_err(|e| Error::new(ErrorKind::Other, e))?;
            sess.userauth_password(&username, &password)
                .map_err(|e| Error::new(ErrorKind::Other, e))?;
            Ok::<Session, Error>(sess)
        })
        .await
        .map_err(|e| Error::new(ErrorKind::Other, e))??;

        Ok(sess)
    }
}

// Pool options for configuring the pool
#[derive(Clone, Debug)]
pub struct PoolOptions {
    pub max_connections: u32,
    pub min_connections: u32,
    pub acquire_timeout: Duration,
    pub idle_timeout: Option<Duration>,
}

impl PoolOptions {
    pub fn new() -> Self {
        Self {
            max_connections: 10,
            min_connections: 0,
            acquire_timeout: Duration::from_secs(30),
            idle_timeout: Some(Duration::from_secs(600)),
        }
    }

    pub fn max_connections(mut self, max: u32) -> Self {
        self.max_connections = max;
        self
    }

    pub fn min_connections(mut self, min: u32) -> Self {
        self.min_connections = min;
        self
    }

    pub fn acquire_timeout(mut self, timeout: Duration) -> Self {
        self.acquire_timeout = timeout;
        self
    }

    pub fn idle_timeout(mut self, timeout: Option<Duration>) -> Self {
        self.idle_timeout = timeout;
        self
    }
}

// Internal pool state
struct PoolInner {
    connect_options: Arc<ConnectOptions>,
    idle_conns: ArrayQueue<Idle>,
    semaphore: Arc<Semaphore>,
    size: std::sync::atomic::AtomicU32,
    num_idle: std::sync::atomic::AtomicUsize,
    options: PoolOptions, // Added to store PoolOptions
}

impl PoolInner {
    fn new(options: PoolOptions, connect_options: ConnectOptions) -> Self {
        let capacity = options.max_connections as usize;
        Self {
            connect_options: Arc::new(connect_options),
            idle_conns: ArrayQueue::new(capacity),
            semaphore: Arc::new(Semaphore::new(capacity)),
            size: std::sync::atomic::AtomicU32::new(0),
            num_idle: std::sync::atomic::AtomicUsize::new(0),
            options,
        }
    }
    fn clone(&self) -> Self {
        Self {
            connect_options: Arc::clone(&self.connect_options),
            idle_conns: ArrayQueue::new(self.idle_conns.capacity()), 
            semaphore: Arc::clone(&self.semaphore),
            size: std::sync::atomic::AtomicU32::new(self.size()),
            num_idle: std::sync::atomic::AtomicUsize::new(self.num_idle()),
            options: self.options.clone(),
        }
    }
    fn size(&self) -> u32 {
        self.size.load(std::sync::atomic::Ordering::Acquire)
    }

    fn num_idle(&self) -> usize {
        self.num_idle.load(std::sync::atomic::Ordering::Acquire)
    }

    fn try_acquire(&self) -> Option<Floating<Idle>> {
        let permit = self.semaphore.clone().try_acquire_owned().ok()?; // Clone Arc<Semaphore>
        self.pop_idle(permit).ok()
    }

    fn pop_idle(
        &self,
        permit: OwnedSemaphorePermit,
    ) -> Result<Floating<Idle>, OwnedSemaphorePermit> {
        if let Some(idle) = self.idle_conns.pop() {
            self.num_idle
                .fetch_sub(1, std::sync::atomic::Ordering::AcqRel);
            Ok(Floating::from_idle(idle, Arc::new(self.clone()), permit))
        } else {
            Err(permit)
        }
    }

    fn release(&self, floating: Floating<Live>) {
        let Floating { inner: idle, guard } = floating.into_idle();
        if self.idle_conns.push(idle).is_err() {
            panic!("Idle queue overflow");
        }
        guard.release_permit();
        self.num_idle
            .fetch_add(1, std::sync::atomic::Ordering::AcqRel);
    }

    async fn acquire(&self) -> Result<Floating<Live>, Error> {
        let permit = tokio::time::timeout(self.options.acquire_timeout, self.semaphore.clone().acquire_owned())
            .await
            .map_err(|_| Error::new(ErrorKind::TimedOut, "Pool acquire timed out"))?
            .map_err(|_| Error::new(ErrorKind::Other, "Pool closed"))?;

        match self.pop_idle(permit) {
            Ok(conn) => {
                if let Some(timeout) = self.options.idle_timeout {
                    if conn.inner.idle_since.elapsed() > timeout {
                        let guard = conn.close().await;
                        return self.connect(guard).await;
                    }
                }
                Ok(conn.into_live())
            }
            Err(permit) => {
                let guard = self.try_increment_size(permit).map_err(|_| {
                    Error::new(ErrorKind::Other, "Cannot increment size")
                })?;
                self.connect(guard).await
            }
        }
    }

    fn try_increment_size(
        &self,
        permit: OwnedSemaphorePermit,
    ) -> Result<DecrementSizeGuard, OwnedSemaphorePermit> {
        let result = self.size.fetch_update(
            std::sync::atomic::Ordering::AcqRel,
            std::sync::atomic::Ordering::Acquire,
            |size| {
                size.checked_add(1)
                    .filter(|&s| s <= self.options.max_connections)
            },
        );
        match result {
            Ok(_) => Ok(DecrementSizeGuard::from_permit(
                Arc::new(self.clone()),
                permit,
            )),
            Err(_) => Err(permit),
        }
    }

    async fn connect(&self, guard: DecrementSizeGuard) -> Result<Floating<Live>, Error> {
        let sess = self.connect_options.connect().await?;
        Ok(Floating::new_live(sess, guard))
    }

    async fn try_min_connections(&self) -> Result<(), Error> {
        while self.size() < self.options.min_connections {
            let permit = self
                .semaphore
                .clone()
                .try_acquire_owned()
                .map_err(|_| Error::new(ErrorKind::Other, "No permit"))?;
            let guard = self
                .try_increment_size(permit)
                .map_err(|_| Error::new(ErrorKind::Other, "Cannot increment"))?;
            let conn = self.connect(guard).await?;
            self.release(conn);
        }
        Ok(())
    }
}

// Connection structs
struct Live {
    raw: Session,
    created_at: std::time::Instant,
}

struct Idle {
    live: Live,
    idle_since: std::time::Instant,
}

struct Floating<C> {
    inner: C,
    guard: DecrementSizeGuard,
}

struct DecrementSizeGuard {
    pool: Arc<PoolInner>,
    cancelled: bool,
}

impl DecrementSizeGuard {
    fn new_permit(pool: Arc<PoolInner>) -> Self {
        Self {
            pool,
            cancelled: false,
        }
    }

    fn from_permit(pool: Arc<PoolInner>, permit: OwnedSemaphorePermit) -> Self {
        drop(permit);
        Self::new_permit(pool)
    }

    fn release_permit(mut self) {
        self.pool.semaphore.add_permits(1);
        self.cancel();
    }

    fn cancel(&mut self) {
        self.cancelled = true;
    }
}

impl Drop for DecrementSizeGuard {
    fn drop(&mut self) {
        if !self.cancelled {
            self.pool
                .size
                .fetch_sub(1, std::sync::atomic::Ordering::AcqRel);
            self.pool.semaphore.add_permits(1);
        }
    }
}

impl Live {
    fn float(self, pool: Arc<PoolInner>) -> Floating<Self> {
        Floating {
            inner: self,
            guard: DecrementSizeGuard::new_permit(pool),
        }
    }

    fn into_idle(self) -> Idle {
        Idle {
            live: self,
            idle_since: std::time::Instant::now(),
        }
    }
}

impl Floating<Live> {
    fn new_live(sess: Session, guard: DecrementSizeGuard) -> Self {
        Self {
            inner: Live {
                raw: sess,
                created_at: std::time::Instant::now(),
            },
            guard,
        }
    }

    fn reattach(self) -> PoolConnection {
        let Floating { inner, mut guard } = self;
        let pool = Arc::clone(&guard.pool);
        guard.cancel();
        PoolConnection {
            live: Some(inner),
            pool,
        }
    }

    fn into_idle(self) -> Floating<Idle> {
        Floating {
            inner: self.inner.into_idle(),
            guard: self.guard,
        }
    }
}

impl Floating<Idle> {
    fn from_idle(idle: Idle, pool: Arc<PoolInner>, permit: OwnedSemaphorePermit) -> Self {
        Self {
            inner: idle,
            guard: DecrementSizeGuard::from_permit(pool, permit),
        }
    }

    async fn close(self) -> DecrementSizeGuard {
        let raw = self.inner.live.raw;
        task::spawn_blocking(move || raw.disconnect(None, "", None))
            .await
            .ok();
        self.guard
    }

    fn into_live(self) -> Floating<Live> {
        Floating {
            inner: self.inner.live,
            guard: self.guard,
        }
    }
}

// Public pool connection
pub struct PoolConnection {
    live: Option<Live>,
    pool: Arc<PoolInner>,
}

impl std::ops::Deref for PoolConnection {
    type Target = Session;
    fn deref(&self) -> &Self::Target {
        &self.live.as_ref().expect("BUG: session taken").raw
    }
}

impl std::ops::DerefMut for PoolConnection {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.live.as_mut().expect("BUG: session taken").raw
    }
}

impl Drop for PoolConnection {
    fn drop(&mut self) {
        if let Some(live) = self.live.take() {
            let floating = live.float(self.pool.clone());
            self.pool.release(floating);
        }
    }
}

// Public pool struct
#[derive(Clone)]
pub struct Pool(Arc<PoolInner>);

impl Pool {
    pub async fn connect_with(
        options: PoolOptions,
        conn_options: ConnectOptions,
    ) -> Result<Self, Error> {
        let pool = Self(Arc::new(PoolInner::new(options, conn_options)));
        if pool.0.options.min_connections > 0 {
            pool.0.try_min_connections().await?;
        }
        Ok(pool)
    }

    pub fn connect_lazy_with(options: PoolOptions, conn_options: ConnectOptions) -> Self {
        Self(Arc::new(PoolInner::new(options, conn_options)))
    }

    pub async fn acquire(&self) -> Result<PoolConnection, Error> {
        self.0.acquire().await.map(|conn| conn.reattach())
    }
}

// Execute a command using the pool
pub async fn execute_command(pool: &Pool, command: &str) -> Result<String, Error> {
    let mut conn = pool.acquire().await?;
    let cmd = command.to_string();
    let output = task::spawn_blocking(move || {
        let mut channel = conn
            .channel_session()
            .map_err(|e| Error::new(ErrorKind::Other, e))?;
        channel
            .exec(&cmd)
            .map_err(|e| Error::new(ErrorKind::Other, e))?;
        let mut output = String::new();
        channel
            .read_to_string(&mut output)
            .map_err(|e| Error::new(ErrorKind::Other, e))?;
        channel
            .wait_close()
            .map_err(|e| Error::new(ErrorKind::Other, e))?;
        if channel
            .exit_status()
            .map_err(|e| Error::new(ErrorKind::Other, e))?
            != 0
        {
            return Err(Error::new(
                ErrorKind::Other,
                format!("Command `{}` failed", cmd),
            ));
        }
        Ok(output)
    })
    .await
    .map_err(|e| Error::new(ErrorKind::Other, e))??;
    Ok(output)
}
