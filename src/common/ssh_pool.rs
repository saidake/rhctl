use dashmap::DashMap;
use ssh2::Session;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::io::{Error, ErrorKind, Read};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};
use tokio::net::TcpStream;
use tokio::sync::{OwnedSemaphorePermit, Semaphore};
use tokio::task;

use crate::domain::cmd_params::ServerMetadata;

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
            sess.handshake()
                .map_err(|e| Error::new(ErrorKind::Other, e))?;
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
    pub max_connections: u32, // max sessions per server
    pub min_connections: u32,
    pub acquire_timeout: Duration,
    pub idle_timeout: Option<Duration>, // session idle timeout
    pub max_channel_per_session: u32,   // max concurrent channel per session
}

impl PoolOptions {
    pub fn new() -> Self {
        Self {
            max_connections: 10,
            min_connections: 0,
            acquire_timeout: Duration::from_secs(30),
            idle_timeout: Some(Duration::from_secs(600)), // 10min default
            max_channel_per_session: 5,
        }
    }
}

#[derive(Clone)]
struct Live {
    raw: Session,
    created_at: Instant,
}

#[derive(Clone)]
struct Idle {
    live: Live,
    idle_since: Instant,
}

// Wrap session to manage channel count and idle timestamp
struct SessionWrapper {
    live: Live,
    semaphore: Arc<Semaphore>,  // control channel concurrency
    idle_since: Mutex<Instant>, // track idle time
}

impl SessionWrapper {
    fn new(live: Live, max_channel: u32) -> Self {
        Self {
            live,
            semaphore: Arc::new(Semaphore::new(max_channel as usize)),
            idle_since: Mutex::new(Instant::now()),
        }
    }

    async fn acquire_channel(&self) -> OwnedSemaphorePermit {
        let permit = self.semaphore.clone().acquire_owned().await.unwrap();
        // update idle timestamp
        let mut idle = self.idle_since.lock().unwrap();
        *idle = Instant::now();
        permit
    }

    fn is_idle_timeout(&self, timeout: Duration) -> bool {
        let idle = self.idle_since.lock().unwrap();
        idle.elapsed() > timeout
    }
}

// Manage multiple sessions for one host/user/port
struct SessionPool {
    connect_options: ConnectOptions,
    sessions: Mutex<Vec<Arc<SessionWrapper>>>,
    options: PoolOptions,
    semaphore: Arc<Semaphore>, // total session limit
}

impl SessionPool {
    fn new(connect_options: ConnectOptions, options: PoolOptions) -> Self {
        Self {
            connect_options,
            sessions: Mutex::new(vec![]),
            options: options.clone(),
            semaphore: Arc::new(Semaphore::new(options.max_connections as usize)),
        }
    }

    async fn get_session(&self) -> Result<Arc<SessionWrapper>, Error> {
        // try reuse idle session with free channel
        let mut sessions_guard = self.sessions.lock().unwrap();
        for s in sessions_guard.iter() {
            if s.semaphore.available_permits() > 0 {
                return Ok(s.clone());
            }
        }

        // no available session, try create new session if limit allows
        let permit = tokio::time::timeout(
            self.options.acquire_timeout,
            self.semaphore.clone().acquire_owned(),
        )
        .await
        .map_err(|_| Error::new(ErrorKind::TimedOut, "Session acquire timed out"))?
        .map_err(|_| Error::new(ErrorKind::Other, "Server pool closed"))?;

        let conn_opts = self.connect_options.clone();
        let sess = conn_opts.connect().await?;
        let live = Live {
            raw: sess,
            created_at: Instant::now(),
        };
        let wrapper = Arc::new(SessionWrapper::new(
            live,
            self.options.max_channel_per_session,
        ));
        sessions_guard.push(wrapper.clone());
        drop(permit); // session already counted by semaphore
        Ok(wrapper)
    }

    fn cleanup_idle(&self) {
        if let Some(timeout) = self.options.idle_timeout {
            let mut sessions_guard = self.sessions.lock().unwrap();
            sessions_guard.retain(|s| !s.is_idle_timeout(timeout));
        }
    }
}

pub struct ServerPool {
    servers: DashMap<u64, Arc<SessionPool>>,
    options: PoolOptions,
}

impl ServerPool {
    pub fn new(options: PoolOptions) -> Self {
        Self {
            servers: DashMap::new(),
            options,
        }
    }

    fn cleanup_idle_sessions(&self) {
        // let timeout = self.options.idle_timeout.unwrap_or(Duration::from_secs(600));
        for pool in self.servers.iter() {
            pool.value().cleanup_idle();
        }
    }

    pub fn generate_server_key(host: &str, port: u16, username: &str) -> u64 {
        let mut hasher = DefaultHasher::new();
        host.hash(&mut hasher);
        port.hash(&mut hasher);
        username.hash(&mut hasher);
        hasher.finish()
    }

    async fn get_server_pool<T: ServerMetadata>(&self, server_key: T) -> Arc<SessionPool> {
        if let Some(pool) = self.servers.get(&server_key.get_server_key()) {
            return pool.clone();
        }

        let connect_opts = ConnectOptions::new(
            server_key.get_host(),
            server_key.get_ssh_port(),
            server_key.get_user(),
            server_key.get_password(),
        );
        let server_pool = Arc::new(SessionPool::new(connect_opts, self.options.clone()));
        self.servers
            .insert(server_key.get_server_key(), server_pool.clone());
        server_pool
    }

    // Behavior functions
    // pub async fn execute_command(
    //     &self,
    //     host: &str,
    //     port: u16,
    //     username: &str,
    //     password: &str,
    //     command: &str,
    // ) -> Result<String, Error> {
    //     let server_pool = self.get_server_pool(host, port, username, password).await;
    //     let session_wrapper = server_pool.get_session().await?;
    //     let permit = session_wrapper.acquire_channel().await;

    //     let session_wrapper = session_wrapper.clone(); // Arc<SessionWrapper>
    //     let cmd = command.to_string();

    //     // run command in blocking thread
    //     let output = task::spawn_blocking(move || -> Result<String, Error> {
    //         let mut channel = session_wrapper
    //             .live
    //             .raw
    //             .channel_session()
    //             .map_err(|e| Error::new(ErrorKind::Other, e))?;
    //         channel
    //             .exec(&cmd)
    //             .map_err(|e| Error::new(ErrorKind::Other, e))?;
    //         let mut out = String::new();
    //         channel
    //             .read_to_string(&mut out)
    //             .map_err(|e| Error::new(ErrorKind::Other, e))?;
    //         channel
    //             .wait_close()
    //             .map_err(|e| Error::new(ErrorKind::Other, e))?;
    //         Ok(out)
    //     })
    //     .await
    //     .map_err(|e| Error::new(ErrorKind::Other, e))??;

    //     drop(permit); // release channel slot
    //     Ok(output)
    // }

    // Start background idle cleanup thread
    pub fn start_idle_cleanup(self: Arc<Self>, interval: Duration) {
        thread::spawn(move || {
            loop {
                thread::sleep(interval);
                self.cleanup_idle_sessions();
            }
        });
    }
}
