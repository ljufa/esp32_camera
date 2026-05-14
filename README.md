# ESP32 Security Camera

A minimal, self-hosted security camera system built around an AI-Thinker ESP32-CAM (original ESP32, OV2640). The firmware streams JPEG frames over HTTP to a lightweight Rust server that handles live viewing, software motion detection, video encoding, and optional Telegram notifications.

```
┌─────────────────────┐        HTTP POST /upload/<id>       ┌──────────────────────────┐
│  ESP32-CAM firmware │  ──────────────────────────────►    │  Rust server (Docker)    │
│                     │                                     │                          │
│  • OV2640 capture   │                                     │  • Live MJPEG streams    │
│  • PIR wake-up      │                                     │  • Motion detection      │
│  • OTA updates      │                                     │  • MP4 encoding (ffmpeg) │
│  • HTTP keep-alive  │                                     │  • Telegram alerts       │
└─────────────────────┘                                     │  • Web dashboard         │
                                                            └──────────────────────────┘
```

---

## Firmware

See [`firmware/README.md`](firmware/README.md) for hardware requirements, pin mapping, build instructions, and configuration reference.

---

## Server

### Quick start (Docker Compose)

```bash
cd server

# Create your env file from the template
cp .env.example .env
$EDITOR .env   # fill in TRAEFIK_HOST, TELEGRAM_TOKEN, etc.

docker compose up -d
```

The server listens on port **8080** inside the container. The compose file assumes a Traefik reverse proxy on an external `proxy` network — adjust or remove the `labels` section for a simpler setup.

### Without Docker (local dev)

```bash
cd server
cargo run --release
```

Environment variables (all optional except `SAVE_DIR`):

| Variable | Default | Description |
|----------|---------|-------------|
| `SAVE_DIR` | required | Directory for saved frames and videos |
| `DB_DIR` | `$SAVE_DIR/db` | Camera config database directory |
| `FIRMWARE_DIR` | — | Directory served at `/firmware/`; omit to disable OTA |
| `SERVER_BIND_ADDRESS` | `0.0.0.0:8080` | Listen address |
| `TELEGRAM_TOKEN` | — | Bot token; omit to disable notifications |
| `TELEGRAM_CHAT_ID` | — | Chat/group ID for alerts |
| `MOTION_TIMEOUT_MS` | `60000` | Idle time before a session closes |
| `PIXEL_THRESHOLD` | `40` | Per-pixel diff threshold (0–255) |
| `MOTION_CHECK_EVERY` | `5` | Check every Nth frame |
| `MOTION_PERCENT` | `1.0` | % of pixels that must change to trigger |
| `RETAIN_DAYS` | `7` | Days before raw frames are deleted by cron |

### HTTP API

| Method | Path | Description |
|--------|------|-------------|
| `POST` | `/upload/<camera_id>` | Receive a JPEG frame from firmware |
| `GET` | `/stream/<camera_id>` | MJPEG stream (multipart/x-mixed-replace) |
| `GET` | `/` | Web dashboard |
| `GET` | `/status.json` | JSON status for all cameras |
| `PATCH` | `/api/camera/<id>/config` | Update camera settings |
| `DELETE` | `/api/camera/<id>` | Remove a camera |
| `GET` | `/firmware/version` | Current firmware version string (OTA check) |
| `GET` | `/firmware/<filename>` | Download firmware binary (OTA update) |
| `GET` | `/swagger-ui` | Interactive API docs |

### Storage layout

```
$SAVE_DIR/
└── <camera-name>/
    └── <DD-MM-YYYY>/
        ├── .raw/
        │   └── <HH-MM-SS>/
        │       ├── frame_000001.jpg
        │       └── frame_000002.jpg
        └── <HH-MM-SS>.mp4   ← encoded after motion session ends
```

### Cleanup cron

The container runs a cron job (`cleanup.sh`) that deletes raw `.jpg` frames older than `RETAIN_DAYS` days and removes any empty directories left behind. MP4 videos are not touched by cleanup — delete them manually if needed.

The schedule is controlled by `CLEANUP_CRON` (default `*/20 * * * *` — every 20 minutes). Change it in `docker-compose.yaml`:

