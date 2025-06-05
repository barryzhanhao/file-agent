use std::env;

#[derive(Debug, Clone)]
pub struct SftpConfig {
    pub username: String,
    pub password: String,
    pub host: String,
    pub port: u16,
}

impl SftpConfig {
    pub fn from_env() -> Self {
        dotenvy::dotenv().ok(); // 加载 .env 文件

        Self {
            username: env::var("SFTP_USERNAME").expect("Missing SFTP_USERNAME"),
            password: env::var("SFTP_PASSWORD").expect("Missing SFTP_PASSWORD"),
            host: env::var("SFTP_HOST").expect("Missing SFTP_HOST"),
            port: env::var("SFTP_PORT")
                .expect("Missing SFTP_PORT")
                .parse()
                .expect("SFTP_PORT must be a number"),
        }
    }
}
