/*
 * Copyright 2025 the original author or authors.
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *      https://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 * **************************************************************************
 * An SSH connection pool for managing sessions and channels across
 * multiple servers.
 *
 * Author: Craig Brown
 * Date: October 16, 2025
 * Since: 1.0.0
 */
use crate::domain::cmd_params::ServerMetadata;
use crate::domain::constants::{
    DEFAULT_SSH_HANDSHAKE_TIMEOUT, DEFAULT_SSH_PORT, REMOTE_TEMP_SBXCTL_FOLDER, SYSTEM_TASK_NAME,
};
use crate::domain::yml_config::ServerConfig;
use crate::utils::file_utils::{generate_remote_temp_dir, get_local_path_base_name};
use crate::utils::log_utils::ask_user;
use crate::utils::ssh_utils::execution_print;
use crate::{log_debug, log_error_direct, log_info, log_warn, log_warn_direct};
use async_recursion::async_recursion;
use async_trait::async_trait;
use dashmap::DashMap;
use dashmap::DashSet;
use dirs_next as dirs;
use futures::stream::{FuturesUnordered, StreamExt};
use russh::ChannelMsg;
use russh::client::{Config, Handler};
use russh::client::{Handle, Msg};
use russh_keys::PublicKeyBase64;
use russh_keys::key::PublicKey;
use std::collections::HashSet;
use std::collections::hash_map::DefaultHasher;
use std::fs::File;
use std::hash::{Hash, Hasher};
use std::io::{self, BufReader, Cursor, Read, Write};
use std::path::Path;
use std::process::exit;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;
use tokio::sync::oneshot;
use tokio::sync::{OwnedSemaphorePermit, Semaphore};
use tokio::task::{self};
use tokio::time::timeout;

#[derive(Debug)]
pub enum SshError {
    Russh(russh::Error),
    Io(io::Error),
    Custom(String),
}

impl From<russh::Error> for SshError {
    fn from(err: russh::Error) -> Self {
        SshError::Russh(err)
    }
}

impl From<io::Error> for SshError {
    fn from(err: io::Error) -> Self {
        SshError::Io(err)
    }
}

impl std::fmt::Display for SshError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            SshError::Russh(e) => write!(f, "SSH error: {}", e),
            SshError::Io(e) => write!(f, "IO error: {}", e),
            SshError::Custom(e) => write!(f, "Custom error: {}", e),
        }
    }
}

#[derive(Clone)]
struct Client {
    username: String,
    password: String,
}

#[async_trait]
impl russh::client::Handler for Client {
    type Error = SshError;

    async fn check_server_key(
        &mut self,
        server_public_key: &PublicKey,
    ) -> Result<bool, Self::Error> {
        // println!("server_public_key: {}",server_public_key.fingerprint());
        Ok(true)
    }
}

#[derive(Clone)]
struct DummyHandler {
    captured_key: Arc<Mutex<Option<PublicKey>>>,
    notify: Arc<Mutex<Option<oneshot::Sender<()>>>>,
}

impl DummyHandler {
    fn new() -> (Self, oneshot::Receiver<()>) {
        let (tx, rx) = oneshot::channel();
        let handler = Self {
            captured_key: Arc::new(Mutex::new(None)),
            notify: Arc::new(Mutex::new(Some(tx))),
        };
        (handler, rx)
    }

    fn take_key(&self) -> Option<PublicKey> {
        self.captured_key.lock().unwrap().clone()
    }
}

#[async_trait::async_trait]
impl Handler for DummyHandler {
    type Error = SshError;

    async fn check_server_key(
        &mut self,
        server_public_key: &PublicKey,
    ) -> Result<bool, Self::Error> {
        // Store key
        *self.captured_key.lock().unwrap() = Some(server_public_key.clone());

        // Notify the waiter
        if let Some(tx) = self.notify.lock().unwrap().take() {
            let _ = tx.send(());
        }

        Ok(true)
    }
}

#[derive(Clone, Debug)]
pub struct ServerOptions {
    pub session_acquire_timeout: Duration,
    pub max_session_lifetime: Option<Duration>,
    pub max_channels_per_session: u32,
    pub max_sessions_per_server: u32,
}

impl ServerOptions {
    pub fn new() -> Self {
        Self {
            max_sessions_per_server: 2000,
            session_acquire_timeout: Duration::from_secs(30),
            max_session_lifetime: Some(Duration::from_secs(600)),
            max_channels_per_session: 5,
        }
    }
}

#[derive(Clone)]
struct Live {
    raw: Arc<Handle<Client>>,
    created_at: Instant,
}

#[derive(Clone)]
struct Idle {
    live: Live,
    idle_since: Instant,
}

pub struct ChannelGuard {
    pub channel: russh::Channel<Msg>,
    _permit: OwnedSemaphorePermit,
}

impl Drop for ChannelGuard {
    fn drop(&mut self) {
        // Channels are automatically closed when dropped
    }
}

#[derive(Clone)]
pub struct LiveSessionWrapper {
    live: Live,
    channel_semaphore: Arc<Semaphore>,
    max_permits: usize,
}

impl LiveSessionWrapper {
    fn new(live: Live, max_channels_per_session: usize) -> Self {
        Self {
            live,
            channel_semaphore: Arc::new(Semaphore::new(max_channels_per_session as usize)),
            max_permits: max_channels_per_session as usize,
        }
    }

    pub async fn get_channel_guard(&self) -> Result<ChannelGuard, SshError> {
        let permit = self
            .channel_semaphore
            .clone()
            .acquire_owned()
            .await
            .map_err(|_| {
                SshError::Io(io::Error::new(
                    io::ErrorKind::Other,
                    "Failed to acquire channel permit",
                ))
            })?;

        let channel = self
            .live
            .raw
            .channel_open_session()
            .await
            .map_err(|e| SshError::Russh(e))?;

        Ok(ChannelGuard {
            channel,
            _permit: permit,
        })
    }

