use ssh2::Session;
use std::collections::HashMap;
use std::io::{Error, ErrorKind, Read};
use std::net::TcpStream;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::Semaphore;
use tokio::task;

// Connection options
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
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
}

// One SSH session with a channel limit
struct SessionHandle {
    session: Arc<Mutex<Session>>,
    channel_limit: Arc<Semaphore>, // control max concurrent channels
}

impl SessionHandle {
    fn new(session: Session, max_channels: usize) -> Self {
        Self {
            session: Arc::new(Mutex::new(session)),
            channel_limit: Arc::new(Semaphore::new(max_channels)),
        }
    }

    // run one command using a new channel
    async fn execute(&self, command: &str) -> Result<String, Error> {
        let permit = self
            .channel_limit
            .clone()
            .acquire_owned()
            .await
            .map_err(|_| Error::new(ErrorKind::Other, "Semaphore closed"))?;

        let session = Arc::clone(&self.session);
        let cmd = command.to_string();

        let result = task::spawn_blocking(move || {
            let mut session = session.lock().unwrap();
            let mut channel = session.channel_session()?;
            channel.exec(&cmd)?;
            let mut output = String::new();
            channel.read_to_string(&mut output)?;
            channel.wait_close()?;
            let status = channel.exit_status()?;
            if status != 0 {
                return Err(Error::new(
                    ErrorKind::Other,
                    format!("Command `{}` failed with {}", cmd, status),
                ));
            }
            Ok(output)
        })
        .await
        .map_err(|e| Error::new(ErrorKind::Other, e))?;

        drop(permit); // release channel slot
        result
    }
}

// Pool for one server (host+port+user)
struct ServerPool {
    connect_options: ConnectOptions,
    sessions: Mutex<Vec<Arc<SessionHandle>>>,
    max_sessions: usize,
    max_channels_per_session: usize,
}

impl ServerPool {
    fn new(connect_options: ConnectOptions, max_sessions: usize, max_channels_per_session: usize) -> Self {
        Self {
            connect_options,
            sessions: Mutex::new(Vec::new()),
            max_sessions,
            max_channels_per_session,
        }
    }

    // ensure at least one session exists
    async fn get_or_create_session(&self) -> Result<Arc<SessionHandle>, Error> {
        {
            let sessions = self.sessions.lock().unwrap();
            // try to find one session with available channel slots
            for s in sessions.iter() {
                if s.channel_limit.available_permits() > 0 {
                    return Ok(Arc::clone(s));
                }
            }
        }

        // all sessions full, maybe create a new one
        let mut sessions = self.sessions.lock().unwrap();
        if sessions.len() < self.max_sessions {
            let session = self.connect().await?;
            let handle = Arc::new(SessionHandle::new(session, self.max_channels_per_session));
            sessions.push(Arc::clone(&handle));
            return Ok(handle);
        }

        // fallback: just pick the first session (will block on channel semaphore)
        Ok(Arc::clone(&sessions[0]))
    }

    // establish a new SSH session
    async fn connect(&self) -> Result<Session, Error> {
        let addr = format!("{}:{}", self.connect_options.host, self.connect_options.port);
        let timeout = self.connect_options.connect_timeout;
        let stream = tokio::time::timeout(timeout, tokio::net::TcpStream::connect(&addr))
            .await
            .map_err(|_| Error::new(ErrorKind::TimedOut, "Connection timed out"))?
            .map_err(|e| Error::new(ErrorKind::Other, e))?;

        let username = self.connect_options.username.clone();
        let password = self.connect_options.password.clone();

        task::spawn_blocking(move || {
            let std_stream: TcpStream = stream.into_std()?;
            let mut sess = Session::new().map_err(|e| Error::new(ErrorKind::Other, e))?;
            sess.set_tcp_stream(std_stream);
            sess.handshake().map_err(|e| Error::new(ErrorKind::Other, e))?;
            sess.userauth_password(&username, &password)
                .map_err(|e| Error::new(ErrorKind::Other, e))?;
            if !sess.authenticated() {
                return Err(Error::new(ErrorKind::Other, "Authentication failed"));
            }
            Ok(sess)
        })
        .await
        .map_err(|e| Error::new(ErrorKind::Other, e))?
    }

    // execute command on this server
    async fn execute(&self, command: &str) -> Result<String, Error> {
        let session = self.get_or_create_session().await?;
        session.execute(command).await
    }
}

// Global manager for all servers
#[derive(Clone)]
pub struct SessionManager {
    servers: Arc<Mutex<HashMap<(String, u16, String), Arc<ServerPool>>>>,
    max_sessions: usize,
    max_channels_per_session: usize,
}

impl SessionManager {
    pub fn new(max_sessions: usize, max_channels_per_session: usize) -> Self {
        Self {
            servers: Arc::new(Mutex::new(HashMap::new())),
            max_sessions,
            max_channels_per_session,
        }
    }

    // get or create a server pool
    fn get_or_create_pool(&self, opts: ConnectOptions) -> Arc<ServerPool> {
        let key = (opts.host.clone(), opts.port, opts.username.clone());
        let mut servers = self.servers.lock().unwrap();
        servers
            .entry(key)
            .or_insert_with(|| {
                Arc::new(ServerPool::new(
                    opts,
                    self.max_sessions,
                    self.max_channels_per_session,
                ))
            })
            .clone()
    }

    // execute command by (host, user, port)
    pub async fn execute_command(&self, opts: ConnectOptions, command: &str) -> Result<String, Error> {
        let pool = self.get_or_create_pool(opts);
        pool.execute(command).await
    }
}

// Example main
#[tokio::main]
async fn main() -> Result<(), Error> {
    let manager = SessionManager::new(5, 10); // max 5 sessions per server, 10 channels per session

    let opts = ConnectOptions::new("127.0.0.1", 22, "user", "password");

    let mut handles = vec![];
    for i in 0..20 {
        let manager = manager.clone();
        let opts = opts.clone();
        handles.push(tokio::spawn(async move {
            let out = manager.execute_command(opts, "hostname").await.unwrap();
            println!("Task {} output: {}", i, out.trim());
        }));
    }

    for h in handles {
        h.await.unwrap();
    }

    Ok(())
}
