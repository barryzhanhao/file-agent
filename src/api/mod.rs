use actix_web::web;
pub mod sftp;

pub mod domain;

pub mod local;

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(web::scope("")
        .route("/sftp/upload", web::post().to(sftp::upload::upload_file_sftp))
        .route("/sftp/download", web::post().to(sftp::download::download_file_sftp))
        .route("/sftp/task/{seq_no}", web::get().to(sftp::task::get_task_by_seq))
        .route("/local/exist",web::post().to(local::exist::exist))
        .route("/local/delete",web::post().to(local::delete::delete)));
}
