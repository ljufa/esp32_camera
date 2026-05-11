FROM ubuntu:noble
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates curl ffmpeg cron && rm -rf /var/lib/apt/lists/*
COPY target/release/esp32-camera-server /usr/local/bin/esp32-camera-server
COPY cleanup.sh /usr/local/bin/cleanup.sh
COPY entrypoint.sh /entrypoint.sh
RUN chmod +x /usr/local/bin/cleanup.sh /entrypoint.sh
EXPOSE 8080
CMD ["/entrypoint.sh"]
