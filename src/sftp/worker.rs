use crate::sftp::client::SftpPool;
use crate::sftp::client::upload_file;
use crate::sftp::client::download_file;

use crate::storage::models::SftpTask;
use crate::storage::ttl::insert_with_ttl;
use std::path::Path;
use std::{sync::Arc};
use tokio::sync::mpsc::Receiver;
use tokio::sync::Mutex;

pub async fn start_sftp_workers(
    rx: Receiver<SftpTask>,
    worker_count: usize,
    db: Arc<sled::Db>,
    sftp_pool: Arc<SftpPool>,
) {
    let rx = Arc::new(Mutex::new(rx));

    for i in 0..worker_count {
        let rx = rx.clone();
        let db = db.clone();
        let sftp_pool = sftp_pool.clone();

        tokio::spawn(async move {
            loop {
                let task_opt = {
                    let mut locked_rx = rx.lock().await;
                    locked_rx.recv().await
                };

                let mut task = match task_opt {
                    Some(task) => task,
                    None => break, // 所有发送端已关闭
                };

                log::info!("[worker-{i}] recv sftp task: {:?}", task);

                let mut session_guard = match sftp_pool.get().await {
                    Some(guard) => guard,
                    None => {
                        task.task_status = "FAILED".into();
                        task.detail_log = "无法获取 SFTP 连接".into();
                        let seq_no = task.seq_no.clone();
                        let _ = insert_with_ttl(&db, &seq_no, task, 60 * 60 * 24 * 7);
                        continue;
                    }
                };

                let session = session_guard.session();

                match task.task_type.as_str() {
                    "UPLOAD" => {
                        let remote_path = Path::new(&task.remote_path);
                        let local_path = Path::new(&task.local_path);

                        if let Err(e) = upload_file(session, remote_path, local_path)
                            .await {
                            task.task_status = "FAILED".into();
                            task.detail_log = format!("上传失败: {}", e);
                            log::info!("[worker-{i}] task:{task:?} upload failed,e:{e}");
                        } else {
                            task.task_status = "SUCCESS".into();
                            log::info!("[worker-{i}] task:{task:?} upload success");
                        }
                    }
                    "DOWNLOAD" => {
                        let remote_path = Path::new(&task.remote_path);
                        let local_path = Path::new(&task.local_path);

                        if let Err(e) = download_file(session, remote_path, local_path)
                            .await {
                            task.task_status = "FAILED".into();
                            task.detail_log = format!("下载失败: {}", e);
                            log::info!("[worker-{i}] task:{task:?} download failed,e:{e}");
                        } else {
                            task.task_status = "SUCCESS".into();
                            log::info!("[worker-{i}] task:{task:?} download success");
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
