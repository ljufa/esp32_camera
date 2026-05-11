use axum::{
    body::Body,
    extract::{ConnectInfo, Path, State},
    http::{header, HeaderMap, StatusCode},
    response::{Html, Json, Response},
};
use utoipa::ToSchema;
use bytes::{BufMut, Bytes, BytesMut};
use futures::StreamExt;
use log::{debug, info, warn};
use serde::Deserialize;
use std::{
    net::{IpAddr, SocketAddr},
    sync::atomic::Ordering,
};
use tokio_stream::wrappers::BroadcastStream;

use crate::dashboard::DASHBOARD_HTML;
use crate::db::CameraConfig;
use crate::motion::{now_ms, process_frame, render_video_async};
use crate::state::{
    AppState, CameraState, MotionTracker, StatusResponse, ViewerGuard, ACTIVE_THRESHOLD_MS,
    BOUNDARY, COMPARE_HEIGHT, COMPARE_WIDTH,
};
use crate::telegram::{send_telegram_notification, send_telegram_video};

pub async fn handler_index(_: State<AppState>) -> Html<&'static str> {
    Html(DASHBOARD_HTML)
}

#[utoipa::path(
    get,
    path = "/status.json",
    responses(
        (status = 200, description = "List of all cameras with status", body = StatusResponse),
    ),
)]
pub async fn handler_status(State(state): State<AppState>) -> Json<StatusResponse> {
    let cameras = state.cameras.read().await;
    let now = now_ms();

    let mut ids: Vec<String> = cameras.keys().cloned().collect();
    ids.sort();

    let mut camera_statuses = Vec::new();
    for id in ids {
        let cam = &cameras[&id];
        let last_ms = cam.last_frame_ms.load(Ordering::Relaxed);
        let active = last_ms > 0 && now.saturating_sub(last_ms) < ACTIVE_THRESHOLD_MS;
        let viewers = cam.viewers.read().await;
        let mut viewer_ips: Vec<String> = viewers.values().map(|ip| ip.to_string()).collect();
        viewer_ips.sort();
        let cfg = cam.config.read().await;
        let fps = if active { f32::from_bits(cam.fps.load(Ordering::Relaxed)) } else { 0.0 };
        camera_statuses.push(crate::state::CameraStatus {
            id,
            name: cfg.name.clone(),
            active,
            frame_count: cam.frame_count.load(Ordering::Relaxed),
            fps,
            viewer_count: viewers.len(),
            viewers: viewer_ips,
            motion_enabled: cfg.motion_enabled,
            notifications_enabled: cfg.notifications_enabled,
            rotation: cfg.rotation,
            mirror: cfg.mirror,
            pixel_threshold: cfg.pixel_threshold,
            motion_percent: cfg.motion_percent,
            motion_check_every: cfg.motion_check_every,
            motion_timeout_ms: cfg.motion_timeout_ms,
        });
    }

    Json(StatusResponse {
        cameras: camera_statuses,
    })
}

fn sanitize_dir_name(name: &str) -> String {
    let s: String = name
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' || c == '.' { c } else { '_' })
        .collect();
    let s = s.trim_matches('_').to_string();
    if s.is_empty() { "_".to_string() } else { s }
}

pub async fn finalize_motion_session(
    state: &AppState,
    camera: &CameraState,
    cam_dir: &str,
    session: String,
    duration_ms: u64,
) {
    let dir_name = cam_dir.to_string();
    let base_dir = state.save_dir.clone();
    let token = state.telegram_token.clone();
    let chat_id = state.telegram_chat_id.clone();
    let config = camera.config.clone();

    tokio::spawn(async move {
        let (rotation, mirror, notifications_enabled, display_name) = {
            let cfg = config.read().await;
            let dn = cfg.name.clone().unwrap_or_else(|| dir_name.clone());
            (cfg.rotation, cfg.mirror, cfg.notifications_enabled, dn)
        };

        match render_video_async(base_dir, dir_name.clone(), session, duration_ms, rotation, mirror)
            .await
        {
            Ok(video_bytes) => {
                info!("[{display_name}] video created, size: {} bytes", video_bytes.len());
                if notifications_enabled {
                    if let (Some(t), Some(c)) = (token, chat_id) {
                        send_telegram_video(&t, &c, &dir_name, &display_name, video_bytes).await;
                    }
                }
            }
            Err(e) => {
                warn!("[{display_name}] {e}");
            }
        }
    });
}

