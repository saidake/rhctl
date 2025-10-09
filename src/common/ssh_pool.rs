use crate::domain::cmd_params::ServerMetadata;
use crate::domain::constants::REMOTE_TEMP_SBXCTL_FOLDER;
use crate::utils::file_utils::{generate_remote_temp_dir, get_local_path_base_name};
use crate::utils::log_utils::ask_user;
use crate::utils::ssh_utils::execution_print;
use crate::{log_debug, log_info};
use async_recursion::async_recursion;
use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::Path;

use dashmap::DashMap;
use dashmap::DashSet;
use ssh2::Channel;
use ssh2::Session;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::io;
use std::io::{Error, ErrorKind};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
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
    _permit: OwnedSemaphorePermit, // RAII for channel slot
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
    fn new(live: Live, max_channel: u32) -> Self {
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
    pub pending_clean_servers: Arc<DashSet<Arc<ServerMetadata>>>,
    options: PoolOptions,
}

impl ServerPool {
    pub fn new(options: PoolOptions) -> Self {
        Self {
            servers: DashMap::new(),
            pending_clean_servers: Arc::new(DashSet::new()),
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

    async fn get_session_pool(&self, server_metadata: &Arc<ServerMetadata>) -> Arc<SessionPool> {
        if let Some(pool) = self.servers.get(&server_metadata.server_key) {
            return pool.clone();
        }

        let connect_opts = ConnectOptions::new(
            &server_metadata.host,
            server_metadata.ssh_port,
            &server_metadata.user,
            &server_metadata.password,
        );
        let server_pool = Arc::new(SessionPool::new(connect_opts, self.options.clone()));
        self.servers
            .insert(server_metadata.server_key, server_pool.clone());
        server_pool
    }

    // Behavior functions
    pub async fn get_channel(
        &self,
        server_metadata: &Arc<ServerMetadata>,
    ) -> Result<ChannelGuard, String> {
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
        use_sudo: bool,
        path: &str,
    ) -> Result<String, String> {
        self.exec(server_metadata, &format!("echo {}", path), use_sudo)
            .await
    }
    /// Check if a remote path REMOTE_TEMP_SBXCTL_FOLDER exists, ask user for overwrite if needed,
    /// and delete it if confirmed.
    /// Returns Ok(true) if deleted, Ok(false) if skipped, Err(...) on error.
    pub async fn check_global_remote_temp_dir(
        &self,
        server_metadata: &Arc<ServerMetadata>,
        use_sudo: bool,
        silent: bool,
    ) -> Result<(), String> {
        // Only non-root sudo users need to handle the global temp folder
        if use_sudo && server_metadata.user != "root" {
            self.pending_clean_servers.insert(server_metadata.clone());
            let exists = self
                .file_or_dir_exists(server_metadata, REMOTE_TEMP_SBXCTL_FOLDER, use_sudo)
                .await?;
            // println!("exists: {}",exists);
            if exists {
                ask_user(format!(
                        "Remote path '{}' already exists. Transfering will DELETE it and use it as a temp folder. Continue?",
                        REMOTE_TEMP_SBXCTL_FOLDER
                    ).as_str(),silent).await?;

                self.exec(
                    server_metadata,
                    &format!("rm -rf \"{}\"", REMOTE_TEMP_SBXCTL_FOLDER),
                    use_sudo,
                )
                .await?;
                // .map_err(|e| {
                //     format!(
                //         "Failed to remove existing remote '{}'. \n\t{}",
                //         REMOTE_TEMP_SBXCTL_FOLDER, e
                //     )
                // })?;
            }
        }
        Ok(())
    }

    pub async fn cleanup_pending_servers(&self) -> Result<(), String> {
        for server in self.pending_clean_servers.iter() {
            self.delete_global_temp_dir(&server).await?;
        }
        Ok(())
    }

    /// Check if file or directory exists (-e).
    pub async fn file_or_dir_exists(
        &self,
        server_metadata: &Arc<ServerMetadata>,
        path: &str,
        use_sudo: bool,
    ) -> Result<bool, String> {
        self.check_path(server_metadata, path, "-e", use_sudo).await
    }

    /// Check if file exists (-f).
    pub async fn file_exists(
        &self,
        server_metadata: &Arc<ServerMetadata>,
        path: &str,
        use_sudo: bool,
    ) -> Result<bool, String> {
        self.check_path(server_metadata, path, "-f", use_sudo).await
    }

    /// Check if directory exists (-d).
    pub async fn dir_exists(
        &self,
        server_metadata: &Arc<ServerMetadata>,
        path: &str,
        use_sudo: bool,
    ) -> Result<bool, String> {
        self.check_path(server_metadata, path, "-d", use_sudo).await
    }

    /// Check if a remote path exists with specific test flag (-e, -f, -d).
    pub async fn check_path(
        &self,
        server_metadata: &Arc<ServerMetadata>,
        path: &str,
        flag: &str,
        use_sudo: bool,
    ) -> Result<bool, String> {
        log_debug!("Checking if '{}' exists with flag '{}'", path, flag);

        let full_cmd = format!("sh -c 'test {} {}'", flag, path);
        // println!(
        //     "path: {}, full_cmd: {}, use_sudo: {}",
        //     path, full_cmd, use_sudo
        // );

        let result = self.exec(server_metadata, &full_cmd, use_sudo).await;

        match result {
            Ok(_) => {
                log_debug!("Remote path '{}' exists (flag '{}'): true", path, flag);
                Ok(true)
            }
            Err(e) => {
                // If test returns 1 → path does not exist
                if e.contains("exit status 1") {
                    log_debug!("Remote path '{}' exists (flag '{}'): false", path, flag);
                    Ok(false)
                } else {
                    // Other errors (such as insufficient sudo permissions) return Err directly
                    Err(format!("Failed to check remote path '{}'. \n\t{}", path, e))
                }
            }
        }
    }

    pub async fn delete_global_temp_dir(
        &self,
        server_metadata: &Arc<ServerMetadata>,
    ) -> Result<(), String> {
        // println!("delete_global_temp_dir");
        let remote_temp_sbxctl_folder_exists = self
            .file_or_dir_exists(server_metadata, REMOTE_TEMP_SBXCTL_FOLDER, true)
            .await?;
        // println!(
        //     "remote_temp_sbxctl_folder_exists: {}",
        //     remote_temp_sbxctl_folder_exists
        // );
        if remote_temp_sbxctl_folder_exists {
            self.exec(
                server_metadata,
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
        cmd: &str,
        use_sudo: bool,
    ) -> Result<String, String> {
        self.exec_with_stream(server_metadata, cmd, use_sudo, false)
            .await
    }

    pub async fn exec_with_log(
        &self,
        server_metadata: &Arc<ServerMetadata>,
        cmd: &str,
        use_sudo: bool,
    ) -> Result<String, String> {
        self.exec_with_stream(server_metadata, cmd, use_sudo, true)
            .await
    }

    async fn exec_with_stream(
        &self,
        server_metadata: &Arc<ServerMetadata>,
        cmd: &str,
        use_sudo: bool,
        print_log: bool,
    ) -> Result<String, String> {
        let cmd: String = cmd.to_string();
        let mut channel_guard = self.get_channel(&server_metadata).await?;

        log_debug!("Streaming command: {} (sudo: {})", cmd, use_sudo);
        let full_cmd = if use_sudo {
            let escaped = cmd.replace("'", "'\\''");
            format!("sudo -S bash -c '{}'", escaped)
        } else {
            cmd.to_string()
        };

        // println!("full_cmd: {}", full_cmd);
        if use_sudo {
            channel_guard
                .channel
                .request_pty("xterm", None, None)
                .map_err(|e| format!("Failed to request pty for sudo. \n\t{}", e))?;
        }

        channel_guard
            .channel
            .exec(&full_cmd)
            .map_err(|e| format!("Failed to execute command '{}'. \n\t{}", cmd, e))?;

        // Input password
        if use_sudo {
            let mut prompt_buf = [0u8; 1024];
            channel_guard
                .channel
                .read(&mut prompt_buf)
                .map_err(|e| format!("Failed to read sudo prompt: {}", e))?;
            let pw_with_newline = format!("{}\n", server_metadata.password);
            channel_guard
                .channel
                .write_all(pw_with_newline.as_bytes())
                .map_err(|e| format!("Failed to send sudo password. \n\t{}", e))?;
        }

        // Read stdout
        let mut stdout_buf = String::new();
        let mut stderr_buf = String::new();

        {
            let stdout = channel_guard.channel.stream(0);
            let stdout_reader = BufReader::new(stdout);
            let mut first_line = true;
            for line in stdout_reader.lines() {
                let line = line.map_err(|e| format!("Failed to read stdout. \n\t{}", e))?;
                if first_line && use_sudo && line.trim().is_empty() {
                    first_line = false;
                    continue;
                }
                // println!("line: ------------{}--------", line);
                stdout_buf.push_str(&line);
                stdout_buf.push('\n');
                if print_log {
                    execution_print(&line, false)?;
                }
            }
        }

        // Read stderr
        {
            let stderr = channel_guard.channel.stderr();
            let stderr_reader = BufReader::new(stderr);
            for line in stderr_reader.lines() {
                let line = line.map_err(|e| format!("Failed to read stderr. \n\t{}", e))?;
                stderr_buf.push_str(&line);
                stderr_buf.push('\n');
                if print_log {
                    execution_print(&line, true)?;
                }
            }
        }

        channel_guard
            .channel
            .wait_close()
            .map_err(|e| format!("Failed to close channel. \n\t{}", e))?;
        let exit_status = channel_guard
            .channel
            .exit_status()
            .map_err(|e| format!("Failed to get exit status. \n\t{}", e))?;

        log_debug!("Command exit status: {}", exit_status);

        if exit_status != 0 {
            let mut msg = format!("Command '{}' failed with exit status {}.", cmd, exit_status);
            if !stdout_buf.trim().is_empty() {
                msg.push_str(&format!("\n\tstdout:\n\t{}", stdout_buf));
            }
            if !stderr_buf.trim().is_empty() {
                msg.push_str(&format!("\n\tstderr:\n\t{}", stderr_buf));
            }
            return Err(msg);
        }
        if stdout_buf.ends_with('\n') {
            stdout_buf.pop();
        }
        // println!("stdout_buf: ------------{}--------", stdout_buf);
        Ok(stdout_buf)
    }
    // This method does not create remote_dir; you should ensure the directory exists and is writable.
    pub async fn upload_file_or_dir_contents_into_dir(
        &self,
        server_metadata: &Arc<ServerMetadata>,
        local_file_or_dir: &Path,
        remote_dir: &str,
        new_file_name: Option<&str>,
        use_sudo: bool,
        use_rsync: bool,
        silent: bool,
        direct_write_if_sudo: bool,
        print_log: bool,
    ) -> Result<(), String> {
        // Create a temp directory for the current user
        let mut remote_temp_dir: Option<String> = None;
        if use_sudo && server_metadata.user != "root" && !direct_write_if_sudo {
            remote_temp_dir = Some(
                self.create_remote_temp_dir(server_metadata, "upload", use_sudo)
                    .await?,
            );
        }

        // Create remote directory with appropriate permissions
        if local_file_or_dir.is_dir() {
            for entry in std::fs::read_dir(local_file_or_dir).map_err(|e| {
                format!(
                    "Failed to read local directory '{}'. \n\t{}",
                    local_file_or_dir.display(),
                    e
                )
            })? {
                let entry =
                    entry.map_err(|e| format!("Error reading directory entry. \n\t{}", e))?;
                let sub_path = entry.path();
                let base_name = get_local_path_base_name(&sub_path)?;
                let remote_sub = format!("{}/{}", remote_dir, base_name);

                // Check if remote file or directory exists
                self.ask_safe_to_transfer(server_metadata, &remote_sub, use_sudo, silent)
                    .await?;
                // thread::sleep(Duration::from_secs(500000));
                if use_sudo && server_metadata.user != "root" && !direct_write_if_sudo {
                    // println!("sub_path: {}", sub_path.display());

                    let temp_dir = remote_temp_dir.as_ref().unwrap();
                    // println!("do_upload_with_scp_recursive");
                    self.do_upload(
                        server_metadata,
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
                // Move the content in temp_dir to the remote_dir
                // println!("move_and_delete_temp_dir - temp_dir: {}", temp_dir);
                // println!("move_and_delete_temp_dir - remote_dir: {}", remote_dir);
                // thread::sleep(Duration::from_secs(30));
                self.move_and_delete_temp_dir(server_metadata, temp_dir, remote_dir, use_sudo)
                    .await?;
            }
            if print_log {
                log_info!(
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
            self.ask_safe_to_transfer(server_metadata, &remote_file, use_sudo, silent)
                .await?;
            if use_sudo && server_metadata.user != "root" && !direct_write_if_sudo {
                let temp_dir = remote_temp_dir.as_ref().unwrap();
                // println!("do_upload_with_scp_recursive");
                self.do_upload(
                    server_metadata,
                    use_sudo,
                    use_rsync,
                    &local_file_or_dir,
                    &temp_dir,
                    new_file_name,
                )
                .await?;
                // Move the content in temp_dir to the remote_dir
                // println!("move_and_delete_temp_dir - temp_dir: {}", temp_dir);
                // println!("move_and_delete_temp_dir - remote_dir: {}", remote_dir);
                // thread::sleep(Duration::from_secs(30));

                self.move_and_delete_temp_dir(server_metadata, temp_dir, remote_dir, use_sudo)
                    .await?;
            } else {
                self.do_upload(
                    server_metadata,
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
        temp_dir: &str,
        remote_dir: &str,
        use_sudo: bool,
    ) -> Result<(), String> {
        // Move hidden files if any exist
        let hidden_exists = self
            .file_or_dir_exists(server_metadata, &format!("{}/.[!.]*", temp_dir), use_sudo)
            .await?;
        // println!("hidden_exists: {}", hidden_exists);
        if hidden_exists {
            self.exec(
                server_metadata,
                &format!("mv \"{0}\"/.[!.]* \"{1}\"/", temp_dir, remote_dir),
                use_sudo,
            )
            .await?;
        }

        // Move normal files if any exist
        let normal_exists = self
            .file_or_dir_exists(server_metadata, &format!("{0}/*", temp_dir), use_sudo)
            .await?;
        // println!("normal_exists: {}", normal_exists);
        if normal_exists {
            self.exec(
                server_metadata,
                &format!("mv \"{0}\"/* \"{1}\"/", temp_dir, remote_dir),
                use_sudo,
            )
            .await?;
        }
        // thread::sleep(Duration::from_secs(30));
        // Remove the temporary directory
        self.exec(
            server_metadata,
            &format!("rm -rf \"{}\"", temp_dir),
            use_sudo,
        )
        .await?;

        Ok(())
    }

    // Upload a local file or directory to a remote directory via SCP recursively.
    // An error occurs if the file’s parent directory does not exist.
    #[async_recursion]
    async fn do_upload_with_scp_recursive(
        &self,
        server_metadata: &Arc<ServerMetadata>,
        local_file_or_dir: &Path,
        remote_dir: &str,
        use_sudo: bool,
        new_base_name: Option<&str>,
    ) -> Result<(), String> {
        // println!("local_file_or_dir: {}", local_file_or_dir.display());
        // println!("remote_dir: {}", remote_dir);
        // thread::sleep(Duration::from_secs(60));
        if !local_file_or_dir.exists() {
            return Err(format!(
                "Local path '{}' does not exist",
                local_file_or_dir.display()
            ));
        }

        let base_name = new_base_name
            .map(|s| s.to_string())
            .unwrap_or(get_local_path_base_name(&local_file_or_dir)?);
        // Build the remote target path
        let remote_target = format!("{}/{}", remote_dir, base_name);
        // println!(
        //     "local_file_or_dir:{}, remote_target: {}",
        //     local_file_or_dir.display(),
        //     remote_target
        // );

        // Create remote directory if local path is a directory
        if local_file_or_dir.is_dir() {
            // println!("local_file_or_dir.is_dir");
            self.create_remote_dir(server_metadata, remote_target.as_str(), use_sudo)
                .await?;
            // Recursively upload each entry
            for entry in fs::read_dir(local_file_or_dir).map_err(|e| {
                format!(
                    "Failed to read local directory '{}'. \n\t{}",
                    local_file_or_dir.display(),
                    e
                )
            })? {
                let entry =
                    entry.map_err(|e| format!("Error reading directory entry. \n\t{}", e))?;
                let sub_path = entry.path();

                // Recursive call
                self.do_upload_with_scp_recursive(
                    server_metadata,
                    &sub_path,
                    &remote_target,
                    use_sudo,
                    None,
                )
                .await?;
            }
        } else {
            // Local path is a file → upload
            let mut file = fs::File::open(local_file_or_dir).map_err(|e| {
                format!(
                    "Failed to open local file '{}'. \n\t{}",
                    local_file_or_dir.display(),
                    e
                )
            })?;

            let stat = file.metadata().map_err(|e| {
                format!(
                    "Failed to get metadata for '{}'. \n\t{}",
                    local_file_or_dir.display(),
                    e
                )
            })?;

            log_debug!(
                "Uploading file '{}' ({} bytes) to '{}'",
                local_file_or_dir.display(),
                stat.len(),
                remote_target
            );

            let mut buffer = Vec::new();
            file.read_to_end(&mut buffer).map_err(|e| {
                format!(
                    "Failed to read file '{}'. \n\t{}",
                    local_file_or_dir.display(),
                    e
                )
            })?;

            //                     self
            // .use_channel(&server_metadata, move |channel| {

            // }).await?;
            let mut channel_guard = self.get_channel(&server_metadata).await?;

            channel_guard.channel.write_all(&buffer).map_err(|e| {
                format!(
                    "Failed to write to SCP channel for '{}'. \n\t{}",
                    remote_target, e
                )
            })?;
            channel_guard.channel.send_eof().map_err(|e| {
                format!(
                    "Failed to send EOF for SCP upload to '{}'. \n\t{}",
                    remote_target, e
                )
            })?;
            channel_guard.channel.wait_eof().map_err(|e| {
                format!(
                    "Failed to wait for EOF for SCP upload to '{}'. \n\t{}",
                    remote_target, e
                )
            })?;
            channel_guard.channel.close().map_err(|e| {
                format!(
                    "Failed to close SCP channel for '{}'. \n\t{}",
                    remote_target, e
                )
            })?;
            channel_guard.channel.wait_close().map_err(|e| {
                format!(
                    "Failed to wait for SCP channel close for '{}'. \n\t{}",
                    remote_target, e
                )
            })?;
        }
        // thread::sleep(Duration::from_secs(1500000000));
        Ok(())
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
        use_sudo: bool,
        use_rsync: bool,
        local_file_or_dir: &Path,
        remote_dir: &str,
        new_file_name: Option<&str>,
    ) -> Result<(), String> {
        log_debug!(
            "Attempting to upload '{}' to '{}'",
            local_file_or_dir.display(),
            remote_dir
        );

        if use_rsync && self.command_exists("rsync") {
            log_debug!("Using rsync for upload");
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
                        .map_err(|e| format!("Failed to execute rsync via sshpass. \n\t{}", e))?;

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
                "Starting SCP upload from '{}' to '{}'",
                local_file_or_dir.display(),
                remote_dir
            );
            self.do_upload_with_scp_recursive(
                server_metadata,
                local_file_or_dir,
                remote_dir,
                use_sudo,
                new_file_name,
            )
            .await?;
            log_debug!("SCP upload to '{}' completed", remote_dir);
        }

        Ok(())
    }

    /// Check if a remote path exists (file or directory), ask user for overwrite if needed,
    /// and delete it if confirmed.
    /// Returns Ok(true) if deleted, Ok(false) if skipped, Err(...) on error.
    pub async fn ask_safe_to_transfer(
        &self,
        server_metadata: &Arc<ServerMetadata>,
        remote_path: &str,
        use_sudo: bool,
        silent: bool,
    ) -> Result<(), String> {
        // Check if remote path is a file or directory
        let is_file = self
            .file_exists(server_metadata, remote_path, use_sudo)
            .await?;
        let is_dir = self
            .dir_exists(server_metadata, remote_path, use_sudo)
            .await?;
        // println!(
        //     "is_file: {}, is_dir: {}, remote_path: {}",
        //     is_file, is_dir, remote_path
        // );

        if is_file || is_dir {
            // Build prompt message
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

            // Ask user unless in silent mode
            ask_user(&prompt, silent).await?;
            // Execute deletion
            self.exec(
                server_metadata,
                &format!("rm -rf \"{}\"", remote_path),
                use_sudo,
            )
            .await
            .map_err(|e| {
                format!(
                    "Failed to remove existing remote {} '{}'. \n\t{}",
                    if is_file { "file" } else { "directory" },
                    remote_path,
                    e
                )
            })?;
            // println!("delete remote_path: {}", remote_path);
            return Ok(()); // Deleted successfully
        }

        Ok(()) // Path does not exist
    }

    pub async fn validate_remote_dir(
        &self,
        server_metadata: &Arc<ServerMetadata>,
        remote_dir: &str,
        use_sudo: bool,
    ) -> Result<(), String> {
        log_debug!("Ensuring remote directory '{}' exists", remote_dir);
        if self
            .file_exists(server_metadata, remote_dir, use_sudo)
            .await?
        {
            return Err(format!(
                "Path '{}' exists and is a file, not a directory",
                remote_dir
            ));
        }

        log_debug!("Checking if remote directory '{}' is writable", remote_dir);
        let check_cmd = format!("test -w \"{}\"; echo $?", remote_dir);
        let output = self
            .exec(server_metadata, &check_cmd, use_sudo)
            .await
            .map_err(|e| {
                format!(
                    "Failed to check write permission for '{}'. \n\t{}",
                    remote_dir, e
                )
            })?;
        if output.trim() != "0" {
            return Err(format!("Directory '{}' is not writable", remote_dir));
        }

        Ok(())
    }

    // Creates a remote directory if it doesn't exist.
    // If the directory already exists, no error occurs.
    pub async fn create_remote_dir(
        &self,
        server_metadata: &Arc<ServerMetadata>,
        remote_dir: &str,
        use_sudo: bool,
    ) -> Result<(), String> {
        if self
            .dir_exists(server_metadata, remote_dir, use_sudo)
            .await?
        {
            return Ok(());
        }
        let cmd = if use_sudo {
            format!(
                "mkdir -p \"{}\"; chown {} \"{}\"; chmod 700 \"{}\"",
                remote_dir, server_metadata.user, remote_dir, remote_dir
            )
        } else {
            format!("mkdir -p \"{}\"; chmod 700 \"{}\"", remote_dir, remote_dir)
        };

        self.exec(server_metadata, &cmd, use_sudo)
            .await
            .map_err(|e| {
                format!(
                    "Failed to create remote directory '{}'. \n\t{}",
                    remote_dir, e
                )
            })?;
        Ok(())
    }

    pub async fn create_remote_temp_dir(
        &self,
        server_metadata: &Arc<ServerMetadata>,
        prefix: &str,
        use_sudo: bool,
    ) -> Result<String, String> {
        let temp_dir = generate_remote_temp_dir(prefix);
        log_debug!("Uploading to temporary path '{}' with sudo", temp_dir);
        self.create_remote_dir(server_metadata, temp_dir.as_str(), use_sudo)
            .await?;
        Ok(temp_dir)
    }
}
