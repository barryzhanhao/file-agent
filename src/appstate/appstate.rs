use crate::storage::models::SftpTask;
use tokio::sync::mpsc::{Sender};
use sled::Db;
use std::sync::Arc;

pub struct AppState {
    pub tx: Sender<SftpTask>,
    pub db: Arc<Db>,
}
