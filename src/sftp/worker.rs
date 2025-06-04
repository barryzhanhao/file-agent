use crossbeam::channel::Receiver;
use std::{sync::Arc, thread};
use crate::storage::models::SftpTask;
use crate::storage::ttl::insert_with_ttl;
use crate::sftp::client::{upload, download};

pub fn start_sftp_workers(rx: Receiver<SftpTask>, worker_count: usize, db: Arc<sled::Db>) {
    for i in 0..worker_count {
        let rx = rx.clone();
        let db = db.clone();
        thread::spawn(move || {
            while let Ok(mut task) = rx.recv() {
                log::info!("[worker-{i}] recv sftp task: {:?}", task);

                match task.task_type.as_str() {
                    "UPLOAD" => {
                        if let Err(e) = upload(&task.remote_path, &task.buffer) {
                            task.task_status = "FAILED".into();
                            task.detail_log = format!("失败: {}", e);
                        } else {
                            task.task_status = "SUCCESS".into();
                        }
                    }
                    "DOWNLOAD" => {
                        match download(&task.remote_path) {
                            Ok(buf) => {
                                if let Err(e) = std::fs::write(&task.local_path, &buf) {
                                    task.task_status = "FAILED".into();
                                    task.detail_log = format!("写入失败: {}", e);
                                } else {
                                    task.task_status = "SUCCESS".into();
                                }
                            }
                            Err(e) => {
                                task.task_status = "FAILED".into();
                                task.detail_log = format!("下载失败: {}", e);
                            }
                        }
                    }
                    _ => {}
                }

                let seq_no = task.seq_no.clone();
                let _ = insert_with_ttl(&db, &seq_no, task, 60 * 60 * 24 * 7);
            }
        });
    }
}
