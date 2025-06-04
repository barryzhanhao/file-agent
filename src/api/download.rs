use actix_web::{web, HttpResponse, Responder};
use uuid::Uuid;
use request::UploadRequest;
use crate::{appstate::appstate::AppState, storage::models::{SftpTask}};
use crate::api::request;

pub async fn download_file_sftp(
    state: web::Data<AppState>,
    req: web::Json<UploadRequest>,
) -> impl Responder {
    let req = req.into_inner();
    let task_result = web::block(move || {
        let seq_no = Uuid::new_v4();
        let task = SftpTask::new_download(seq_no.to_string(), req.remote_path, req.local_path);
        state.tx.send(task).map_err(|e| {
            std::io::Error::new(std::io::ErrorKind::Other, format!("发送任务失败: {}", e))
        })?;

        Ok::<_, std::io::Error>(seq_no.to_string())
    })
        .await;

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
