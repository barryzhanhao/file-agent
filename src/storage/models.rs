use bincode::{Decode, Encode};
use serde::Serialize;

#[derive(Clone, Encode, Decode, PartialEq, Debug, Serialize)]
pub struct SftpTask {
    pub seq_no: String,
    pub remote_path: String,
    pub local_path: String,
    pub task_type: String,
    pub task_status: String,
    pub detail_log: String,
}

impl SftpTask {
    pub fn new_task(seq_no: String, remote_path: String, local_path: String, task_type: String) -> Self {
        SftpTask {
            seq_no,
            remote_path,
            local_path,
            task_type: task_type,
            task_status: "INIT".into(),
            detail_log: String::new(),
        }
    }
}

#[derive(Encode, Decode, PartialEq, Debug)]
pub struct ValueWithTtl {
    pub expires_at: u64,
    pub data: SftpTask,
}
