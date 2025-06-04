use actix_web::{web, HttpResponse, Responder};
use crate::{appstate::appstate::AppState, storage::ttl::get_if_not_expired};

pub async fn get_task_by_seq(state: web::Data<AppState>, path: web::Path<String>) -> impl Responder {
    let seq_no = path.into_inner();
    let maybe_task = get_if_not_expired(&state.db, &seq_no);

    match maybe_task {
        Some(task) => HttpResponse::Ok().json(task),
        None => HttpResponse::NotFound().json(serde_json::json!({"error": format!("Task {} not found or expired", seq_no)})),
    }
}
