use bytes::Bytes;
use serde::Serialize;
use std::{
    collections::HashMap,
    net::IpAddr,
    sync::{atomic::{AtomicU32, AtomicU64}, Arc},
};
use tokio::sync::{broadcast, RwLock};
use utoipa::ToSchema;

use crate::db::{CameraConfig, Db};

pub const BOUNDARY: &str = "frame";
pub const ACTIVE_THRESHOLD_MS: u64 = 10_000;
pub const COMPARE_WIDTH: u32 = 160;
pub const COMPARE_HEIGHT: u32 = 120;

pub struct MotionTracker {
    pub prev_gray: Option<Vec<u8>>,
    pub frame_counter: u64,
    pub saving: bool,
    pub last_motion_ms: u64,
    pub session_dir: Option<String>,
    pub session_cam_dir: Option<String>,
    pub session_start_ms: u64,
    pub timeout_handle: Option<tokio::task::JoinHandle<()>>,
    /* In-flight frame writes for the current session. Drained and awaited at
       finalize time so the encoded video doesn't miss late-landing frames. */
    pub pending_saves: Vec<tokio::task::JoinHandle<()>>,
}

#[derive(Clone)]
pub struct CameraState {
    pub tx: broadcast::Sender<Bytes>,
    pub frame_count: Arc<AtomicU64>,
    pub last_frame_ms: Arc<AtomicU64>,
    pub fps: Arc<AtomicU32>,
    pub viewers: Arc<RwLock<HashMap<u64, IpAddr>>>,
    pub next_viewer_id: Arc<AtomicU64>,
    pub motion: Arc<RwLock<MotionTracker>>,
    pub config: Arc<RwLock<CameraConfig>>,
}

#[derive(Clone)]
pub struct AppState {
    pub cameras: Arc<RwLock<HashMap<String, CameraState>>>,
    pub save_dir: std::path::PathBuf,
    pub telegram_token: Option<String>,
    pub telegram_chat_id: Option<String>,
    pub db: Arc<Db>,
}

pub struct ViewerGuard {
    pub viewers: Arc<RwLock<HashMap<u64, IpAddr>>>,
    pub viewer_id: u64,
}

impl Drop for ViewerGuard {
    fn drop(&mut self) {
        let viewers = self.viewers.clone();
        let id = self.viewer_id;
        tokio::spawn(async move {
            viewers.write().await.remove(&id);
        });
    }
}

#[derive(Serialize, ToSchema)]
pub struct CameraStatus {
    pub id: String,
    pub name: Option<String>,
    pub active: bool,
    pub frame_count: u64,
    pub fps: f32,
    pub viewer_count: usize,
    pub viewers: Vec<String>,
    pub motion_enabled: bool,
    pub notifications_enabled: bool,
    pub rotation: u16,
    pub mirror: bool,
    pub pixel_threshold: u8,
    pub motion_percent: f32,
    pub motion_check_every: u64,
    pub motion_timeout_ms: u64,
}

#[derive(Serialize, ToSchema)]
pub struct StatusResponse {
    pub cameras: Vec<CameraStatus>,
}

impl CameraState {
    pub fn new(config: CameraConfig) -> Self {
        let (tx, _) = broadcast::channel::<Bytes>(4);
        CameraState {
            tx,
            frame_count: Arc::new(AtomicU64::new(0)),
            last_frame_ms: Arc::new(AtomicU64::new(0)),
            fps: Arc::new(AtomicU32::new(0)),
            viewers: Arc::new(RwLock::new(HashMap::new())),
            next_viewer_id: Arc::new(AtomicU64::new(0)),
            motion: Arc::new(RwLock::new(MotionTracker {
                prev_gray: None,
                frame_counter: 0,
                saving: false,
                last_motion_ms: 0,
                session_dir: None,
                session_cam_dir: None,
                session_start_ms: 0,
                timeout_handle: None,
                pending_saves: Vec::new(),
            })),
            config: Arc::new(RwLock::new(config)),
        }
    }
}
