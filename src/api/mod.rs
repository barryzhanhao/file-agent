use actix_web::web;

pub mod upload;
pub mod download;
pub mod task;

pub mod request;

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(web::scope("")
        .route("/upload", web::post().to(upload::upload_file_sftp))
        .route("/download", web::post().to(download::download_file_sftp))
        .route("/task/{seq_no}", web::get().to(task::get_task_by_seq)));
}
