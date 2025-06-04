use serde::Deserialize;

#[derive(Deserialize)]
pub struct SftpFileRequest {
    pub local_path: String,
    pub remote_path: String,
}

#[derive(Deserialize)]
pub struct LocalFileRequest {
    pub local_path: String,
}
