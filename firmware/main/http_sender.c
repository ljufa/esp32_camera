#include "http_sender.h"
#include "device_id.h"
#include "sdkconfig.h"
#include "esp_http_client.h"
#include "esp_crt_bundle.h"
#include "esp_app_desc.h"
#include "esp_log.h"
#include <stdio.h>

static const char *TAG = "http_sender";

static esp_http_client_handle_t s_client = NULL;
static bool s_stream_open = false;
static char s_url[256];

static esp_http_client_handle_t get_client(void)
{
    if (s_client) return s_client;

    snprintf(s_url, sizeof(s_url), "%s/%s/stream",
             CONFIG_HTTP_ENDPOINT_URL, device_id_get());

    esp_http_client_config_t config = {
        .url                = s_url,
        .method             = HTTP_METHOD_POST,
        .timeout_ms         = CONFIG_HTTP_TIMEOUT_MS,
        .keep_alive_idle    = 30,
        .keep_alive_interval = 5,
        .keep_alive_count   = 3,
        .keep_alive_enable  = true,
        .crt_bundle_attach  = esp_crt_bundle_attach,
#if CONFIG_HTTP_BASIC_AUTH_ENABLE
        .username           = CONFIG_HTTP_BASIC_AUTH_USERNAME,
        .password           = CONFIG_HTTP_BASIC_AUTH_PASSWORD,
        .auth_type          = HTTP_AUTH_TYPE_BASIC,
#endif
    };
    s_client = esp_http_client_init(&config);
    if (s_client) {
        esp_http_client_set_header(s_client, "Content-Type", "application/octet-stream");
        esp_http_client_set_header(s_client, "X-Firmware-Version",
                                   esp_app_get_description()->version);
    }
    return s_client;
}

static void stream_close(void)
{
    if (s_client) {
        esp_http_client_close(s_client);
    }
    s_stream_open = false;
}

static esp_err_t stream_ensure_open(void)
{
    if (s_stream_open) return ESP_OK;

    esp_http_client_handle_t client = get_client();
    if (!client) return ESP_FAIL;

    /* write_len -1 makes esp_http_client send Transfer-Encoding: chunked */
    esp_err_t err = esp_http_client_open(client, -1);
    if (err != ESP_OK) {
        ESP_LOGW(TAG, "Stream open failed: %s", esp_err_to_name(err));
        return err;
    }
    s_stream_open = true;
    ESP_LOGI(TAG, "Upload stream connected");
    return ESP_OK;
}

/* Send one JPEG frame over the persistent chunked-POST stream. The server
   never replies per frame, so a frame costs no WAN round trip.

   esp_http_client_write() is a raw transport write, so the HTTP chunk
   framing is formatted here. Each frame is one chunk whose payload is a
   4-byte big-endian length followed by the JPEG; proxies are free to
   re-chunk the stream, so the receiver parses the length prefix, never
   chunk boundaries. */
esp_err_t http_send_frame(const uint8_t *data, size_t len)
{
    if (stream_ensure_open() != ESP_OK) return ESP_FAIL;

    char chunk_hdr[16];
    int hdr_len = snprintf(chunk_hdr, sizeof(chunk_hdr), "%X\r\n",
                           (unsigned)(len + 4));
    uint8_t frame_len[4] = {
        (uint8_t)(len >> 24), (uint8_t)(len >> 16),
        (uint8_t)(len >> 8),  (uint8_t)len,
    };

    if (esp_http_client_write(s_client, chunk_hdr, hdr_len) != hdr_len ||
        esp_http_client_write(s_client, (const char *)frame_len, 4) != 4 ||
        esp_http_client_write(s_client, (const char *)data, (int)len) != (int)len ||
        esp_http_client_write(s_client, "\r\n", 2) != 2) {
        ESP_LOGW(TAG, "Stream write failed — reconnecting on next frame");
        stream_close();
        return ESP_FAIL;
    }
    return ESP_OK;
}
