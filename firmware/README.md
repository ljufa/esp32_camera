# Firmware

ESP-IDF project for the AI-Thinker ESP32-CAM (original ESP32, OV2640).

## Hardware

| Component | Detail |
|-----------|--------|
| MCU | AI-Thinker ESP32-CAM (original ESP32, not S3) |
| Camera | OV2640 (or OV3660 — same pin-out) |
| PSRAM | PSRAM64H — required for JPEG capture alongside WiFi |
| PIR | Optional; any GPIO, active-HIGH output |

Default pin mapping matches the AI-Thinker ESP32-CAM board:

| Signal | GPIO |
|--------|------|
| PWDN | 32 |
| XCLK | 0 |
| SIOD | 26 |
| SIOC | 27 |
| D0–D7 | 5, 18, 19, 21, 36, 39, 34, 35 |
| VSYNC | 25 |
| HREF | 23 |
| PCLK | 22 |
| Flash LED | 4 |
| PIR (default) | 16 |

All pins are overridable in `menuconfig`.

## Build & flash

**Requirements:** ESP-IDF ≥ 6.0

```bash
# Configure WiFi, endpoint URL, pins, etc.
idf.py menuconfig
# → Security Camera Configuration

idf.py build
idf.py -p /dev/ttyUSB0 flash monitor
```

## Configuration (menuconfig)

| Setting | Default | Description |
|---------|---------|-------------|
| WiFi SSID | `myssid` | Network to join |
| WiFi password | `mypassword` | Network password |
| HTTP endpoint URL | `http://192.168.1.100:8080/upload` | Server base URL |
| HTTP timeout | 10 000 ms | Per-frame POST timeout |
| HTTP basic auth | disabled | Enable + set user/pass if server requires it |
| PIR enable | yes | Disable for continuous streaming |
| PIR GPIO | 16 | GPIO connected to PIR output |
| Flash LED enable | yes | Flash LED on GPIO 4 while capturing |
| JPEG quality | 10 | Lower = better quality, larger frame |

The device ID (`<board>-<mac6>-<fwver>`, e.g. `esp32cam-a1b2c3-1.0.0`) is appended to the URL automatically — no per-device config needed on the server.

## Notes

- `sdkconfig` is git-ignored because it stores credentials. Configure with `idf.py menuconfig` and run `idf.py build` — the file is created automatically.
- `sdkconfig.defaults` (committed) sets PSRAM, partition table, and HTTP/TLS options. User credentials are never stored there.
- The camera is initialised with vertical flip on — disable in `camera_init.c` if your image is upside-down.
- Enabling PIR (`CONFIG_PIR_ENABLE`) reduces effective FPS: the main loop polls the PIR pin and only queues a frame when the pin is HIGH, so the capture rate depends on how quickly the loop iterates rather than the camera's native frame rate. Disable PIR for maximum throughput (continuous streaming).