fn reset_motion_timeout(
    motion: &mut MotionTracker,
    state: &AppState,
    camera_id: &str,
    camera: &CameraState,
) {
    if let Some(handle) = motion.timeout_handle.take() {
        handle.abort();
    }

    let state = state.clone();
    let dir_name = motion.session_cam_dir.clone().unwrap_or_else(|| camera_id.to_string());
    let camera_state = camera.clone();

    let handle = tokio::spawn(async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            let timeout = camera_state.config.read().await.motion_timeout_ms;
            let mut motion = camera_state.motion.write().await;
            if !motion.saving {
                break;
            }
            let elapsed = now_ms().saturating_sub(motion.last_motion_ms);
            if elapsed >= timeout {
                let display_name = {
                    let cfg = camera_state.config.read().await;
                    cfg.name.clone().unwrap_or_else(|| dir_name.clone())
                };
                info!("[{display_name}] motion timed out after {elapsed}ms of inactivity");
                let session = motion.session_dir.take();
                let duration_ms = now_ms().saturating_sub(motion.session_start_ms);
                let pending = std::mem::take(&mut motion.pending_saves);
                motion.saving = false;
                drop(motion);

                for h in pending {
                    let _ = h.await;
                }

                if let Some(session) = session {
                    finalize_motion_session(&state, &camera_state, &dir_name, session, duration_ms)
                        .await;
                }
                break;
            }
        }
    });

    motion.timeout_handle = Some(handle);
}

pub async fn handler_upload(
    State(state): State<AppState>,
    Path(camera_id): Path<String>,
    body: Bytes,
) -> StatusCode {
    if body.is_empty() {
        return StatusCode::BAD_REQUEST;
    }

    let camera = get_or_register_camera(&state, &camera_id).await;

    let now = now_ms();
    let prev_ms = camera.last_frame_ms.load(Ordering::Relaxed);
    camera.last_frame_ms.store(now, Ordering::Relaxed);

    if prev_ms > 0 && now > prev_ms {
        let instant_fps = 1000.0 / (now - prev_ms) as f32;
        let prev_fps = f32::from_bits(camera.fps.load(Ordering::Relaxed));
        let new_fps = if prev_fps == 0.0 { instant_fps } else { 0.2 * instant_fps + 0.8 * prev_fps };
        camera.fps.store(new_fps.to_bits(), Ordering::Relaxed);
    }
    let n = camera.frame_count.fetch_add(1, Ordering::Relaxed) + 1;
    trace!("[{camera_id}] frame #{n}: {} bytes", body.len());

    /* Snapshot config once per frame so a tweak mid-frame doesn't cause an
       inconsistent decision. */
    let cfg = camera.config.read().await.clone();

    /* Brief lock: bump counter and snapshot prev_gray so we can decode
       outside the lock. Skip motion check entirely when disabled. */
    let (do_motion_check, prev_gray) = {
        let mut motion = camera.motion.write().await;
        motion.frame_counter += 1;
        let do_check = cfg.motion_enabled && motion.frame_counter % cfg.motion_check_every == 0;
        let prev = if do_check { motion.prev_gray.clone() } else { None };
        (do_check, prev)
    };

    let detection = if do_motion_check {
        let body_for_decode = body.clone();
        let threshold = cfg.pixel_threshold;
        let had_prev = prev_gray.is_some();
        let min_pixels =
            ((COMPARE_WIDTH * COMPARE_HEIGHT) as f32 * cfg.motion_percent / 100.0) as u32;
        tokio::task::spawn_blocking(move || {
            let gray = process_frame(&body_for_decode)?;
            let changed = match prev_gray {
                Some(prev) => prev
                    .iter()
                    .zip(gray.iter())
                    .filter(|(a, b)| a.abs_diff(**b) > threshold)
                    .count() as u32,
                None => 0,
            };
            let detected = had_prev && changed >= min_pixels;
            Some((gray, changed, detected, min_pixels))
        })
        .await
        .ok()
        .flatten()
    } else {
        None
    };
    if do_motion_check && detection.is_none() {
        debug!("[{camera_id}] failed to decode frame for motion check");
    }

    let mut motion = camera.motion.write().await;

    let mut need_timeout_reset = false;

    if let Some((gray, changed, detected, min_pixels)) = detection {
        motion.prev_gray = Some(gray);

        if detected {
            motion.last_motion_ms = now;
            if !motion.saving {
                let now_dt = chrono::Local::now();
                let date_part = now_dt.format("%d-%m-%Y").to_string();
                let time_part = now_dt.format("%H-%M-%S").to_string();
                let session = format!("{date_part}_{time_part}");
                let display_name = cfg.name.as_deref().unwrap_or(&camera_id);
                let safe_dir = sanitize_dir_name(display_name);
                let cam_dir = state.save_dir.join(&safe_dir).join(&date_part).join(".raw").join(&time_part);
                if let Err(e) = tokio::fs::create_dir_all(&cam_dir).await {
                    warn!(
                        "[{camera_id}] failed to create session dir {}: {e}",
                        cam_dir.display()
                    );
                } else {
                    info!(
                        "[{display_name}] motion START ({} changed pixels, threshold {}) session={}",
                        changed, min_pixels, session
                    );
                    motion.saving = true;
                    motion.session_dir = Some(session);
                    motion.session_cam_dir = Some(safe_dir);
                    motion.session_start_ms = now;

                    if cfg.notifications_enabled {
                        if let (Some(token), Some(chat_id)) =
                            (state.telegram_token.clone(), state.telegram_chat_id.clone())
                        {
                            let frame = body.clone();
                            let cam = camera_id.clone();
                            let dn = display_name.to_string();
                            tokio::spawn(async move {
                                send_telegram_notification(&token, &chat_id, &cam, &dn, frame).await;
                            });
                        }
                    }
                }
            } else {
                debug!(
                    "[{camera_id}] motion continuing ({} changed pixels)",
                    changed
                );
            }
            need_timeout_reset = true;
        } else {
            trace!(
                "[{camera_id}] no motion ({} changed pixels, threshold {})",
                changed,
                min_pixels
            );
        }
    }

    if need_timeout_reset {
        reset_motion_timeout(&mut motion, &state, &camera_id, &camera);
    }

    if let Some(ref session) = motion.session_dir {
        let dir = motion.session_cam_dir.as_deref().unwrap_or(&camera_id);
        let (date_part, time_part) = session.split_once('_').unwrap_or((session.as_str(), "00-00-00"));
        let cam_dir = state.save_dir.join(dir).join(date_part).join(".raw").join(time_part);
        let path = cam_dir.join(format!("frame_{:06}.jpg", n));
        let data = body.clone();
        let cam_id = camera_id.clone();
        let handle = tokio::spawn(async move {
            if let Err(e) = tokio::fs::write(&path, &data).await {
                warn!("[{cam_id}] failed to save frame #{n}: {e}");
            } else {
                debug!("[{cam_id}] saved frame #{n} to {}", path.display());
            }
        });
        if motion.pending_saves.len() >= 64 {
            motion.pending_saves.retain(|h| !h.is_finished());
        }
        motion.pending_saves.push(handle);
    }
    drop(motion);

    let _ = camera.tx.send(body);
    StatusCode::OK
}

