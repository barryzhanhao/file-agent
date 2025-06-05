use crate::api::domain::request;
use crate::appstate::appstate::AppState;
use crate::storage::models::SftpTask;
use actix_web::{HttpResponse, Responder, web};
use request::SftpFileRequest;
use uuid::Uuid;

pub async fn upload_file_sftp(
    state: web::Data<AppState>,
    req: web::Json<SftpFileRequest>,
) -> impl Responder {
    let req = req.into_inner();
    let seq_no = Uuid::new_v4();
    let task = SftpTask::new_task(
        seq_no.to_string(),
        req.remote_path,
        req.local_path,
        "UPLOAD".to_string(),
    );

    match state.tx.send(task).await {
        Ok(_) => {
            log::info!("任务提交成功: {}", seq_no);
            HttpResponse::Ok().json(serde_json::json!({ "seq_no": &seq_no.to_string() }))
        }
        Err(e) => {
            log::error!("任务提交失败: {}", e);
            HttpResponse::InternalServerError()
                .json(serde_json::json!({ "error": format!("任务提交失败: {}", e) }))
        }
    }
}
