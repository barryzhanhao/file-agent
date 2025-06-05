mod api;
mod appstate;
mod sftp;
mod storage;
mod config;

use crate::appstate::appstate::AppState;
use crate::sftp::worker::start_sftp_workers;
use crate::storage::ttl::spawn_ttl_cleaner;
use actix_web::{App, HttpResponse, HttpServer, middleware::Logger, web};
use env_logger::Env;
use std::sync::Arc;
use crate::sftp::client::SftpPool;
use tokio::sync::mpsc;
use crate::config::config::SftpConfig;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();

    let db = Arc::new(sled::open("file-agent").unwrap());

    spawn_ttl_cleaner(db.clone(), 60 * 60 * 12);

    let (tx, rx) = mpsc::channel(1024 * 1024);

    let sftp_config = SftpConfig::from_env();

    let sftp_pool = SftpPool::new(
        num_cpus::get() * 10,
        &sftp_config.username,
        &sftp_config.password,
        &sftp_config.host,
        sftp_config.port,
    ).await;


    start_sftp_workers(rx, num_cpus::get() * 10, db.clone(), sftp_pool).await;

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
    .workers(num_cpus::get() * 4)
    .bind(("127.0.0.1", 8080))?
    .run()
    .await
}
