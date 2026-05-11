use bytes::Bytes;
use serde_json::json;

use crate::db::CameraConfig;
use crate::state::{AppState, ACTIVE_THRESHOLD_MS};
use crate::motion::now_ms;

pub async fn send_telegram_notification(
    token: &str,
    chat_id: &str,
    camera_id: &str,
    display_name: &str,
    frame: Bytes,
) {
    let client = reqwest::Client::new();

    let caption = format!("\u{26a0}\u{fe0f} <b>Motion on {display_name}</b>");

    let photo_part = reqwest::multipart::Part::bytes(frame.to_vec())
        .file_name("motion.jpg")
        .mime_str("image/jpeg")
        .unwrap();
    let form = reqwest::multipart::Form::new()
        .part("photo", photo_part)
        .text("chat_id", chat_id.to_owned())
        .text("caption", caption)
        .text("parse_mode", "HTML");

    if let Err(e) = client
        .post(format!("https://api.telegram.org/bot{token}/sendPhoto"))
        .multipart(form)
        .send()
        .await
    {
        log::warn!("[{camera_id}] telegram sendPhoto failed: {e}");
    }
}

pub async fn send_telegram_video(
    token: &str,
    chat_id: &str,
    camera_id: &str,
    display_name: &str,
    video_bytes: Vec<u8>,
) {
    let client = reqwest::Client::new();
    let video_part = reqwest::multipart::Part::bytes(video_bytes)
        .file_name("motion.mp4")
        .mime_str("video/mp4")
        .unwrap();
    let form = reqwest::multipart::Form::new()
        .part("video", video_part)
        .text("chat_id", chat_id.to_owned())
        .text("caption", format!("\u{1f6a8} Motion ended on {}", display_name))
        .text("supports_streaming", "true");

    if let Err(e) = client
        .post(format!("https://api.telegram.org/bot{token}/sendVideo"))
        .multipart(form)
        .send()
        .await
    {
        log::warn!("[{camera_id}] telegram sendVideo failed: {e}");
    }
}

async fn send_reply(token: &str, chat_id: &str, text: &str) {
    let client = reqwest::Client::new();
    let _ = client
        .post(format!("https://api.telegram.org/bot{token}/sendMessage"))
        .json(&json!({
            "chat_id": chat_id,
            "text": text,
            "parse_mode": "HTML",
        }))
        .send()
        .await;
}

async fn dispatch(state: &AppState, text: &str) -> Option<String> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return None;
    }
    let mut parts = trimmed.split_whitespace();
    let cmd = parts.next()?;

    match cmd {
        "/help" => Some(
            concat!(
                "<b>Commands</b>\n",
                "/help — this help\n",
                "/list — list cameras\n",
                "/motion_on &lt;id&gt; / /motion_off &lt;id&gt; — motion detection toggle\n",
                "/notify_on &lt;id&gt; / /notify_off &lt;id&gt; — notifications toggle\n\n",
                "Other settings (name, rotation, mirror, thresholds) are available in the web dashboard."
            ).to_string(),
        ),

        "/list" => {
            let cameras = state.cameras.read().await;
            if cameras.is_empty() {
                return Some("No cameras registered.".to_string());
            }
            let now = now_ms();
            let mut lines: Vec<String> = Vec::new();
            for (id, cam) in cameras.iter() {
                let cfg = cam.config.read().await;
                let name = cfg.name.as_deref().unwrap_or("-");
                let last_ms = cam.last_frame_ms.load(std::sync::atomic::Ordering::Relaxed);
                let online = last_ms > 0 && now.saturating_sub(last_ms) < ACTIVE_THRESHOLD_MS;
                let status = if online { "online" } else { "offline" };
                let motion = if cfg.motion_enabled { "on" } else { "off" };
                let notify = if cfg.notifications_enabled { "on" } else { "off" };
                lines.push(format!("<code>{id}</code>  {name}  {status}  motion={motion}  notify={notify}"));
            }
            lines.sort();
            lines.insert(0, "<b>Cameras</b>".to_string());
            Some(lines.join("\n"))
        }

        "/notify_on" => {
            let id = parts.next()?;
            mutate_config(state, id, |cfg| cfg.notifications_enabled = true).await;
            Some(format!("Notifications enabled for <code>{id}</code>"))
        }

        "/notify_off" => {
            let id = parts.next()?;
            mutate_config(state, id, |cfg| cfg.notifications_enabled = false).await;
            Some(format!("Notifications disabled for <code>{id}</code>"))
        }

        "/motion_on" => {
            let id = parts.next()?;
            mutate_config(state, id, |cfg| cfg.motion_enabled = true).await;
            Some(format!("Motion detection enabled for <code>{id}</code>"))
        }

        "/motion_off" => {
            let id = parts.next()?;
            mutate_config(state, id, |cfg| cfg.motion_enabled = false).await;
            Some(format!("Motion detection disabled for <code>{id}</code>"))
        }

        _ => Some(format!("Unknown command: {cmd}. Try /help")),
    }
}

async fn mutate_config<F: FnOnce(&mut CameraConfig)>(state: &AppState, id: &str, f: F) {
    let cameras = state.cameras.read().await;
    if let Some(cam) = cameras.get(id) {
        let mut cfg = cam.config.write().await;
        f(&mut cfg);
        let cloned = cfg.clone();
        drop(cfg);
        drop(cameras);
        let db = state.db.clone();
        let id = id.to_string();
        tokio::task::spawn_blocking(move || {
            if let Err(e) = db.put(&id, &cloned) {
                log::warn!("[{id}] failed to persist config: {e}");
            }
        });
    }
}

pub async fn poll_telegram_commands(state: AppState) {
    let token = match &state.telegram_token {
        Some(t) => t.clone(),
        None => return,
    };
    let chat_id = match &state.telegram_chat_id {
        Some(c) => c.clone(),
        None => return,
    };
    let chat_id_i64: i64 = match chat_id.parse() {
        Ok(id) => id,
        Err(_) => {
            log::warn!("Invalid TELEGRAM_CHAT_ID, command polling disabled");
            return;
        }
    };
    let client = reqwest::Client::new();
    let mut interval = tokio::time::interval(std::time::Duration::from_secs(15));
    let mut next_offset: i64 = -1;

    loop {
        interval.tick().await;

        let resp = client
            .post(format!("https://api.telegram.org/bot{token}/getUpdates"))
            .json(&json!({
                "limit": 100,
                "offset": next_offset,
                "allowed_updates": ["message"],
            }))
            .send()
            .await;

        let updates = match resp {
            Ok(r) => match r.json::<serde_json::Value>().await {
                Ok(data) => data["result"].as_array().cloned().unwrap_or_default(),
                Err(_) => continue,
            },
            Err(_) => continue,
        };

        for update in &updates {
            if let Some(id) = update["update_id"].as_i64() {
                if id >= next_offset {
                    next_offset = id + 1;
                }
            }
            let chat = update["message"]["chat"]["id"].as_i64();
            if chat != Some(chat_id_i64) {
                continue;
            }
            let text = match update["message"]["text"].as_str() {
                Some(t) => t,
                None => continue,
            };

            if let Some(reply) = dispatch(&state, text).await {
                send_reply(&token, &chat_id, &reply).await;
            }
        }
    }
}
