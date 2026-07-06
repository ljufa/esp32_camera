# ESP32 Security Camera

Security camera firmware that captures JPEG frames and POSTs them to the
streaming server. One source tree supports multiple boards via per-board
config files.

## Supported boards

| Board id  | Hardware                                                        |
|-----------|-----------------------------------------------------------------|
| `esp32cam`| Original ESP32 (AI-Thinker pinout, USB-C carrier), OV2640, 4 MB flash, quad PSRAM |
| `esp32s3` | Freenove ESP32-S3-WROOM, OV2640 (OV5640 overheats on this board), 16 MB flash, 8 MB OPI PSRAM    |

Board config lives in `sdkconfig.defaults.<board>` (target, flash, PSRAM,
camera pins, partition table); shared settings in `sdkconfig.defaults`.
Each board builds into its own `build.<board>/` directory with its own
generated sdkconfig, so builds never interfere.

## Requirements

- ESP-IDF >= 5.0

## Build & Flash

```bash
idf.py @boards/esp32cam.args build
idf.py @boards/esp32s3.args build

idf.py @boards/esp32cam.args -p /dev/ttyUSB0 flash monitor
```

WiFi credentials, endpoint URL, OTA settings, and frame size/quality are
Kconfig defaults (`main/Kconfig.projbuild`); tweak per checkout with
`idf.py @boards/<board>.args menuconfig`.

**Always go through `@boards/<board>.args`** — never run bare `idf.py build`
or `idf.py set-target`. Bare invocations use a `sdkconfig` at the project
root: `set-target` regenerates it (losing menuconfig-only values), and a
stale root sdkconfig from another board silently drives chip detection, so
you can end up with e.g. an esp32cam binary built for the S3 toolchain. If a
root `sdkconfig`/`sdkconfig.old` ever appears, delete it.

## Releasing (OTA)

1. Bump `PROJECT_VER` in `CMakeLists.txt`. Never skip a version — devices
   only ever check for `v{current+1}`.
2. Run `./release.sh` — builds every board and stages
   `release/<board_type>_v<N>.bin`, which the server serves as
   `FIRMWARE_DIR`.
3. Devices check for the next version once at boot, so power-cycle (or wait
   for the next natural reboot).

### OTA limits

- **OTA only replaces the app partition.** Bootloader-level settings — flash
  frequency, flash size, anything in the bootloader image header — need one
  flash over serial. Concretely: the 40→80 MHz flash-clock change (July 2026)
  reaches the original `esp32cam` board only via serial; on the S3 the flash
  clock was already 80 MHz, so it updates fully over the air. On the original
  ESP32, 80 MHz PSRAM also depends on 80 MHz flash, so that speedup rides on
  the same serial flash.
- **Binary names use the sanitized board type** (non-alphanumerics → `_`,
  matching the CMake project name), and since v5 `ota.c` sanitizes the same
  way. Firmware v3/v4 on the S3 asked for the raw hyphenated name
  (`esp32s3-freenove_v4.bin`) — `release/esp32s3-freenove_v4.bin` exists
  (v5 content) purely so those devices can escape; new boards whose
  `BOARD_TYPE` contains `-` don't need this.
- **Devices more than one version behind need stepping stones.** A device at
  v1 asks only for `_v2.bin` and gives up (404) if it's missing. Serve every
  intermediate name; the *content* can simply be the newest build renamed —
  the device flashes it, boots reporting the new (higher) version, and
  continues from there. Example: `release/esp32s3_freenove_v2.bin` is a copy
  of the v3 binary, placed so the S3 camera that shipped at v1 can reach v3.

## Adding a board

1. `sdkconfig.defaults.<name>` — target, flash size/clock, PSRAM mode,
   `CONFIG_BOARD_TYPE`, camera pins, partition CSV name.
2. `partitions_<name>.csv` — sized to the board's flash.
3. `boards/<name>.args` — copy an existing one, replace the board name
   everywhere.
4. Add the name to the board loop in `release.sh`.

`CONFIG_BOARD_TYPE` becomes both the OTA binary name and part of the device
ID, and the CMake build reads it from the defaults file (via `-D CAM_BOARD`
in the args file), so it stays consistent everywhere by construction.

## Project structure

```
firmware/
├── CMakeLists.txt               # PROJECT_VER + board-aware project name
├── boards/<board>.args          # idf.py argument files (build dir, defaults)
├── sdkconfig.defaults           # shared settings
├── sdkconfig.defaults.<board>   # per-board target/flash/PSRAM/pins
├── partitions_<board>.csv       # per-board partition tables
├── release.sh                   # build all boards, stage OTA binaries
├── release/                     # served by the server as FIRMWARE_DIR
└── main/                        # C sources (board-independent)
```

## Notes

- PSRAM is required; JPEG format keeps memory usage manageable alongside WiFi.
- The sensor is initialized with vertical flip enabled — adjust in
  `camera_init.c` if your image is upside down.
