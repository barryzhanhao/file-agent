use async_ssh2_lite::{AsyncSession, SessionConfiguration, TokioTcpStream};
use futures_util::AsyncReadExt;
use futures_util::AsyncWriteExt;
use futures_util::future::join_all;
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
    let mut config = SessionConfiguration::default();
    config.set_compress(true);

    // 设置 SSH 协议版本，尝试兼容更多服务器
    config.set_banner("SSH-2.0-OpenSSH_8.0");

    let mut session = AsyncSession::new(stream, config)?;
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
        log::info!(
            "Initializing SFTP pool with {} connections to {}:{}",
            size,
            host,
            port
        );

        // 创建所有连接的 futures，带重试
        let connection_futures: Vec<_> = (0..size)
            .map(|i| {
                let username = username.to_string();
                let password = password.to_string();
                let host = host.to_string();
                async move {
                    // 重试 10 次
                    for attempt in 1..=10 {
                        match connect_ssh(&username, &password, &host, port).await {
                            Ok(session) => {
                                log::debug!("Connection {} established on attempt {}", i, attempt);
                                return Ok(session);
                            }
                            Err(e) => {
                                log::warn!("Connection {} failed on attempt {}: {}", i, attempt, e);
                                if attempt == 3 {
                                    return Err(e);
                                }
                                // 等待一段时间再重试
                                tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
                            }
                        }
                    }
                    unreachable!()
                }
            })
            .collect();

        // 并发执行所有连接
        let results = join_all(connection_futures).await;

        let mut conns = VecDeque::new();
        let mut success_count = 0;
        let mut failure_count = 0;

        for (i, result) in results.into_iter().enumerate() {
            match result {
                Ok(session) => {
                    conns.push_back(session);
                    success_count += 1;
                }
                Err(e) => {
                    failure_count += 1;
                    log::error!("Failed to create SFTP connection {}: {}", i, e);
                }
            }
        }

        log::info!(
            "SFTP pool initialized: {} successful, {} failed connections",
            success_count,
            failure_count
        );

        if success_count == 0 {
            log::error!("No SFTP connections could be established!");
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