    pub fn is_fully_idle(&self) -> bool {
        self.channel_semaphore.available_permits() == self.max_permits
    }
}

struct SessionPool {
    server_metadata: Arc<ServerMetadata>,
    active_sessions: Mutex<Vec<Arc<LiveSessionWrapper>>>,
    idle_sessions: Mutex<Vec<Idle>>,
    session_semaphore: Arc<Semaphore>,
}

impl SessionPool {
    fn new(server_metadata: &Arc<ServerMetadata>) -> Self {
        Self {
            server_metadata: server_metadata.clone(),
            active_sessions: Mutex::new(vec![]),
            idle_sessions: Mutex::new(vec![]),
            session_semaphore: Arc::new(Semaphore::new(
                server_metadata.max_sessions_per_server as usize,
            )),
        }
    }
    async fn connect(&self) -> Result<Handle<Client>, SshError> {
        let addr = format!(
            "{}:{}",
            self.server_metadata.host, self.server_metadata.ssh_port
        );
        let stream = tokio::time::timeout(
            self.server_metadata.connect_timeout,
            TcpStream::connect(&addr),
        )
        .await
        .map_err(|_| {
            SshError::Io(io::Error::new(
                io::ErrorKind::TimedOut,
                "Connection timed out",
            ))
        })?
        .map_err(|e| SshError::Io(io::Error::new(io::ErrorKind::Other, e)))?;

        let config = Arc::new(russh::client::Config::default());
        let client = Client {
            username: self.server_metadata.user.clone(),
            password: self.server_metadata.password.clone(),
        };
        let mut session = russh::client::connect_stream(config, stream, client).await?;

        let auth_result = session
            .authenticate_password(
                self.server_metadata.user.clone(),
                self.server_metadata.password.clone(),
            )
            .await
            .map_err(SshError::Russh)?;

        if !auth_result {
            return Err(SshError::Io(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "Authentication failed",
            )));
        }

        Ok(session)
    }

    pub async fn get_session(&self) -> Result<Arc<LiveSessionWrapper>, SshError> {
        {
            let mut idle_guard = self.idle_sessions.lock().unwrap();
            if let Some(idle) = idle_guard.pop() {
                let wrapper = Arc::new(LiveSessionWrapper::new(
                    idle.live,
                    self.server_metadata.max_channels_per_session,
                ));
                let mut active_guard = self.active_sessions.lock().unwrap();
                active_guard.push(wrapper.clone());
                return Ok(wrapper);
            }
        }

        {
            let active_guard = self.active_sessions.lock().unwrap();
            for s in active_guard.iter() {
                if s.channel_semaphore.available_permits() > 0 {
                    return Ok(s.clone());
                }
            }
        }

        let _permit = tokio::time::timeout(
            self.server_metadata.session_acquire_timeout,
            self.session_semaphore.clone().acquire_owned(),
        )
        .await
        .map_err(|_| {
            SshError::Io(io::Error::new(
                io::ErrorKind::TimedOut,
                "Session acquire timed out",
            ))
        })?
        .map_err(|_| SshError::Io(io::Error::new(io::ErrorKind::Other, "Server pool closed")))?;

        let sess = self.connect().await?;
        let live = Live {
            raw: Arc::new(sess),
            created_at: Instant::now(),
        };
        let wrapper = Arc::new(LiveSessionWrapper::new(
            live,
            self.server_metadata.max_channels_per_session,
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
                false
            } else {
                true
            }
        });
    }

    fn cleanup_idle(&self) {
        let mut idle_guard = self.idle_sessions.lock().unwrap();
        idle_guard.retain(|s| s.idle_since.elapsed() <= self.server_metadata.max_session_lifetime);
    }
}

pub struct ServerPool {
    servers: DashMap<u64, Arc<SessionPool>>,
    pub pending_clean_servers: Arc<DashSet<Arc<ServerMetadata>>>,
}

