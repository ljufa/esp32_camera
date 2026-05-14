# ESP32 Security Camera — Firmware

AI-Thinker ESP32-CAM (original ESP32, OV2640) firmware. Captures JPEG frames and POSTs them to a configurable HTTP endpoint. Supports PIR-triggered capture and OTA firmware updates.

## Requirements

- ESP-IDF >= 5.0
- AI-Thinker ESP32-CAM (original ESP32 with PSRAM, OV2640)

## Quick Start

### 1. Configure

```bash
idf.py menuconfig
# → Security Camera Configuration
#   Set WiFi SSID/password, HTTP endpoint URL
#   Set OTA version/firmware URLs
#   Adjust camera pins if your board differs from defaults
```

### 2. Build & Flash

First flash must be a full flash (writes the OTA partition table):

```bash
idf.py build
idf.py -p /dev/ttyUSB0 flash monitor
```

Subsequent updates can be pushed OTA — see the server README.

## Project Structure

```
firmware/
├── CMakeLists.txt           # PROJECT_VER lives here
├── sdkconfig.defaults       # ESP32 + PSRAM defaults
├── partitions.csv           # OTA-aware partition table (ota_0 + ota_1)
└── main/
    ├── Kconfig.projbuild    # All runtime config (WiFi, pins, OTA, etc.)
    ├── idf_component.yml    # Pulls in espressif/esp32-camera
    ├── main.c               # Capture loop, PIR logic
    ├── camera_init.c/h      # OV2640 init, sensor tweaks, retry logic
    ├── wifi_connect.c/h     # WiFi STA with retry
    ├── http_sender.c/h      # HTTP POST of JPEG frames (keep-alive)
    ├── ota.c/h              # Poll-on-boot OTA update
    ├── pir.c/h              # PIR motion sensor
    └── device_id.c/h        # Stable device ID: <board>-<mac6>
```

## Camera Pin Defaults (AI-Thinker ESP32-CAM)

| Signal | GPIO |
|--------|------|
| PWDN   | 32   |
| XCLK   | 0    |
| SIOD   | 26   |
| SIOC   | 27   |
| D0–D7  | 5,18,19,21,36,39,34,35 |
| VSYNC  | 25   |
| HREF   | 23   |
| PCLK   | 22   |

Override in `menuconfig → Camera Pins` if your board differs.

## Configuration Reference

All options are under `menuconfig → Security Camera Configuration`.

| Section | Key | Default | Description |
|---------|-----|---------|-------------|
| Device Identity | `BOARD_TYPE` | `esp32cam` | Board name prefix in device ID |
| Status LED | `STATUS_LED_ENABLE` | y | Flash LED on GPIO4 while capturing |
| PIR | `PIR_ENABLE` | y | Trigger capture on motion; disable for continuous |
| PIR | `PIR_GPIO` | 16 | GPIO connected to PIR output |
| WiFi | `WIFI_SSID` / `WIFI_PASSWORD` | — | Network credentials |
| HTTP Endpoint | `HTTP_ENDPOINT_URL` | — | Base URL; camera ID appended automatically |
| HTTP Endpoint | `HTTP_BASIC_AUTH_ENABLE` | n | Enable Basic Auth on upload |
| OTA | `OTA_ENABLE` | y | Check for updates on every boot |
| OTA | `OTA_VERSION_URL` | — | `GET` returns plain-text version string |
| OTA | `OTA_FIRMWARE_URL` | — | URL of `.bin` to download on update |
| Capture | `JPEG_QUALITY` | 10 | JPEG quality (lower = better, 4–63) |

## Device ID

Each device identifies itself as `<board>-<mac6>`, e.g. `esp32cam-a1b2c3`, derived from the board type config and the last 3 bytes of the factory eFuse MAC. This ID is stable across firmware updates and is appended to the upload URL: `POST /upload/esp32cam-a1b2c3`.

## OTA Updates

On every boot (after WiFi connects), the firmware GETs `OTA_VERSION_URL`. If the returned version string differs from the running firmware, it downloads `OTA_FIRMWARE_URL`, flashes it, and reboots. On failure it logs the error and continues normally.

See the server README for how to publish a new firmware version.

## Partition Table

Uses an OTA-aware layout with two 1.5 MB app slots:

| Partition | Size |
|-----------|------|
| nvs | 20 KB |
| otadata | 8 KB |
| ota_0 | 1.5 MB |
| ota_1 | 1.5 MB |
| spiffs | 960 KB |

Current binary is ~1 MB, leaving ~512 KB of headroom per slot.
