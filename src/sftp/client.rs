use async_ssh2_lite::{AsyncSession, SessionConfiguration, TokioTcpStream};
use futures_util::AsyncReadExt;
use futures_util::AsyncWriteExt;
use ssh2::OpenFlags;
use ssh2::OpenType;
use std::collections::VecDeque;
use std::net::ToSocketAddrs;
use std::path::Path;
use std::sync::Arc;
use tokio::fs::File;
use tokio::io::{AsyncReadExt as TokioAsyncReadExt, AsyncWriteExt as TokioAsyncWriteExt};
use tokio::sync::{Mutex, Semaphore};

async fn connect_ssh(
    username: &str,
    password: &str,
    host: &str,
    port: u16,
) -> Result<AsyncSession<tokio::net::TcpStream>, Box<dyn std::error::Error>> {
    let addr = (host, port)
        .to_socket_addrs()?
        .next()
        .ok_or("Invalid address")?;
    let stream = tokio::net::TcpStream::connect(addr).await?;
    let mut session = AsyncSession::new(stream, SessionConfiguration::default())?;
    session.handshake().await?;
    session.userauth_password(username, password).await?;
    if !session.authenticated() {
        return Err("Authentication failed".into());
    }
    Ok(session)
}

pub async fn upload_file(
    session: &AsyncSession<TokioTcpStream>,
    remote_path: &Path,
    local_path: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let sftp = session.sftp().await?;
    let mut local_file = File::open(local_path).await?;
    let mut remote_file = sftp
        .open_mode(
            remote_path,
            OpenFlags::CREATE | OpenFlags::WRITE | OpenFlags::TRUNCATE,
            0o644,
            OpenType::File,
        )
        .await?;

    let mut buffer = [0u8; 256 * 1024]; // 256KB
    loop {
        let n = local_file.read(&mut buffer).await?;
        if n == 0 {
            break;
        }
        let mut written = 0;
        while written < n {
            let nwritten = remote_file.write(&buffer[written..n]).await?;
            if nwritten == 0 {
                return Err("Remote write returned 0 bytes".into());
            }
            written += nwritten;
        }
    }

    remote_file.close().await?;
    Ok(())
}

pub async fn download_file(
    session: &AsyncSession<TokioTcpStream>,
    remote_path: &Path,
    local_path: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let sftp = session.sftp().await?;
    let mut remote_file = sftp.open(remote_path).await?;
    let mut local_file = File::create(local_path).await?;

    let mut buffer = [0u8; 256 * 1024]; // 256KB 缓冲区

    loop {
        let n = remote_file.read(&mut buffer).await?;
        if n == 0 {
            break;
        }

        let mut written = 0;
        while written < n {
            let nwritten = local_file.write(&buffer[written..n]).await?;
            if nwritten == 0 {
                return Err("Local write returned 0 bytes".into());
            }
            written += nwritten;
        }
    }

    Ok(())
}


pub struct SftpPool {
    connections: Mutex<VecDeque<AsyncSession<tokio::net::TcpStream>>>,
    semaphore: Semaphore,
}

impl SftpPool {
    pub async fn new(
        size: usize,
        username: &str,
        password: &str,
        host: &str,
        port: u16,
    ) -> Arc<Self> {
        let mut conns = VecDeque::new();
        for _ in 0..size {
            let session = connect_ssh(username, password, host, port).await.unwrap(); // 上文中的 connect_ssh
            conns.push_back(session);
        }

        Arc::new(Self {
            connections: Mutex::new(conns),
            semaphore: Semaphore::new(size),
        })
    }

    pub async fn get(self: &Arc<Self>) -> Option<SftpSessionGuard<'_>> {
        let permit = self.semaphore.acquire().await.ok()?;
        let session = {
            let mut conns = self.connections.lock().await;
            conns.pop_front()
        };

        session.map(|s| SftpSessionGuard {
            session: Some(s),
            pool: self.clone(),
            _permit: permit,
        })
    }
}

pub struct SftpSessionGuard<'a> {
    session: Option<AsyncSession<tokio::net::TcpStream>>,
    pool: Arc<SftpPool>,
    _permit: tokio::sync::SemaphorePermit<'a>,
}

impl<'a> SftpSessionGuard<'a> {
    pub fn session(&mut self) -> &mut AsyncSession<tokio::net::TcpStream> {
        self.session.as_mut().unwrap()
    }
}

impl<'a> Drop for SftpSessionGuard<'a> {
    fn drop(&mut self) {
        if let Some(session) = self.session.take() {
            let pool = self.pool.clone();
            tokio::spawn(async move {
                let mut conns = pool.connections.lock().await;
                conns.push_back(session);
            });
        }
    }
}
