use ssh2::Session;
use std::net::TcpStream;
use std::path::Path;
use std::io::{Read, Write};

pub fn upload(remote_path: &str, buffer: &[u8]) -> std::io::Result<()> {
    let tcp = TcpStream::connect("localhost:22")?;
    let mut sess = Session::new()?;
    sess.set_tcp_stream(tcp);
    sess.handshake()?;
    sess.userauth_password("foo", "pass")?;
    if !sess.authenticated() {
        return Err(std::io::Error::new(std::io::ErrorKind::PermissionDenied, "认证失败"));
    }
    let sftp = sess.sftp()?;
    let mut remote_file = sftp.create(Path::new(remote_path))?;
    remote_file.write_all(buffer)?;
    Ok(())
}

pub fn download(remote_path: &str) -> std::io::Result<Vec<u8>> {
    let tcp = TcpStream::connect("localhost:22")?;
    let mut sess = Session::new()?;
    sess.set_tcp_stream(tcp);
    sess.handshake()?;
    sess.userauth_password("foo", "pass")?;
    if !sess.authenticated() {
        return Err(std::io::Error::new(std::io::ErrorKind::PermissionDenied, "认证失败"));
    }
    let sftp = sess.sftp()?;
    let mut remote_file = sftp.open(Path::new(remote_path))?;
    let mut buffer = Vec::new();
    remote_file.read_to_end(&mut buffer)?;
    Ok(buffer)
}
