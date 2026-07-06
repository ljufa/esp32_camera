#!/bin/sh

CLEANUP_CRON=${CLEANUP_CRON:-"0 3 * * *"}
echo "[entrypoint] cleanup cron: ${CLEANUP_CRON}"

# Cron setup needs root — skip it when running as non-root
if [ "$(id -u)" -eq 0 ]; then
    printf '%s root /usr/local/bin/cleanup.sh >> /proc/1/fd/1 2>&1\n' "$CLEANUP_CRON" > /etc/cron.d/cleanup
    chmod 0644 /etc/cron.d/cleanup
    cron
else
    echo "[entrypoint] not root — skipping cron"
fi

exec esp32-camera-server