async fn get_or_register_camera(state: &AppState, camera_id: &str) -> CameraState {
    {
        let cameras = state.cameras.read().await;
        if let Some(c) = cameras.get(camera_id) {
            return c.clone();
        }
    }

    let mut cameras = state.cameras.write().await;
    if let Some(c) = cameras.get(camera_id) {
        return c.clone();
    }

    info!("New camera registered: {camera_id}");
    let cfg = CameraConfig::default_from_env();
    /* Persist the seeded config in the background so we don't block the
       upload on a DB write. The in-memory state is correct immediately. */
    let db = state.db.clone();
    let id_for_db = camera_id.to_string();
    let cfg_for_db = cfg.clone();
    tokio::task::spawn_blocking(move || {
        if let Err(e) = db.put(&id_for_db, &cfg_for_db) {
            warn!("[{id_for_db}] failed to persist initial config: {e}");
        }
    });
    let cs = CameraState::new(cfg);
    cameras.insert(camera_id.to_string(), cs.clone());
    cs
}

pub async fn handler_stream(
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    State(state): State<AppState>,
    Path(camera_id): Path<String>,
) -> Response {
    let camera = {
        let cameras = state.cameras.read().await;
        cameras.get(&camera_id).cloned()
    };

    let Some(camera) = camera else {
        return Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Body::empty())
            .unwrap();
    };

    let client_ip = headers
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.split(',').next())
        .and_then(|s| s.trim().parse::<IpAddr>().ok())
        .unwrap_or_else(|| addr.ip());

    let viewer_id = camera.next_viewer_id.fetch_add(1, Ordering::Relaxed);
    camera.viewers.write().await.insert(viewer_id, client_ip);

    let guard = ViewerGuard {
        viewers: camera.viewers.clone(),
        viewer_id,
    };

    let rx = camera.tx.subscribe();
    let inner = Box::pin(BroadcastStream::new(rx).filter_map(|result| async move {
        let frame = result.ok()?;
        let mut buf = BytesMut::new();
        buf.put(
            format!(
                "--{BOUNDARY}\r\nContent-Type: image/jpeg\r\nContent-Length: {}\r\n\r\n",
                frame.len()
            )
            .as_bytes(),
        );
        buf.put(frame.as_ref());
        buf.put(&b"\r\n"[..]);
        Some(Ok::<_, std::convert::Infallible>(buf.freeze()))
    }));

    let stream = futures::stream::unfold((inner, guard), |(mut s, g)| async move {
        match tokio::time::timeout(std::time::Duration::from_secs(30), s.next()).await {
            Ok(Some(item)) => Some((item, (s, g))),
            _ => None,
        }
    });

    Response::builder()
        .header(
            header::CONTENT_TYPE,
            format!("multipart/x-mixed-replace; boundary={BOUNDARY}"),
        )
        .header(header::CACHE_CONTROL, "no-cache")
        .body(Body::from_stream(stream))
        .unwrap()
}

