use std::{sync::Arc, thread, time::Duration};
use sled::Db;
use crate::storage::models::{ValueWithTtl, SftpTask};
use bincode::config;

pub fn now_ts() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs()
}

pub fn insert_with_ttl(db: &Db, key: &str, value: SftpTask, ttl_secs: u64) -> sled::Result<()> {
    let val = ValueWithTtl {
        expires_at: now_ts() + ttl_secs,
        data: value,
    };
    let encoded = bincode::encode_to_vec(&val, config::standard()).unwrap();
    db.insert(key, encoded)?;
    Ok(())
}

pub fn get_if_not_expired(db: &Db, key: &str) -> Option<SftpTask> {
    if let Some(raw) = db.get(key).ok()? {
        let (val, _): (ValueWithTtl, usize) = bincode::decode_from_slice(&raw, config::standard()).ok()?;
        if now_ts() < val.expires_at {
            Some(val.data)
        } else {
            db.remove(key).ok();
            None
        }
    } else {
        None
    }
}

pub fn spawn_ttl_cleaner(db: Arc<Db>, interval_secs: u64) {
    thread::spawn(move || {
        loop {
            thread::sleep(Duration::from_secs(interval_secs));
            for item in db.iter() {
                if let Ok((key, val)) = item {
                    let (entry, _): (ValueWithTtl, usize) =
                        bincode::decode_from_slice(&val, config::standard()).unwrap();
                    if now_ts() >= entry.expires_at {
                        let _ = db.remove(key);
                    }
                }
            }
        }
    });
}
