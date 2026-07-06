#!/usr/bin/env bash
# Build every board and stage the OTA binaries in release/, which the server
# serves as FIRMWARE_DIR. Bump PROJECT_VER in CMakeLists.txt first; devices
# only ever look for version current+1, so never skip a version.
set -euo pipefail
cd "$(dirname "$0")"

command -v idf.py >/dev/null 2>&1 || source "$HOME/esp/esp-idf/export.sh"

ver=$(sed -n 's/^set(PROJECT_VER "\(.*\)")$/\1/p' CMakeLists.txt)

mkdir -p release
for board in esp32cam esp32s3; do
    # Defaults files are only applied when the generated sdkconfig doesn't
    # exist, so regenerate it — releases always build from committed config.
    rm -f build.${board}/sdkconfig
    idf.py @boards/${board}.args build
    # Copy exactly this version's binary; the build dir may hold older ones
    board_type=$(sed -n 's/^CONFIG_BOARD_TYPE="\(.*\)"$/\1/p' "sdkconfig.defaults.${board}" | tr -c 'a-zA-Z0-9_\n' '_')
    cp "build.${board}/${board_type}_v${ver}.bin" release/
done

echo "Staged binaries:"
ls -lh release/*.bin
