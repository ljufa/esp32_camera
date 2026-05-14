# ESP32 Security Camera

ESP32-S3 (USB-C) + OV3660 security camera that captures JPEG frames and POSTs them to a configurable HTTP endpoint.

## Requirements

- ESP-IDF >= 5.0
- ESP32-S3 board with OV3660 and PSRAM

## Quick Start

### 1. Configure

```bash
idf.py menuconfig
# → Security Camera Configuration
#   Set WiFi SSID/password, HTTP endpoint URL, capture interval
#   Adjust camera pins if your board differs from defaults
```

### 2. Build & Flash

```bash
idf.py build
idf.py -p /dev/ttyUSB0 flash monitor
```

### 3. Run the test server

```bash
cd server
python3 server.py --port 8080 --output ./frames
```

Set the endpoint URL in menuconfig to `http://<your-pc-ip>:8080/upload`.

## Project Structure

```
esp32_camera/
├── CMakeLists.txt
├── sdkconfig.defaults       # ESP32-S3 + PSRAM defaults
├── main/
│   ├── Kconfig.projbuild    # All runtime config (WiFi, pins, interval, etc.)
│   ├── idf_component.yml    # Pulls in espressif/esp32-camera via IDF Component Manager
│   ├── main.c               # Capture loop
│   ├── camera_init.c/h      # OV3660 init + sensor tweaks
│   ├── wifi_connect.c/h     # WiFi STA connection with retry
│   └── http_sender.c/h      # HTTP POST of raw JPEG
└── server/
    └── server.py            # Reference receive server
```

## Camera Pin Defaults (AI-Thinker ESP32-CAM / OV3660)

Board: original ESP32 (ESP-32S module) with PSRAM64H, USB-C carrier.

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

These are the standard AI-Thinker ESP32-CAM pin assignments. Override in `menuconfig` if your board differs.

## Notes

- PSRAM is required; JPEG format is used to keep memory usage manageable alongside WiFi.
- The OV3660 is initialized with vertical flip enabled — disable in `camera_init.c` if your image appears correct.
- The HTTP endpoint stub (`server/server.py`) is intentionally minimal. Replace with your production backend later.
