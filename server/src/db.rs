use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use fjall::{Database, Keyspace, KeyspaceCreateOptions};

/* Per-camera persisted configuration. Stored as JSON under camera_id key
   in the "cameras" keyspace. */
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct CameraConfig {
    pub name: Option<String>,
    pub rotation: u16, // 0 / 90 / 180 / 270
    pub mirror: bool,
    pub motion_enabled: bool,
    pub notifications_enabled: bool,
    pub pixel_threshold: u8,
    pub motion_percent: f32,
    pub motion_check_every: u64,
    pub motion_timeout_ms: u64,
}

impl CameraConfig {
    /// Defaults for a never-before-seen camera, seeded from env vars so
    /// existing deployments keep their tuning. Once a row exists in the DB,
    /// these env vars no longer affect that camera.
    pub fn default_from_env() -> Self {
        Self {
            name: None,
            rotation: 0,
            mirror: false,
            motion_enabled: true,
            notifications_enabled: true,
            pixel_threshold: env_or("PIXEL_THRESHOLD", 40),
            motion_percent: env_or("MOTION_PERCENT", 1.0_f32),
            motion_check_every: env_or("MOTION_CHECK_EVERY", 5_u64),
            motion_timeout_ms: env_or("MOTION_TIMEOUT_MS", 60_000_u64),
        }
    }
}

fn env_or<T: std::str::FromStr>(key: &str, default: T) -> T {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

pub struct Db {
    /* Keep the Database handle alive so the on-drop fsync runs on shutdown. */
    _database: Database,
    cameras: Keyspace,
}

impl Db {
    pub fn open(path: impl AsRef<Path>) -> fjall::Result<Arc<Self>> {
        let database = Database::builder(path).open()?;
        let cameras = database.keyspace("cameras", KeyspaceCreateOptions::default)?;
        Ok(Arc::new(Self {
            _database: database,
            cameras,
        }))
    }

    pub fn load_all(&self) -> fjall::Result<HashMap<String, CameraConfig>> {
        let mut out = HashMap::new();
        for guard in self.cameras.iter() {
            let (k, v) = guard.into_inner()?;
            let id = match std::str::from_utf8(&k) {
                Ok(s) => s.to_string(),
                Err(_) => {
                    log::warn!("db: skipping non-utf8 camera key");
                    continue;
                }
            };
            match serde_json::from_slice::<CameraConfig>(&v) {
                Ok(cfg) => {
                    out.insert(id, cfg);
                }
                Err(e) => log::warn!("db: skipping invalid config for {id}: {e}"),
            }
        }
        Ok(out)
    }

    /// Synchronous write-through. Callers in async context should wrap in
    /// `tokio::task::spawn_blocking`. fjall's background writer fsyncs the
    /// journal asynchronously; the Database fsyncs on drop.
    pub fn put(&self, id: &str, cfg: &CameraConfig) -> fjall::Result<()> {
        let bytes = serde_json::to_vec(cfg).expect("CameraConfig is always serializable");
        self.cameras.insert(id.as_bytes(), bytes)?;
        Ok(())
    }

    pub fn delete(&self, id: &str) -> fjall::Result<()> {
        self.cameras.remove(id.as_bytes())?;
        Ok(())
    }
}
