use crossbeam_queue::ArrayQueue;
use dashmap::DashMap;
use ssh2::Session;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::io::Read;
use std::io::{Error, ErrorKind};
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpStream;
use tokio::sync::{OwnedSemaphorePermit, Semaphore};
use tokio::task;

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
}

struct PoolInner {
    connect_options: Arc<ConnectOptions>,
    idle_conns: ArrayQueue<Idle>,
    semaphore: Arc<Semaphore>,
    size: std::sync::atomic::AtomicU32,
    num_idle: std::sync::atomic::AtomicUsize,
    options: PoolOptions,
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

    async fn acquire(&self) -> Result<Floating<Live>, Error> {
        let permit = tokio::time::timeout(
            self.options.acquire_timeout,
            self.semaphore.clone().acquire_owned(),
        )
        .await
        .map_err(|_| Error::new(ErrorKind::TimedOut, "Pool acquire timed out"))?
        .map_err(|_| Error::new(ErrorKind::Other, "Pool closed"))?;

        if let Some(idle) = self.idle_conns.pop() {
            self.num_idle
                .fetch_sub(1, std::sync::atomic::Ordering::AcqRel);
            let conn = Floating::from_idle(idle, Arc::new(self.clone()), permit);
            if let Some(timeout) = self.options.idle_timeout {
                if conn.inner.idle_since.elapsed() > timeout {
                    let guard = conn.close().await;
                    return self.connect(guard).await;
                }
            }
            Ok(conn.into_live())
        } else {
            let guard = self.try_increment_size(permit).map_err(|_| {
                Error::new(ErrorKind::Other, "Cannot increment size")
            })?;
            self.connect(guard).await
        }
    }

    fn try_increment_size(
        &self,
        permit: OwnedSemaphorePermit,
    ) -> Result<DecrementSizeGuard, OwnedSemaphorePermit> {
        let result = self.size.fetch_update(
            std::sync::atomic::Ordering::AcqRel,
            std::sync::atomic::Ordering::Acquire,
            |size| size.checked_add(1).filter(|&s| s <= self.options.max_connections),
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

    fn release(&self, floating: Floating<Live>) {
        let Floating { inner: idle, guard } = floating.into_idle();
        if self.idle_conns.push(idle).is_err() {
            // drop if overflow
            return;
        }
        guard.release_permit();
        self.num_idle
            .fetch_add(1, std::sync::atomic::Ordering::AcqRel);
    }
}

#[derive(Clone)]
struct Live {
    raw: Session,
    created_at: std::time::Instant,
}

#[derive(Clone)]
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

#[derive(Clone)]
pub struct Pool(Arc<PoolInner>);

impl Pool {
    pub async fn acquire(&self) -> Result<PoolConnection, Error> {
        self.0.acquire().await.map(|conn| conn.reattach())
    }

    pub fn new(options: PoolOptions, conn_options: ConnectOptions) -> Self {
        Self(Arc::new(PoolInner::new(options, conn_options)))
    }
}

#[derive(Clone, Eq, PartialEq, Hash)]
struct SessionKey {
    host: String,
    port: u16,
    username: String,
}

impl SessionKey {
    fn new(host: &str, port: u16, username: &str) -> Self {
        Self {
            host: host.to_string(),
            port,
            username: username.to_string(),
        }
    }
}

pub struct SessionPool {
    pools: DashMap<u64, Pool>,
    options: PoolOptions,
}

impl SessionPool {
    pub fn new(options: PoolOptions) -> Self {
        Self {
            pools: DashMap::new(),
            options,
        }
    }

    fn make_key(&self, host: &str, port: u16, username: &str) -> u64 {
        let key = SessionKey::new(host, port, username);
        let mut hasher = DefaultHasher::new();
        key.hash(&mut hasher);
        hasher.finish()
    }

    async fn get_or_create_pool(
        &self,
        host: &str,
        port: u16,
        username: &str,
        password: &str,
    ) -> Pool {
        let key = self.make_key(host, port, username);
        if let Some(p) = self.pools.get(&key) {
            return p.clone();
        }
        let conn_opt = ConnectOptions::new(host, port, username, password);
        let pool = Pool::new(self.options.clone(), conn_opt);
        self.pools.insert(key, pool.clone());
        pool
    }

    // execute command on specific host/user/port
    pub async fn execute_command(
        &self,
        host: &str,
        port: u16,
        username: &str,
        password: &str,
        command: &str,
    ) -> Result<String, Error> {
        let pool = self.get_or_create_pool(host, port, username, password).await;
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
            Ok(output)
        })
        .await
        .map_err(|e| Error::new(ErrorKind::Other, e))??;

        Ok(output)
    }
}
