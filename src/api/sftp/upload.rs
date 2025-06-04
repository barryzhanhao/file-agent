use actix_web::{web, HttpResponse, Responder};
use std::{fs::File, io::Read, path::Path};
use uuid::Uuid;
use request::SftpFileRequest;
use crate::appstate::appstate::AppState;
use crate::storage::models::SftpTask;
use crate::api::domain::request;

pub async fn upload_file_sftp(
    state: web::Data<AppState>,
    req: web::Json<SftpFileRequest>,
) -> impl Responder {
    let req = req.into_inner();
    let task_result = web::block(move || {
        let local_path = Path::new(&req.local_path);
        let mut file = File::open(local_path)?;
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer)?;

        let seq_no = Uuid::new_v4();
        let task = SftpTask::new_upload(seq_no.to_string(), req.remote_path, req.local_path, buffer);
        state.tx.send(task).map_err(|e| {
            std::io::Error::new(std::io::ErrorKind::Other, format!("发送任务失败: {}", e))
        })?;

        Ok::<_, std::io::Error>(seq_no.to_string())
    }).await;

    match task_result {
        Ok(Ok(seq_no)) => HttpResponse::Ok()
            .content_type("application/json")
            .body(format!(r#"{{"seq_no": "{}"}}"#, seq_no)),
        Ok(Err(e)) => HttpResponse::InternalServerError()
            .content_type("application/json")
            .body(format!(r#"{{"error": "任务提交失败: {}"}}"#, e)),
        Err(e) => HttpResponse::InternalServerError()
            .content_type("application/json")
            .body(format!(r#"{{"error": "线程池错误: {}"}}"#, e)),
    }
}
