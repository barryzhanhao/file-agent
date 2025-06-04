use crate::api::domain::request;
use actix_web::{HttpResponse, Responder, web};
use request::LocalFileRequest;
use std::path::Path;

pub async fn exist(req: web::Json<LocalFileRequest>) -> impl Responder {
    let path = Path::new(req.local_path.as_str());
    let mut is_exist = true;

    if !path.exists() {
        is_exist = false;
    }

    HttpResponse::Ok()
        .content_type("application/json")
        .body(format!(r#"{{"is_exist": "{}"}}"#, is_exist))
}
