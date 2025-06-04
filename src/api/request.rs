use serde::Deserialize;

#[derive(Deserialize)]
pub struct UploadRequest {
    pub local_path: String,
    pub remote_path: String,
}