impl ServerPool {
    pub fn new() -> Self {
        Self {
            servers: DashMap::new(),
            pending_clean_servers: Arc::new(DashSet::new()),
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

    /// Check connectivity to many servers in parallel and record their SSH host keys.
    pub async fn check_servers_and_update_known_hosts(
        &self,
        servers: Vec<ServerConfig>,
    ) -> HashSet<String> {
        let mut futures = FuturesUnordered::new();

        for server in servers {
            let host = server.host.clone();
            let name = server.name.clone();
            let fut = task::spawn(Self::check_single_server_by_info(
                host,
                server.ssh_port.unwrap_or(DEFAULT_SSH_PORT),
                Some(name),
            ));
            futures.push(fut);
        }

        let mut failed_server_names = HashSet::new();

        while let Some(res) = futures.next().await {
            match res {
                Ok((server_name, server_host, result)) => {
                    if let Err(e) = result {
                        if let Some(name) = &server_name {
                            log_warn_direct!(
                                "Server '{}' ({}) failed: \n\t> {}",
                                name,
                                server_host,
                                e
                            );
                        } else {
                            log_warn_direct!("Server ({}) failed: \n\t> {}", server_host, e);
                        }
                        failed_server_names.insert(server_name.unwrap());
                    }
                }
                Err(e) => {
                    // The task itself panicked or was cancelled
                    log_error_direct!("Task error: \n\t> {}", e);
                    exit(1);
                    // Optional: can't get server name here
                }
            }
        }

        failed_server_names
    }

    /// Connect to a single SSH server by host, port, and name, and fetch its public key.
    pub async fn check_single_server_by_info(
        host: String,
        port: u16,
        name: Option<String>,
    ) -> (Option<String>, String, Result<(), String>) {
        let addr = format!("{}:{}", host, port);
        let res = match timeout(DEFAULT_SSH_HANDSHAKE_TIMEOUT, TcpStream::connect(&addr)).await {
            Ok(Ok(stream)) => {
                // Try to extract SSH host key
                match Self::fetch_ssh_host_key(stream).await {
                    Ok(key) => {
                        if let Err(e) = Self::add_host_to_known_hosts(&host, port, &key).await {
                            Err(format!("Failed to write known_hosts: {}", e))
                        } else {
                            Ok(())
                        }
                    }
                    Err(e) => Err(format!("Failed to fetch SSH key: {}", e)),
                }
            }
            Ok(Err(e)) => Err(format!("Connection error: {}", e)),
            Err(_) => Err("Initial connection timed out".to_string()),
        };

        (name, host.to_string(), res)
    }

    /// Fetch SSH public key from a remote host.
    /// This is a lightweight TCP-level handshake to read the server's identification and key.
    async fn fetch_ssh_host_key(stream: TcpStream) -> Result<PublicKey, String> {
        let config = Arc::new(Config::default());
        let (handler, rx) = DummyHandler::new();
        let handler_ref = handler.clone();

        // Connect: russh::client::connect_stream returns a Handle
        let handle = russh::client::connect_stream(config, stream, handler)
            .await
            .map_err(|e| format!("SSH handshake error: {}", e))?;

        // Wait for the check_server_key callback notification
        let _ = timeout(Duration::from_secs(10), rx)
            .await
            .map_err(|_| "Timed out waiting for server key".to_string())?;

        handle
            .disconnect(russh::Disconnect::ByApplication, "", "en")
            .await
            .ok();

        handler_ref
            .take_key()
            .ok_or_else(|| "Server did not send key".to_string())
    }

    /// Asynchronously append a host key to ~/.ssh/known_hosts
    pub async fn add_host_to_known_hosts(
        host: &str,
        port: u16,
        key: &PublicKey,
    ) -> std::io::Result<()> {
        // println!("add_host_to_known_hosts: {}", host);
        // Get home directory
        let mut path = dirs::home_dir().ok_or(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "Home directory not found",
        ))?;
        path.push(".ssh");
        tokio::fs::create_dir_all(&path).await.ok();
        path.push("known_hosts");

        // Encode the public key
        let encoded_key = key.public_key_base64();
        let line_to_add = if port == DEFAULT_SSH_PORT {
            format!("{} {} {}\n", host, key.name(), encoded_key)
        } else {
            format!("[{}]:{} {} {}\n", host, port, key.name(), encoded_key)
        };

        // If file exists, check if the same entry already exists
        let mut file_exists = false;
        let mut file_content = String::new();
        if tokio::fs::metadata(&path).await.is_ok() {
            file_exists = true;
            let mut file = tokio::fs::File::open(&path).await?;
            file.read_to_string(&mut file_content).await?;
        }

        // Check if same host+key already exists
        let already_exists = file_content.lines().any(|line| {
            let line = line.trim();
            line.starts_with(host) || line.starts_with(&format!("[{}]:{}", host, port))
        });
        if already_exists {
            // println!("key already exists, skip: {}", line_to_add);
            // Already stored, skip writing
            return Ok(());
        }

        // Append the new entry
        let mut file = tokio::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .await?;
        file.write_all(line_to_add.as_bytes()).await?;

        Ok(())
    }

    async fn get_session_pool(&self, server_metadata: &Arc<ServerMetadata>) -> Arc<SessionPool> {
        if let Some(pool) = self.servers.get(&server_metadata.server_key) {
            return pool.clone();
        }
        let server_pool = Arc::new(SessionPool::new(&server_metadata));
        self.servers
            .insert(server_metadata.server_key, server_pool.clone());
        server_pool
    }

    pub async fn get_channel(
        &self,
        server_metadata: &Arc<ServerMetadata>,
    ) -> Result<ChannelGuard, String> {
        let server_pool = self.get_session_pool(server_metadata).await;
        let live_session_wrapper = server_pool
            .get_session()
            .await
            .map_err(|e| format!("Get session error:\n\t> {}", e))?;

        let guard = live_session_wrapper
            .get_channel_guard()
            .await
            .map_err(|e| format!("Get channel error:\n\t> {}", e))?;

        Ok(guard)
    }

    pub fn start_idle_cleanup(self: Arc<Self>, interval: Duration) {
        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(interval);
            loop {
                ticker.tick().await;
                self.cleanup_idle_sessions();
            }
        });
    }

    pub async fn resolve_remote_path(
        &self,
        server_metadata: &Arc<ServerMetadata>,
        task_name: &str,
        use_sudo: bool,
        path: &str,
    ) -> Result<String, String> {
        self.exec(
            server_metadata,
            task_name,
            &format!("echo {}", path),
            use_sudo,
        )
        .await
    }

    pub async fn check_global_remote_temp_dir(
        &self,
        server_metadata: &Arc<ServerMetadata>,
        task_name: &str,
        use_sudo: bool,
        silent: bool,
    ) -> Result<(), String> {
        if use_sudo && server_metadata.user != "root" {
            self.pending_clean_servers.insert(server_metadata.clone());
            let exists = self
                .file_or_dir_exists(
                    server_metadata,
                    task_name,
                    REMOTE_TEMP_SBXCTL_FOLDER,
                    use_sudo,
                )
                .await?;
            if exists {
                ask_user(server_metadata,task_name,
                    format!(
                        "Remote path '{}' already exists. Transfering will DELETE it and use it as a temp folder. Continue?",
                        REMOTE_TEMP_SBXCTL_FOLDER
                    )
                    .as_str(),
                    silent,
                )
                .await?;

                self.exec(
                    server_metadata,
                    task_name,
                    &format!("rm -rf \"{}\"", REMOTE_TEMP_SBXCTL_FOLDER),
                    use_sudo,
                )
                .await?;
            }
        }
        Ok(())
    }

    pub async fn cleanup_pending_servers(&self) -> Result<(), String> {
        for server in self.pending_clean_servers.iter() {
            self.delete_global_temp_dir(&server, SYSTEM_TASK_NAME)
                .await?;
        }
        Ok(())
    }

    pub async fn file_or_dir_exists(
        &self,
        server_metadata: &Arc<ServerMetadata>,
        task_name: &str,
        path: &str,
        use_sudo: bool,
    ) -> Result<bool, String> {
        self.check_path(server_metadata, task_name, path, "-e", use_sudo)
            .await
    }

    pub async fn file_exists(
        &self,
        server_metadata: &Arc<ServerMetadata>,
        task_name: &str,
        path: &str,
        use_sudo: bool,
    ) -> Result<bool, String> {
        self.check_path(server_metadata, task_name, path, "-f", use_sudo)
            .await
    }

    pub async fn dir_exists(
        &self,
        server_metadata: &Arc<ServerMetadata>,
        task_name: &str,
        path: &str,
        use_sudo: bool,
    ) -> Result<bool, String> {
        self.check_path(server_metadata, task_name, path, "-d", use_sudo)
            .await
    }

    /// Check if any files or directories in `dir_path` match the given `pattern`
    pub async fn dir_has_pattern(
        &self,
        server_metadata: &Arc<ServerMetadata>,
        task_name: &str,
        dir_path: &str,
        pattern: &str,
        use_sudo: bool,
    ) -> Result<bool, String> {
        log_debug!(
            server_metadata,
            task_name,
            "Checking if directory '{}' has any content matching pattern '{}'",
            dir_path,
            pattern
        );

        // Use a one-liner shell command to safely check for at least one match
        // Exit 0 if any file/dir exists, exit 1 otherwise
        let full_cmd = format!(
            "sh -c 'for f in {}; do [ -e \"$f\" ] && exit 0; done; exit 1'",
            pattern
        );

        let result = self
            .exec(server_metadata, task_name, &full_cmd, use_sudo)
            .await;

        match result {
            Ok(_) => {
                log_debug!(
                    server_metadata,
                    task_name,
                    "Directory '{}' has content matching '{}': true",
                    dir_path,
                    pattern
                );
                Ok(true)
            }
            Err(e) => {
                // Exit status 1 means no match
                if e.contains("exit status 1") {
                    log_debug!(
                        server_metadata,
                        task_name,
                        "Directory '{}' has no content matching '{}'",
                        dir_path,
                        pattern
                    );
                    Ok(false)
                } else {
                    Err(format!(
                        "Failed to check directory '{}' for pattern '{}'. \n\t> {}",
                        dir_path, pattern, e
                    ))
                }
            }
        }
    }

    /// Check if directory contains any hidden files (names starting with .)
    pub async fn dir_has_hidden_items(
        &self,
        server_metadata: &Arc<ServerMetadata>,
        task_name: &str,
        dir_path: &str,
        use_sudo: bool,
    ) -> Result<bool, String> {
        let pattern = format!("{}/.[!.]*", dir_path);
        self.dir_has_pattern(server_metadata, task_name, dir_path, &pattern, use_sudo)
            .await
    }

    /// Check if directory contains any normal (non-hidden) files
    pub async fn dir_has_normal_items(
        &self,
        server_metadata: &Arc<ServerMetadata>,
        task_name: &str,
        dir_path: &str,
        use_sudo: bool,
    ) -> Result<bool, String> {
        let pattern = format!("{0}/*", dir_path);
        self.dir_has_pattern(server_metadata, task_name, dir_path, &pattern, use_sudo)
            .await
    }

    pub async fn check_path(
        &self,
        server_metadata: &Arc<ServerMetadata>,
        task_name: &str,
        path: &str,
        flag: &str,
        use_sudo: bool,
    ) -> Result<bool, String> {
        log_debug!(
            server_metadata,
            task_name,
            "Checking if '{}' exists with flag '{}'",
            path,
            flag
        );

        let full_cmd = format!("sh -c 'test {} {}'", flag, path);
        let result = self
            .exec(server_metadata, task_name, &full_cmd, use_sudo)
            .await;

        match result {
            Ok(_) => {
                log_debug!(
                    server_metadata,
                    task_name,
                    "Remote path '{}' exists (flag '{}'): true",
                    path,
                    flag
                );
                Ok(true)
            }
            Err(e) => {
                if e.contains("exit status 1") {
                    log_debug!(
                        server_metadata,
                        task_name,
                        "Remote path '{}' exists (flag '{}'): false",
                        path,
                        flag
                    );
                    Ok(false)
                } else {
                    Err(format!(
                        "Failed to check remote path '{}'. \n\t> {}",
                        path, e
                    ))
                }
            }
        }
    }

    pub async fn delete_global_temp_dir(
        &self,
        server_metadata: &Arc<ServerMetadata>,
        task_name: &str,
    ) -> Result<(), String> {
        let remote_temp_sbxctl_folder_exists = self
            .file_or_dir_exists(server_metadata, task_name, REMOTE_TEMP_SBXCTL_FOLDER, true)
            .await?;
        if remote_temp_sbxctl_folder_exists {
            log_debug!(
                server_metadata,
                task_name,
                "Clean up temp forder for server: host - {}, user - {}",
                server_metadata.host,
                server_metadata.user
            );
            self.exec(
                server_metadata,
                task_name,
                &format!("rm -rf \"{}\"", REMOTE_TEMP_SBXCTL_FOLDER),
                true,
            )
            .await?;
        }
        Ok(())
    }

    pub async fn exec(
        &self,
        server_metadata: &Arc<ServerMetadata>,
        task_name: &str,
        cmd: &str,
        use_sudo: bool,
    ) -> Result<String, String> {
        self.exec_with_stream(server_metadata, task_name, cmd, use_sudo, false)
            .await
    }

    pub async fn exec_with_log(
        &self,
        server_metadata: &Arc<ServerMetadata>,
        task_name: &str,
        cmd: &str,
        use_sudo: bool,
    ) -> Result<String, String> {
        self.exec_with_stream(server_metadata, task_name, cmd, use_sudo, true)
            .await
    }

    async fn exec_with_stream(
        &self,
        server_metadata: &Arc<ServerMetadata>,
        task_name: &str,
        cmd: &str,
        use_sudo: bool,
        print_log: bool,
    ) -> Result<String, String> {
        let cmd: String = cmd.to_string();
        let mut channel_guard = self.get_channel(&server_metadata).await?;

        log_debug!(
            server_metadata,
            task_name,
            "Streaming command: {} (sudo: {})",
            cmd,
            use_sudo
        );
        let full_cmd = if use_sudo {
            let escaped = cmd.replace("'", "'\\''");
            format!("sudo -S bash -c '{}'", escaped)
        } else {
            cmd.to_string()
        };
        log_debug!(
            server_metadata,
            task_name,
            "Full command: {} (sudo: {})",
            full_cmd,
            use_sudo
        );
        if use_sudo {
            channel_guard
                .channel
                .request_pty(true, "xterm", 0, 0, 0, 0, &[])
                .await
                .map_err(|e| format!("Failed to request pty for sudo. \n\t> {}", e))?;
        }

        channel_guard
            .channel
            .exec(true, full_cmd.as_bytes().to_vec())
            .await
            .map_err(|e| format!("Failed to execute command '{}'. \n\t> {}", cmd, e))?;

        if use_sudo {
            let mut data = Vec::new();
            while let Some(msg) = channel_guard.channel.wait().await {
                match msg {
                    russh::ChannelMsg::Data { data: channel_data } => {
                        data.extend_from_slice(&channel_data);
                        break;
                    }
                    _ => continue,
                }
            }
            let pw_with_newline = format!("{}\n", server_metadata.password);
            channel_guard
                .channel
                .data(pw_with_newline.as_bytes())
                .await
                .map_err(|e| format!("Failed to send sudo password. \n\t> {}", e))?;
        }

        let mut stdout_buf = String::new();
        let mut stderr_buf = String::new();
        let mut first_line = true;
        let mut stdout_collected = Vec::new();
        let mut stderr_collected = Vec::new();
        let mut partial_stdout_line = String::new();
        let mut partial_stderr_line = String::new();

        while let Some(msg) = channel_guard.channel.wait().await {
            match msg {
                ChannelMsg::Data { data } => {
                    stdout_collected.extend_from_slice(&data);
                    let data_str = String::from_utf8_lossy(&data).to_string();
                    partial_stdout_line.push_str(&data_str);

                    // Process complete lines
                    while let Some(line_end) = partial_stdout_line.find('\n') {
                        let line = partial_stdout_line[..line_end].to_string();
                        partial_stdout_line = partial_stdout_line[line_end + 1..].to_string();

                        if print_log {
                            // Remove the first \n or empty lines from pw_with_newline
                            if first_line && use_sudo && line.trim().is_empty() {
                                first_line = false;
                                continue;
                            }
                            execution_print(server_metadata, task_name, &line, false)?;
                        }

                        if first_line && use_sudo {
                            first_line = false;
                        }
                    }
                }
                ChannelMsg::ExtendedData { data, ext } if ext == 1 => {
                    stderr_collected.extend_from_slice(&data);
                    let data_str = String::from_utf8_lossy(&data).to_string();
                    partial_stderr_line.push_str(&data_str);

                    // Process complete lines
                    while let Some(line_end) = partial_stderr_line.find('\n') {
                        let line = partial_stderr_line[..line_end].to_string();
                        partial_stderr_line = partial_stderr_line[line_end + 1..].to_string();
                        // if print_log {
                        //     execution_print(&line, true)?;
                        // }
                    }
                }
                ChannelMsg::ExitStatus { exit_status } => {
                    if exit_status != 0 {
                        let stdout_str = String::from_utf8_lossy(&stdout_collected);
                        let stderr_str = String::from_utf8_lossy(&stderr_collected);
                        let mut msg =
                            format!("Command '{}' failed with exit status {}.", cmd, exit_status);
                        if !stdout_str.trim().is_empty() {
                            msg.push_str(&format!("\n\t> {}", stdout_str));
                        }
                        if !stderr_str.trim().is_empty() {
                            msg.push_str(&format!("\n\t> {}", stderr_str));
                        }
                        return Err(msg);
                    }
                    break;
                }
                _ => continue,
            }
        }

        // Handle any remaining partial lines
        if !partial_stdout_line.is_empty() && print_log {
            if !(first_line && use_sudo && partial_stdout_line.trim().is_empty()) {
                execution_print(server_metadata, task_name, &partial_stdout_line, false)?;
            }
        }
        if !partial_stderr_line.is_empty() && print_log {
            execution_print(server_metadata, task_name, &partial_stderr_line, true)?;
        }

        // Collect final output for return
        stdout_buf = String::from_utf8_lossy(&stdout_collected).to_string();
        stderr_buf = String::from_utf8_lossy(&stderr_collected).to_string();

        channel_guard
            .channel
            .close()
            .await
            .map_err(|e| format!("Failed to close channel. \n\t> {}", e))?;

        stdout_buf = stdout_buf.trim().to_string();
        log_debug!(
            server_metadata,
            task_name,
            "Streaming command output: '{}'",
            stdout_buf
        );
        Ok(stdout_buf)
    }

    pub async fn upload_file_or_dir_contents_into_dir(
        &self,
        server_metadata: &Arc<ServerMetadata>,
        task_name: &str,
        local_file_or_dir: &Path,
        remote_dir: &str,
        new_file_name: Option<&str>,
        use_sudo: bool,
        use_rsync: bool,
        silent: bool,
        direct_write_if_sudo: bool,
        print_log: bool,
    ) -> Result<(), String> {
        // println!("upload_file_or_dir_contents_into_dir remote_dir: ---{}---",remote_dir);
        let mut remote_temp_dir: Option<String> = None;
        if use_sudo && server_metadata.user != "root" && !direct_write_if_sudo {
            remote_temp_dir = Some(
                self.create_remote_temp_dir(server_metadata, task_name, "upload", use_sudo)
                    .await?,
            );
        }

        if local_file_or_dir.is_dir() {
            for entry in std::fs::read_dir(local_file_or_dir).map_err(|e| {
                format!(
                    "Failed to read local directory '{}'. \n\t> {}",
                    local_file_or_dir.display(),
                    e
                )
            })? {
                let entry =
                    entry.map_err(|e| format!("Error reading directory entry. \n\t> {}", e))?;
                let sub_path = entry.path();
                let base_name = get_local_path_base_name(&sub_path)?;
                let remote_sub = format!("{}/{}", remote_dir, base_name);

                self.ask_safe_to_transfer(
                    server_metadata,
                    task_name,
                    &remote_sub,
                    use_sudo,
                    silent,
                )
                .await?;
                if use_sudo && server_metadata.user != "root" && !direct_write_if_sudo {
                    let temp_dir = remote_temp_dir.as_ref().unwrap();
                    self.do_upload(
                        server_metadata,
                        task_name,
                        use_sudo,
                        use_rsync,
                        &sub_path,
                        &temp_dir,
                        new_file_name,
                    )
                    .await?;
                } else {
                    self.do_upload(
                        server_metadata,
                        task_name,
                        use_sudo,
                        use_rsync,
                        &sub_path,
                        &remote_dir,
                        new_file_name,
                    )
                    .await?;
                }
            }
            if use_sudo && server_metadata.user != "root" && !direct_write_if_sudo {
                let temp_dir = remote_temp_dir.as_ref().unwrap();
                self.move_and_delete_temp_dir(
                    server_metadata,
                    task_name,
                    temp_dir,
                    remote_dir,
                    use_sudo,
                )
                .await?;
            }
            if print_log {
                log_info!(
                    server_metadata,
                    task_name,
                    "Successfully uploaded the contents of the folder '{}' into '{}'",
                    local_file_or_dir.display(),
                    remote_dir
                );
            }
        } else {
            let remote_file = format!(
                "{}/{}",
                remote_dir,
                new_file_name.unwrap_or(get_local_path_base_name(&local_file_or_dir)?.as_str())
            );
            self.ask_safe_to_transfer(server_metadata, task_name, &remote_file, use_sudo, silent)
                .await?;
            if use_sudo && server_metadata.user != "root" && !direct_write_if_sudo {
                let temp_dir = remote_temp_dir.as_ref().unwrap();
                self.do_upload(
                    server_metadata,
                    task_name,
                    use_sudo,
                    use_rsync,
                    &local_file_or_dir,
                    &temp_dir,
                    new_file_name,
                )
                .await?;
                self.move_and_delete_temp_dir(
                    server_metadata,
                    task_name,
                    temp_dir,
                    remote_dir,
                    use_sudo,
                )
                .await?;
            } else {
                self.do_upload(
                    server_metadata,
                    task_name,
                    use_sudo,
                    use_rsync,
                    &local_file_or_dir,
                    &remote_dir,
                    new_file_name,
                )
                .await?;
            }
            if print_log {
                log_info!(
                    server_metadata,
                    task_name,
                    "Successfully uploaded the file '{}' to '{}'",
                    local_file_or_dir.display(),
                    remote_file
                );
            }
        }

        Ok(())
    }

    async fn move_and_delete_temp_dir(
        &self,
        server_metadata: &Arc<ServerMetadata>,
        task_name: &str,
        temp_dir: &str,
        remote_dir: &str,
        use_sudo: bool,
    ) -> Result<(), String> {
        // println!("move_and_delete_temp_dir remote_dir: ---{}---",remote_dir);
        let hidden_exists = self
            .dir_has_hidden_items(server_metadata, task_name, temp_dir, use_sudo)
            .await?;
        if hidden_exists {
            self.exec(
                server_metadata,
                task_name,
                &format!("mv \"{0}\"/.[!.]* \"{1}\"/", temp_dir, remote_dir),
                use_sudo,
            )
            .await?;
        }
        let normal_exists = self
            .dir_has_normal_items(server_metadata, task_name, temp_dir, use_sudo)
            .await?;
        // mv will not move hidden items
        if normal_exists {
            self.exec(
                server_metadata,
                task_name,
                &format!("mv \"{0}\"/* \"{1}\"/", temp_dir, remote_dir),
                use_sudo,
            )
            .await?;
        }

        self.exec(
            server_metadata,
            task_name,
            &format!("rm -rf \"{}\"", temp_dir),
            use_sudo,
        )
        .await?;

        Ok(())
    }

    #[async_recursion]
    async fn do_upload_with_scp_recursive(
        &self,
        server_metadata: &Arc<ServerMetadata>,
        task_name: &str,
        local_file_or_dir: &Path,
        remote_dir: &str,
        use_sudo: bool,
        new_base_name: Option<&str>,
    ) -> Result<(), String> {
        if !local_file_or_dir.exists() {
            return Err(format!(
                "Local path '{}' does not exist",
                local_file_or_dir.display()
            ));
        }

        let base_name = new_base_name
            .map(|s| s.to_string())
            .unwrap_or(get_local_path_base_name(&local_file_or_dir)?);
        let remote_target = format!("{}/{}", remote_dir, base_name);

        if local_file_or_dir.is_dir() {
            self.create_remote_dir(server_metadata, task_name, remote_target.as_str(), use_sudo)
                .await?;
            for entry in std::fs::read_dir(local_file_or_dir).map_err(|e| {
                format!(
                    "Failed to read local directory '{}'. \n\t> {}",
                    local_file_or_dir.display(),
                    e
                )
            })? {
                let entry =
                    entry.map_err(|e| format!("Error reading directory entry. \n\t> {}", e))?;
                let sub_path = entry.path();

                self.do_upload_with_scp_recursive(
                    server_metadata,
                    task_name,
                    &sub_path,
                    &remote_target,
                    use_sudo,
                    None,
                )
                .await?;
            }
        } else {
            let mut file = File::open(local_file_or_dir)
                .map_err(|e| format!("Failed to open local file {:?}: {}", local_file_or_dir, e))?;
            let metadata = file.metadata().map_err(|e| format!("metadata: {}", e))?;
            let mut buffer = Vec::new();
            file.read_to_end(&mut buffer)
                .map_err(|e| format!("read: {}", e))?;

            // Enable scp receiver
            let scp_cmd = format!("scp -t {}", remote_target);
            let mut channel_guard = self.get_channel(&server_metadata).await?;
            channel_guard
                .channel
                .exec(true, scp_cmd.as_bytes().to_vec())
                .await
                .map_err(|e| e.to_string())?;

            // Wait for ACK
            self.wait_for_ack(&mut channel_guard.channel).await?;

            // Send header
            let filename = local_file_or_dir.file_name().unwrap().to_str().unwrap();
            let mode = 0o644;
            let header = format!("C{:04o} {} {}\n", mode, buffer.len(), filename);
            channel_guard
                .channel
                .data(header.as_bytes())
                .await
                .map_err(|e| e.to_string())?;
            self.wait_for_ack(&mut channel_guard.channel).await?;

            // Send file data
            let mut reader =
                BufReader::new(File::open(local_file_or_dir).map_err(|e| e.to_string())?);
            let mut buf = [0u8; 8192];
            loop {
                let n = reader.read(&mut buf).map_err(|e| e.to_string())?;
                if n == 0 {
                    break;
                }
                let mut cursor = Cursor::new(&buf[..n]);
                channel_guard
                    .channel
                    .data(&mut cursor)
                    .await
                    .map_err(|e| e.to_string())?;
            }

            // Send end null byte
            let mut cursor = Cursor::new(&[0u8]);
            channel_guard
                .channel
                .data(&mut cursor)
                .await
                .map_err(|e| e.to_string())?;

            channel_guard
                .channel
                .close()
                .await
                .map_err(|e| e.to_string())?;
        }

        Ok(())
    }

    async fn wait_for_ack(&self, channel: &mut russh::Channel<Msg>) -> Result<(), String> {
        let mut ack_buf = Vec::new();
        while let Some(msg) = channel.wait().await {
            if let ChannelMsg::Data { data } = msg {
                ack_buf.extend_from_slice(&data);
                if ack_buf.contains(&0) {
                    return Ok(());
                } else if ack_buf.contains(&1) || ack_buf.contains(&2) {
                    return Err(format!(
                        "SCP remote error: {:?}",
                        String::from_utf8_lossy(&ack_buf)
                    ));
                }
            }
        }
        Err("SCP: No ACK from server".to_string())
    }

    fn command_exists(&self, cmd: &str) -> bool {
        std::process::Command::new("which")
            .arg(cmd)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }

    async fn do_upload(
        &self,
        server_metadata: &Arc<ServerMetadata>,
        task_name: &str,
        use_sudo: bool,
        use_rsync: bool,
        local_file_or_dir: &Path,
        remote_dir: &str,
        new_file_name: Option<&str>,
    ) -> Result<(), String> {
        log_debug!(
            server_metadata,
            task_name,
            "Attempting to upload '{}' to '{}'",
            local_file_or_dir.display(),
            remote_dir
        );

        if use_rsync && self.command_exists("rsync") {
            log_debug!(server_metadata, task_name, "Using rsync for upload");
            if !server_metadata.password.is_empty() {
                if self.command_exists("sshpass") {
                    let remote_target = if let Some(name) = new_file_name {
                        format!("{}/{}", remote_dir, name)
                    } else {
                        remote_dir.to_string()
                    };
                    let mut sshpass_cmd = std::process::Command::new("sshpass");
                    sshpass_cmd
                        .arg("-p")
                        .arg(&server_metadata.password)
                        .arg("rsync")
                        .arg("-avz")
                        .arg("-e")
                        .arg(format!("ssh -p {}", server_metadata.ssh_port))
                        .arg(
                            local_file_or_dir
                                .to_str()
                                .ok_or("Invalid local path encoding")?,
                        )
                        .arg(format!(
                            "{}@{}:{}",
                            server_metadata.user, server_metadata.host, remote_target
                        ))
                        .stdout(std::process::Stdio::null())
                        .stderr(std::process::Stdio::null());

                    let status = sshpass_cmd
                        .status()
                        .map_err(|e| format!("Failed to execute rsync via sshpass. \n\t> {}", e))?;

                    if !status.success() {
                        return Err(format!(
                            "rsync (sshpass) failed with exit code {}",
                            status.code().unwrap_or(-1)
                        ));
                    }
                    return Ok(());
                } else {
                    return Err("rsync password required but sshpass not found".to_string());
                }
            }
        } else {
            log_debug!(
                server_metadata,
                task_name,
                "Starting SCP upload from '{}' to '{}'",
                local_file_or_dir.display(),
                remote_dir
            );
            self.do_upload_with_scp_recursive(
                server_metadata,
                task_name,
                local_file_or_dir,
                remote_dir,
                use_sudo,
                new_file_name,
            )
            .await?;
            log_debug!(
                server_metadata,
                task_name,
                "SCP upload to '{}' completed",
                remote_dir
            );
        }

        Ok(())
    }

    pub async fn ask_safe_to_transfer(
        &self,
        server_metadata: &Arc<ServerMetadata>,
        task_name: &str,
        remote_path: &str,
        use_sudo: bool,
        silent: bool,
    ) -> Result<(), String> {
        let is_file = self
            .file_exists(server_metadata, task_name, remote_path, use_sudo)
            .await?;
        let is_dir = self
            .dir_exists(server_metadata, task_name, remote_path, use_sudo)
            .await?;

        if is_file || is_dir {
            let prompt = if is_file {
                format!(
                    "Remote file '{}' already exists. Overwriting will DELETE it. Continue?",
                    remote_path
                )
            } else {
                format!(
                    "Remote directory '{}' already exists. Overwriting will DELETE it and all its contents. Continue?",
                    remote_path
                )
            };

            ask_user(server_metadata, task_name, &prompt, silent).await?;
            self.exec(
                server_metadata,
                task_name,
                &format!("rm -rf \"{}\"", remote_path),
                use_sudo,
            )
            .await
            .map_err(|e| {
                format!(
                    "Failed to remove existing remote {} '{}'. \n\t> {}",
                    if is_file { "file" } else { "directory" },
                    remote_path,
                    e
                )
            })?;
            if silent {
                log_warn!(
                    server_metadata,
                    task_name,
                    "Remote {} has been overwritten: '{}'",
                    if is_file { "file" } else { "directory" },
                    remote_path
                );
            } else {
                log_info!(
                    server_metadata,
                    task_name,
                    "Remote {} has been overwritten: '{}'",
                    if is_file { "file" } else { "directory" },
                    remote_path
                );
            }
            return Ok(());
        }

        Ok(())
    }

    pub async fn validate_remote_dir(
        &self,
        server_metadata: &Arc<ServerMetadata>,
        task_name: &str,
        remote_dir: &str,
        use_sudo: bool,
    ) -> Result<(), String> {
        log_debug!(
            server_metadata,
            task_name,
            "Ensuring remote directory '{}' exists",
            remote_dir
        );
        if self
            .file_exists(server_metadata, task_name, remote_dir, use_sudo)
            .await?
        {
            return Err(format!(
                "Path '{}' exists and is a file, not a directory",
                remote_dir
            ));
        }

        if self
            .dir_exists(server_metadata, task_name, remote_dir, use_sudo)
            .await?
        {
            log_debug!(
                server_metadata,
                task_name,
                "Checking if remote directory '{}' is writable",
                remote_dir
            );
            let check_cmd = format!("test -w \"{}\"; echo $?", remote_dir);
            let output = self
                .exec(server_metadata, task_name, &check_cmd, use_sudo)
                .await
                .map_err(|e| {
                    format!(
                        "Failed to check write permission for '{}'. \n\t> {}",
                        remote_dir, e
                    )
                })?;
            if output.trim() != "0" {
                return Err(format!("Directory '{}' is not writable", remote_dir));
            }
        } else {
            self.create_remote_dir(server_metadata, task_name, remote_dir, use_sudo)
                .await?;
        }

        Ok(())
    }

    pub async fn create_remote_dir_if_not_exists(
        &self,
        server_metadata: &Arc<ServerMetadata>,
        task_name: &str,
        remote_dir: &str,
        use_sudo: bool,
    ) -> Result<(), String> {
        if self
            .dir_exists(server_metadata, task_name, remote_dir, use_sudo)
            .await?
        {
            return Ok(());
        }
        self.create_remote_dir(server_metadata, task_name, remote_dir, use_sudo)
            .await
    }

    async fn create_remote_dir(
        &self,
        server_metadata: &Arc<ServerMetadata>,
        task_name: &str,
        remote_dir: &str,
        use_sudo: bool,
    ) -> Result<(), String> {
        let cmd = if use_sudo {
            format!(
                "mkdir -p \"{}\"; chown {} \"{}\"; chmod 700 \"{}\"",
                remote_dir, server_metadata.user, remote_dir, remote_dir
            )
        } else {
            format!("mkdir -p \"{}\"; chmod 700 \"{}\"", remote_dir, remote_dir)
        };

        self.exec(server_metadata, task_name, &cmd, use_sudo)
            .await
            .map_err(|e| {
                format!(
                    "Failed to create remote directory '{}'. \n\t> {}",
                    remote_dir, e
                )
            })?;
        Ok(())
    }

    pub async fn create_remote_temp_dir(
        &self,
        server_metadata: &Arc<ServerMetadata>,
        task_name: &str,
        prefix: &str,
        use_sudo: bool,
    ) -> Result<String, String> {
        let temp_dir = generate_remote_temp_dir(prefix);
        log_debug!(
            server_metadata,
            task_name,
            "Uploading to temporary path '{}' with sudo",
            temp_dir
        );
        self.create_remote_dir_if_not_exists(
            server_metadata,
            task_name,
            temp_dir.as_str(),
            use_sudo,
        )
        .await?;
        Ok(temp_dir)
    }
}
