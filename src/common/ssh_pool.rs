use dashmap::DashMap;
use ssh2::Channel;
use ssh2::Session;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::io;
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

/// Wrapper for a channel that automatically releases resources when dropped
pub struct ChannelGuard {
    pub channel: Channel,
    _permit: OwnedSemaphorePermit,    // RAII for channel slot
}

impl<'a> Drop for ChannelGuard {
    fn drop(&mut self) {
        // Try closing channel gracefully
        let _ = self.channel.send_eof();
        let _ = self.channel.wait_eof();
        let _ = self.channel.close();
        let _ = self.channel.wait_close();
        // OwnedSemaphorePermit is automatically released
    }
}

#[derive(Clone)]
pub struct LiveSessionWrapper {
    live: Live,
    semaphore: Arc<Semaphore>,
    max_permits: usize,
}

impl LiveSessionWrapper {
    pub fn new(live: Live, max_channel: u32) -> Self {
        Self {
            live,
            semaphore: Arc::new(Semaphore::new(max_channel as usize)),
            max_permits: max_channel as usize,
        }
    }

    /// Synchronously acquire a channel, returns ChannelGuard
    pub async fn get_channel_guard(&self) -> io::Result<ChannelGuard> {
        let permit = self.semaphore.clone().acquire_owned().await.map_err(|_| {
            io::Error::new(io::ErrorKind::Other, "Failed to acquire channel permit")
        })?;

        // Create ssh channel in blocking thread
        let channel = tokio::task::spawn_blocking({
            let session = self.live.raw.clone();
            move || {
                session.channel_session().map_err(|e| {
                    io::Error::new(io::ErrorKind::Other, format!("Channel error: {}", e))
                })
            }
        })
        .await
        .map_err(|e| io::Error::new(io::ErrorKind::Other, format!("Join error: {}", e)))??;

        Ok(ChannelGuard {
            channel,
            _permit: permit,
        })
    }

    pub fn is_fully_idle(&self) -> bool {
        self.semaphore.available_permits() == self.max_permits
    }
}

// Manage multiple sessions for one host/user/port
struct SessionPool {
    connect_options: ConnectOptions,
    active_sessions: Mutex<Vec<Arc<LiveSessionWrapper>>>,
    idle_sessions: Mutex<Vec<Idle>>,
    options: PoolOptions,
    semaphore: Arc<Semaphore>, // total session limit
}

impl SessionPool {
    fn new(connect_options: ConnectOptions, options: PoolOptions) -> Self {
        Self {
            connect_options,
            active_sessions: Mutex::new(vec![]),
            idle_sessions: Mutex::new(vec![]),
            options: options.clone(),
            semaphore: Arc::new(Semaphore::new(options.max_connections as usize)),
        }
    }

    pub async fn get_session(&self) -> Result<Arc<LiveSessionWrapper>, Error> {
        // First, try reuse idle sessions
        {
            let mut idle_guard = self.idle_sessions.lock().unwrap();
            if let Some(idle) = idle_guard.pop() {
                let wrapper = Arc::new(LiveSessionWrapper::new(
                    idle.live,
                    self.options.max_channel_per_session,
                ));
                let mut active_guard = self.active_sessions.lock().unwrap();
                active_guard.push(wrapper.clone());
                return Ok(wrapper);
            }
        }

        // Then, try reuse active sessions with free channel
        {
            let active_guard = self.active_sessions.lock().unwrap();
            for s in active_guard.iter() {
                if s.semaphore.available_permits() > 0 {
                    return Ok(s.clone());
                }
            }
        }

        // No available session, create new if under limit
        let _permit = tokio::time::timeout(
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
        let wrapper = Arc::new(LiveSessionWrapper::new(
            live,
            self.options.max_channel_per_session,
        ));
        self.active_sessions.lock().unwrap().push(wrapper.clone());
        Ok(wrapper)
    }

    fn recycle_session(&self) {
        let mut active_guard = self.active_sessions.lock().unwrap();
        let mut idle_guard = self.idle_sessions.lock().unwrap();

        active_guard.retain(|s| {
            if s.is_fully_idle() {
                idle_guard.push(Idle {
                    live: s.live.clone(),
                    idle_since: Instant::now(),
                });
                false // remove from active
            } else {
                true // keep in active
            }
        });
    }

    fn cleanup_idle(&self) {
        if let Some(timeout) = self.options.idle_timeout {
            let mut idle_guard = self.idle_sessions.lock().unwrap();
            idle_guard.retain(|s| s.idle_since.elapsed() <= timeout);
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
        for pool in self.servers.iter() {
            pool.value().recycle_session();
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

    pub async fn get_session_pool<T: ServerMetadata>(
        &self,
        server_metadata: &Arc<T>,
    ) -> Arc<SessionPool> {
        if let Some(pool) = self.servers.get(&server_metadata.get_server_key()) {
            return pool.clone();
        }

        let connect_opts = ConnectOptions::new(
            server_metadata.get_host(),
            server_metadata.get_ssh_port(),
            server_metadata.get_user(),
            server_metadata.get_password(),
        );
        let server_pool = Arc::new(SessionPool::new(connect_opts, self.options.clone()));
        self.servers
            .insert(server_metadata.get_server_key(), server_pool.clone());
        server_pool
    }

    // // Behavior functions
    // pub async fn use_channel<T, F>(
    //     &self,
    //     server_metadata: &Arc<T>,
    //     channel_fn: F, // <-- closure parameter
    // ) -> Result<String, String>
    // where
    //     T: ServerMetadata,
    //     F: FnOnce(&mut ssh2::Channel) -> Result<String, String> + Send + 'static,
    // {
    //     let server_pool = self.get_session_pool(server_metadata).await;
    //     let live_session_wrapper = server_pool
    //         .get_session()
    //         .await
    //         .map_err(|e| format!("Get session join error:\n\t{}", e))?;

    //     let permit = live_session_wrapper.acquire_channel().await;
    //     let live_session_wrapper = live_session_wrapper.clone(); // Arc<LiveSessionWrapper>
    //     // run command in blocking thread
    //     let output = task::spawn_blocking(move || -> Result<String, String> {
    //         let mut channel = live_session_wrapper
    //             .live
    //             .raw
    //             .channel_session()
    //             .map_err(|e| format!("Channel error:\n\t{}", e))?;
    //         // call the injected function
    //         (channel_fn)(&mut channel)
    //     })
    //     .await
    //     .map_err(|e| format!("Join error:\n\t{}", e))?
    //     .map_err(|e| format!("Channel operation error:\n\t{}", e))?;

    //     drop(permit); // release channel slot
    //     Ok(output)
    // }

    // Behavior functions
    pub async fn get_channel<T>(&self, server_metadata: &Arc<T>) -> Result<ChannelGuard, String>
    where
        T: ServerMetadata,
    {
        // get the session pool
        let server_pool = self.get_session_pool(server_metadata).await;
        let server_pool = Arc::clone(&server_pool); // clone Arc if needed

        // get a live session
        let live_session_wrapper = server_pool
            .get_session()
            .await
            .map_err(|e| format!("Get session error:\n\t{}", e))?;

        // get channel guard
        let guard = live_session_wrapper
            .get_channel_guard()
            .await
            .map_err(|e| format!("Get channel error:\n\t{}", e))?;

        Ok(guard)
    }

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