```yaml
environment:
  - CLEANUP_CRON=0 3 * * *   # once a day at 03:00
  - RETAIN_DAYS=7
```

### Web dashboard

The dashboard at `/` auto-refreshes every 2 seconds via `/status.json`. Each camera gets its own card with:

- **Live MJPEG stream** — starts automatically when the camera is active, reconnects on error or when the browser tab regains focus
- **LIVE / OFFLINE badge** with current FPS
- **Viewer count** and IP list of active stream consumers
- **Settings panel** (gear icon) — all settings are persisted in the database and survive a server restart:
  - Rename the camera (display name used for file paths and Telegram messages)
  - Toggle motion detection and Telegram notifications
  - Rotation (0 / 90 / 180 / 270°) and mirror — applied server-side via CSS transform on the stream
  - Pixel threshold, motion percentage, motion timeout, and check-every-N-frames tuning
  - Delete camera — removes config from the database; frames on disk are kept; device will reappear with defaults if it keeps posting
- **Filebrowser link** — camera title links to `/fb/files/<camera-name>/` for browsing saved recordings

### Authentication

The compose setup uses two separate Traefik Basic Auth credentials:

| Credential | Variable | Protects | Protocol |
|------------|----------|----------|----------|
| `camera_stream` | `TRAEFIK_BASIC_AUTH_USERS` | `POST /upload/*` | **HTTP** (plain) |
| `camera_ui` | `TRAEFIK_UI_BASIC_AUTH_USERS` | Dashboard, streams, API | **HTTPS** |
| *(none)* | — | `GET /firmware/*` | **HTTP** (plain) |

The upload and firmware routes are intentionally HTTP-only — skipping TLS cuts latency for high-frequency JPEG POSTs and avoids auth complexity for OTA on the device side. Everything the browser touches goes over HTTPS.

Generate each password hash with:
```bash
echo $(htpasswd -nb put_your_username_here put_your_password_here) | sed -e s/\\$/\\$\\$/g
```

### OTA firmware updates

The server serves firmware from `FIRMWARE_DIR` (mounted from `firmware/build/` in the repo). To publish a new version:

```bash
# 1. Bump PROJECT_VER in firmware/CMakeLists.txt, then build
cd firmware && idf.py build

# 2. Write version file alongside the binary
cd ../server && cargo make package-firmware

# 3. Commit and push
git add ../firmware/build/esp32_security_camera.bin ../firmware/build/version
git commit -m "firmware 0.x.0"
git push

# 4. Pull on the server and restart
git pull && docker compose restart camera-server
```

Devices check for updates on every boot. If the version file on the server differs from the running firmware, the device downloads the binary, flashes it, and reboots.

### Motion detection

Frames are downscaled to 160×120 greyscale before comparison. A session starts when `MOTION_PERCENT`% of pixels differ by more than `PIXEL_THRESHOLD`. After `MOTION_TIMEOUT_MS` of inactivity the session closes, raw frames are encoded into an MP4 with ffmpeg, and the video is sent to Telegram.

### Telegram bot

Create a bot via [@BotFather](https://t.me/BotFather) and set `TELEGRAM_TOKEN` + `TELEGRAM_CHAT_ID`.

Bot commands:

| Command | Description |
|---------|-------------|
| `/help` | Show available commands |
| `/list` | List all cameras with status |
| `/motion_on <id>` | Enable motion detection |
| `/motion_off <id>` | Disable motion detection |
| `/notify_on <id>` | Enable Telegram notifications |
| `/notify_off <id>` | Disable Telegram notifications |

---

## Disclaimer

This is a personal hobby project, shared as-is for anyone who finds it useful. It is **not** production-ready software and comes with no guarantees of reliability, security, or fitness for any particular purpose. Use it at your own risk. The author is not responsible for any damage, data loss, security incidents, or other consequences arising from the use of this project.

## License

MIT — see [LICENSE](LICENSE). The license explicitly provides the software *"as is, without warranty of any kind"* and excludes the author from any liability, which reflects the hobby nature of this project.
