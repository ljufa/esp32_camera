use crate::state::{COMPARE_HEIGHT, COMPARE_WIDTH};

pub fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}

pub fn process_frame(data: &[u8]) -> Option<Vec<u8>> {
    let img = image::load_from_memory(data).ok()?;
    let gray = img.grayscale();
    let small = image::imageops::resize(
        &gray,
        COMPARE_WIDTH,
        COMPARE_HEIGHT,
        image::imageops::FilterType::Nearest,
    );
    Some(small.into_raw())
}

fn build_vf_filter(rotation: u16, mirror: bool) -> Option<String> {
    let mut filters = Vec::new();
    match rotation {
        90 => filters.push("transpose=1".to_string()),
        180 => filters.push("transpose=2,transpose=2".to_string()),
        270 => filters.push("transpose=2".to_string()),
        _ => {}
    }
    if mirror {
        filters.push("hflip".to_string());
    }
    if filters.is_empty() {
        None
    } else {
        Some(filters.join(","))
    }
}

pub fn render_video(
    base_dir: &std::path::Path,
    camera_id: &str,
    session: &str,
    duration_ms: u64,
    rotation: u16,
    mirror: bool,
) -> Result<Vec<u8>, String> {
    let (date_part, time_part) = session.split_once('_').unwrap_or((session, "00-00-00"));
    let frames_dir = base_dir.join(camera_id).join(date_part).join(".raw").join(time_part);
    let video_path = base_dir.join(camera_id).join(date_part).join(format!("{time_part}.mp4"));

    let frames_glob = frames_dir.join("frame_*.jpg");
    let frames_glob = frames_glob.to_string_lossy().to_string();

    let frame_count = std::fs::read_dir(&frames_dir)
        .map_err(|e| format!("failed to read session dir: {e}"))?
        .filter(|e| e.as_ref().map_or(false, |f| {
            f.file_name().to_string_lossy().starts_with("frame_")
        }))
        .count();

    let duration_secs = (duration_ms as f64) / 1000.0;
    let fps = if frame_count > 0 && duration_secs > 0.0 {
        (frame_count as f64 / duration_secs).min(30.0)
    } else {
        3.0
    };

    log::info!("[{camera_id}] encoding video: {frame_count} frames, {duration_secs:.1}s, {fps:.1}fps");

    let mut cmd = std::process::Command::new("ffmpeg");
    cmd.arg("-framerate").arg(&fps.to_string());
    cmd.arg("-pattern_type").arg("glob");
    cmd.arg("-i").arg(&frames_glob);

    if let Some(vf) = build_vf_filter(rotation, mirror) {
        cmd.arg("-vf").arg(&vf);
    }

    cmd.arg("-c:v").arg("libx264");
    cmd.arg("-pix_fmt").arg("yuv420p");
    cmd.arg("-movflags").arg("+faststart");
    cmd.arg("-y");
    cmd.arg(&video_path.to_string_lossy().to_string());

    let output = cmd.output()
        .map_err(|e| format!("ffmpeg not found: {e}"))?;

    if !output.status.success() {
        return Err(format!(
            "ffmpeg failed\nstdout: {}\nstderr: {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        ));
    }

    let bytes = std::fs::read(&video_path)
        .map_err(|e| format!("failed to read video: {e}"))?;

    Ok(bytes)
}

pub async fn render_video_async(
    base_dir: std::path::PathBuf,
    camera_id: String,
    session: String,
    duration_ms: u64,
    rotation: u16,
    mirror: bool,
) -> Result<Vec<u8>, String> {
    let cam_id = camera_id.clone();
    tokio::task::spawn_blocking(move || render_video(&base_dir, &cam_id, &session, duration_ms, rotation, mirror))
        .await
        .map_err(|e| format!("blocking task panicked: {e}"))?
}
