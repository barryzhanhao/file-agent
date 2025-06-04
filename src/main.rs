mod api;
mod appstate;
mod sftp;
mod storage;

use crate::appstate::appstate::AppState;
use crate::sftp::worker::start_sftp_workers;
use crate::storage::ttl::spawn_ttl_cleaner;
use actix_web::{App, HttpResponse, HttpServer, middleware::Logger, web};
use crossbeam::channel::bounded;
use env_logger::Env;
use std::sync::Arc;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();

    let db = Arc::new(sled::open("file-agent").unwrap());

    spawn_ttl_cleaner(db.clone(), 60 * 60 * 12);

    let (tx, rx) = bounded(1024);
    start_sftp_workers(rx, num_cpus::get(), db.clone());

    let state = web::Data::new(AppState { tx, db });

    HttpServer::new(move || {
        App::new()
            .app_data(state.clone())
            .configure(api::configure)
            .wrap(Logger::default())
            .default_service(web::route().to(|req: actix_web::HttpRequest| async move {
                log::info!("404 Not Found: {}", req.path());
                HttpResponse::NotFound().body("Not Found")
            }))
    })
    .bind(("127.0.0.1", 8080))?
    .run()
    .await
}