#[derive(Deserialize, ToSchema)]
pub struct ConfigUpdate {
    pub name: Option<String>,
    pub rotation: Option<u16>,
    pub mirror: Option<bool>,
    pub motion_enabled: Option<bool>,
    pub notifications_enabled: Option<bool>,
    pub pixel_threshold: Option<u8>,
    pub motion_percent: Option<f32>,
    pub motion_check_every: Option<u64>,
    pub motion_timeout_ms: Option<u64>,
}

#[utoipa::path(
    patch,
    path = "/api/camera/{camera_id}/config",
    params(
        ("camera_id" = String, Path, description = "Camera identifier"),
    ),
    request_body = ConfigUpdate,
    responses(
        (status = 200, description = "Config updated successfully"),
        (status = 404, description = "Camera not found"),
        (status = 409, description = "Name already taken by another camera"),
    ),
)]
pub async fn handler_update_config(
    State(state): State<AppState>,
    Path(camera_id): Path<String>,
    Json(update): Json<ConfigUpdate>,
) -> StatusCode {
    let cameras = state.cameras.read().await;
    let Some(camera) = cameras.get(&camera_id) else {
        return StatusCode::NOT_FOUND;
    };

    if let Some(ref new_name) = update.name {
        let new_name = new_name.trim();
        if !new_name.is_empty() {
            for (cid, cam) in cameras.iter() {
                if cid == &camera_id { continue; }
                let cfg = cam.config.read().await;
                if cfg.name.as_deref() == Some(new_name) {
                    return StatusCode::CONFLICT;
                }
            }
        }
    }

    let mut cfg = camera.config.write().await;
    if let Some(name) = update.name {
        let name = name.trim().to_string();
        cfg.name = if name.is_empty() { None } else { Some(name) };
    }
    if let Some(rotation) = update.rotation {
        if [0, 90, 180, 270].contains(&rotation) {
            cfg.rotation = rotation;
        }
    }
    if let Some(mirror) = update.mirror {
        cfg.mirror = mirror;
    }
    if let Some(motion_enabled) = update.motion_enabled {
        cfg.motion_enabled = motion_enabled;
    }
    if let Some(notifications_enabled) = update.notifications_enabled {
        cfg.notifications_enabled = notifications_enabled;
    }
    if let Some(pixel_threshold) = update.pixel_threshold {
        cfg.pixel_threshold = pixel_threshold;
    }
    if let Some(motion_percent) = update.motion_percent {
        cfg.motion_percent = motion_percent;
    }
    if let Some(motion_check_every) = update.motion_check_every {
        cfg.motion_check_every = motion_check_every;
    }
    if let Some(motion_timeout_ms) = update.motion_timeout_ms {
        cfg.motion_timeout_ms = motion_timeout_ms;
    }
    let cloned = cfg.clone();
    drop(cfg);
    drop(cameras);

    let db = state.db.clone();
    let id = camera_id.clone();
    tokio::task::spawn_blocking(move || {
        if let Err(e) = db.put(&id, &cloned) {
            warn!("[{id}] failed to persist config: {e}");
        }
    });

    StatusCode::OK
}

#[utoipa::path(
    delete,
    path = "/api/camera/{camera_id}",
    params(
        ("camera_id" = String, Path, description = "Camera identifier"),
    ),
    responses(
        (status = 200, description = "Camera deleted"),
        (status = 404, description = "Camera not found"),
    ),
)]
pub async fn handler_delete_camera(
    State(state): State<AppState>,
    Path(camera_id): Path<String>,
) -> StatusCode {
    let removed = {
        let mut cameras = state.cameras.write().await;
        cameras.remove(&camera_id)
    };
    if removed.is_none() {
        return StatusCode::NOT_FOUND;
    }

    info!("[{camera_id}] deleted");

    let db = state.db.clone();
    let id = camera_id.clone();
    tokio::task::spawn_blocking(move || {
        if let Err(e) = db.delete(&id) {
            warn!("[{id}] failed to delete config: {e}");
        }
    });

    StatusCode::OK
}
