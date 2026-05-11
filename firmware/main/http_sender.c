#include "http_sender.h"
#include "device_id.h"
#include "sdkconfig.h"
#include "esp_http_client.h"
#include "esp_crt_bundle.h"
#include "esp_log.h"

static const char *TAG = "http_sender";

static esp_http_client_handle_t s_client = NULL;
static char s_url[256];

static esp_err_t http_event_handler(esp_http_client_event_t *evt)
{
    switch (evt->event_id) {
    case HTTP_EVENT_ERROR:
        ESP_LOGW(TAG, "HTTP error");
        break;
    case HTTP_EVENT_DISCONNECTED:
        ESP_LOGD(TAG, "Disconnected — will reconnect on next frame");
        break;
    case HTTP_EVENT_ON_DATA:
        ESP_LOGD(TAG, "Response (%d bytes): %.*s", evt->data_len, evt->data_len, (char *)evt->data);
        break;
    default:
        break;
    }
    return ESP_OK;
}

static esp_http_client_handle_t get_client(void)
{
    if (s_client) return s_client;

    snprintf(s_url, sizeof(s_url), "%s/%s", CONFIG_HTTP_ENDPOINT_URL, device_id_get());

    esp_http_client_config_t config = {
        .url                = s_url,
        .method             = HTTP_METHOD_POST,
        .timeout_ms         = CONFIG_HTTP_TIMEOUT_MS,
        .keep_alive_idle    = 30,
        .keep_alive_interval = 5,
        .keep_alive_count   = 3,
        .event_handler      = http_event_handler,
        .keep_alive_enable  = true,
        .crt_bundle_attach  = esp_crt_bundle_attach,
#if CONFIG_HTTP_BASIC_AUTH_ENABLE
        .username           = CONFIG_HTTP_BASIC_AUTH_USERNAME,
        .password           = CONFIG_HTTP_BASIC_AUTH_PASSWORD,
        .auth_type          = HTTP_AUTH_TYPE_BASIC,
#endif
    };
    s_client = esp_http_client_init(&config);
    return s_client;
}

esp_err_t http_send_frame(const uint8_t *data, size_t len)
{
    esp_http_client_handle_t client = get_client();
    if (!client) {
        return ESP_FAIL;
    }

    esp_http_client_set_header(client, "Content-Type", "image/jpeg");
    esp_http_client_set_header(client, "Connection", "keep-alive");
    esp_http_client_set_post_field(client, (const char *)data, (int)len);

    esp_err_t ret = esp_http_client_perform(client);
    if (ret == ESP_OK) {
        int status = esp_http_client_get_status_code(client);
        if (status < 200 || status >= 300) {
            ret = ESP_FAIL;
        }
    } else {
        ESP_LOGE(TAG, "HTTP POST failed: %s", esp_err_to_name(ret));
        if (ret != ESP_ERR_HTTP_EAGAIN) {
            esp_http_client_cleanup(client);
            s_client = NULL;
        }
    }

    return ret;
}
