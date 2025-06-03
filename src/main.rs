use actix_web::{App, HttpResponse, HttpServer, Responder, web};
use bincode::{Decode, Encode, config};
use crossbeam::channel::{Receiver, Sender, bounded};
use ssh2::Session;
use std::fs::File;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::Path;
use std::sync::Arc;
use std::thread;
use std::time::Duration;
use serde::Serialize;
use uuid::Uuid;

#[derive(Clone, Encode, Decode, PartialEq, Debug, Serialize)]
struct UploadTask {
    seq_no: String,
    remote_path: String,
    local_path: String,
    task_type: String,
    task_status: String,
    detail_log: String,
    #[serde(skip)]
    buffer: Vec<u8>,
}

#[derive(Encode, Decode, PartialEq, Debug)]
struct ValueWithTtl {
    expires_at: u64, // UNIX timestamp (seconds)
    data: UploadTask,
}

fn now_ts() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

fn insert_with_ttl(
    db: &sled::Db,
    key: &str,
    value: UploadTask,
    ttl_secs: u64,
) -> Result<(), Box<dyn std::error::Error>> {
    let val = ValueWithTtl {
        expires_at: now_ts() + ttl_secs,
        data: value,
    };

    let config = config::standard();
    let encoded = bincode::encode_to_vec(&val, config).unwrap();
    db.insert(key, encoded)?;
    Ok(())
}

fn get_if_not_expired(db: &sled::Db, key: &str) -> Option<UploadTask> {
    if let Some(raw) = db.get(key).unwrap() {
        let config = config::standard();
        let (val, _): (ValueWithTtl, usize) = bincode::decode_from_slice(&raw, config).unwrap();
        if now_ts() < val.expires_at {
            Some(val.data)
        } else {
            db.remove(key).ok(); // 清理过期值
            None
        }
    } else {
        None
    }
}

fn spawn_ttl_cleaner(db: Arc<sled::Db>, interval_secs: u64) {
    thread::spawn(move || {
        loop {
            thread::sleep(Duration::from_secs(interval_secs));
            let config = config::standard();

            for item in db.iter() {
                if let Ok((key, val)) = item {
                    let (entry, _): (ValueWithTtl, usize) =
                        bincode::decode_from_slice(&val, config).unwrap();
                    if now_ts() >= entry.expires_at {
                        let _ = db.remove(key);
                    }
                }
            }
        }
    });
}

// 初始化全局上传队列（放在 main 外）
fn start_sftp_workers(rx: Receiver<UploadTask>, worker_count: usize, db: Arc<sled::Db>) {
    for i in 0..worker_count {
        let rx = rx.clone();
        let db = db.clone();
        thread::spawn(move || {
            while let Ok(mut task) = rx.recv() {
                println!("[worker-{i}] Uploading to: {}", task.remote_path);
                if let Err(e) = upload_via_sftp(&task.remote_path, &task.buffer) {
                    eprintln!("[worker-{i}] 上传失败: {}", e);
                    task.task_status = String::from("FAILED");
                    task.detail_log = format!("失败: {}", e);
                    let key = task.seq_no.clone();
                    insert_with_ttl(&db, &key, task, 120);
                } else {
                    println!("[worker-{i}] 上传成功");
                    task.task_status = String::from("SUCCESS");
                    let key = task.seq_no.clone();
                    insert_with_ttl(&db, &key, task, 120);                }
            }
        });
    }
}

fn upload_via_sftp(remote_path: &str, buffer: &[u8]) -> std::io::Result<()> {
    let tcp = TcpStream::connect("localhost:22")?;
    let mut sess = Session::new()?;
    sess.set_tcp_stream(tcp);
    sess.handshake()?;
    sess.userauth_password("foo", "pass")?;
    if !sess.authenticated() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "认证失败",
        ));
    }

    let sftp = sess.sftp()?;
    let mut remote_file = sftp.create(Path::new(remote_path))?;
    remote_file.write_all(buffer)?;
    Ok(())
}

// App 状态，用来共享 Sender
struct AppState {
    tx: Sender<UploadTask>,
    db: Arc<sled::Db>,
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let db = sled::open("mydb").unwrap();
    let db = Arc::new(db);

    // 启动清理线程
    spawn_ttl_cleaner(db.clone(), 10000); // 每10秒清理一次

    let (tx, rx) = bounded::<UploadTask>(1024 * 1024);

    // 启动 worker
    start_sftp_workers(rx, num_cpus::get(), db.clone());

    // 启动 HTTP 服务
    let shared_state = web::Data::new(AppState {
        tx: tx,
        db: db.clone(),
    });
    HttpServer::new(move || {
        App::new()
            .app_data(shared_state.clone())
            .route("/upload", web::get().to(upload_file_sftp))
            .route("/task/{seq_no}", web::get().to(get_task_by_seq))
    })
    .bind(("127.0.0.1", 8080))?
    .run()
    .await
}

async fn upload_file_sftp(state: web::Data<AppState>) -> impl Responder {
    let task_result = web::block(move || {
        // 构建上传任务
        let local_path = Path::new("local.txt");
        let mut file = File::open(local_path)?;
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer)?;

        let seq_no = Uuid::new_v4();

        let task = UploadTask {
            remote_path: "/upload/remote_file.txt".to_string(),
            local_path: local_path.to_str().unwrap().to_string(),
            task_type: String::from("UPLOAD"),
            task_status: String::from("INIT"),
            detail_log: String::from(""),
            seq_no: seq_no.to_string(),
            buffer: buffer,
        };

        // 发送到上传队列
        state.tx.send(task).map_err(|e| {
            std::io::Error::new(std::io::ErrorKind::Other, format!("发送任务失败: {}", e))
        })?;

        Ok::<_, std::io::Error>(seq_no.to_string())
    })
    .await;

    match task_result {
        Ok(Ok(seq_no)) => {
            HttpResponse::Ok()
                .content_type("application/json")
                .body(format!(r#"{{"seq_no": "{}"}}"#, seq_no))
        }
        Ok(Err(e)) => HttpResponse::InternalServerError()
            .content_type("application/json")
            .body(format!(r#"{{"error": "任务提交失败: {}"}}"#, e)),
        Err(e) => HttpResponse::InternalServerError()
            .content_type("application/json")
            .body(format!(r#"{{"error": "线程池错误: {}"}}"#, e)),
    }
}

async fn get_task_by_seq(state: web::Data<AppState>, path: web::Path<String>) -> impl Responder {
    let seq_no = path.into_inner();

    let maybe_task = get_if_not_expired(&state.db, &seq_no);

    match maybe_task {
        Some(task) => HttpResponse::Ok()
            .content_type("application/json")
            .json(task),
        None => HttpResponse::NotFound()
            .content_type("application/json")
            .body(format!(
                r#"{{"error": "Task {} not found or expired"}}"#,
                seq_no
            )),
    }
}
