use crate::storage::models::SftpTask;
use crossbeam::channel::Sender;
use sled::Db;
use std::sync::Arc;

pub struct AppState {
    pub tx: Sender<SftpTask>,
    pub db: Arc<Db>,
}
