use crate::api::domain::request;
use actix_web::{HttpResponse, Responder, web};
use request::LocalFileRequest;
use std::fs;
use std::path::Path;

pub async fn delete(req: web::Json<LocalFileRequest>) -> impl Responder {
    let path = Path::new(req.local_path.as_str());

    let is_delete = match fs::remove_file(path) {
        Ok(_) => true,
        Err(e) => {
            log::error!("delete{path:?} failed: {e}!");
            false
        }
    };

    HttpResponse::Ok()
        .content_type("application/json")
        .body(format!(r#"{{"is_delete": "{}"}}"#, is_delete))
}
