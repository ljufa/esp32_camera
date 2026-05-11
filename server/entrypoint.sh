#!/bin/sh
printenv > /run/container.env

CLEANUP_CRON=${CLEANUP_CRON:-"0 3 * * *"}
printf '%s root /usr/local/bin/cleanup.sh >> /proc/1/fd/1 2>&1\n' "$CLEANUP_CRON" > /etc/cron.d/cleanup
chmod 0644 /etc/cron.d/cleanup

echo "[entrypoint] cleanup cron: ${CLEANUP_CRON}"

cron
exec esp32-camera-server
