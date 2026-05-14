#include "ota.h"
#include "sdkconfig.h"

#if CONFIG_OTA_ENABLE

#include "esp_log.h"
#include "esp_https_ota.h"
#include "esp_http_client.h"
#include "esp_crt_bundle.h"
#include "esp_app_desc.h"
#include <string.h>

static const char *TAG = "ota";

static bool fetch_server_version(char *buf, size_t buf_len)
{
    esp_http_client_config_t config = {
        .url             = CONFIG_OTA_VERSION_URL,
        .crt_bundle_attach = esp_crt_bundle_attach,
        .timeout_ms      = 10000,
    };
    esp_http_client_handle_t client = esp_http_client_init(&config);
    if (!client) return false;

    bool ok = false;
    if (esp_http_client_open(client, 0) == ESP_OK) {
        esp_http_client_fetch_headers(client);
        int status = esp_http_client_get_status_code(client);
        if (status == 200) {
            int n = esp_http_client_read(client, buf, (int)buf_len - 1);
            if (n > 0) {
                buf[n] = '\0';
                for (int i = n - 1; i >= 0 && (unsigned char)buf[i] <= ' '; i--) {
                    buf[i] = '\0';
                }
                ok = (buf[0] != '\0');
            }
        } else {
            ESP_LOGW(TAG, "Version check returned HTTP %d", status);
        }
    }
    esp_http_client_cleanup(client);
    return ok;
}

void ota_check_and_update(void)
{
    char server_ver[32] = {0};
    if (!fetch_server_version(server_ver, sizeof(server_ver))) {
        ESP_LOGW(TAG, "Version check failed — skipping OTA");
        return;
    }

    const char *current_ver = esp_app_get_description()->version;
    ESP_LOGI(TAG, "Firmware: current=%s server=%s", current_ver, server_ver);

    if (strcmp(server_ver, current_ver) == 0) {
        ESP_LOGI(TAG, "Already up to date");
        return;
    }

    ESP_LOGI(TAG, "Updating to %s from %s", server_ver, CONFIG_OTA_FIRMWARE_URL);

    esp_http_client_config_t http_config = {
        .url               = CONFIG_OTA_FIRMWARE_URL,
        .crt_bundle_attach = esp_crt_bundle_attach,
        .timeout_ms        = 60000,
        .keep_alive_enable = true,
    };
    esp_https_ota_config_t ota_config = {
        .http_config = &http_config,
    };

    esp_err_t ret = esp_https_ota(&ota_config);
    if (ret == ESP_OK) {
        ESP_LOGI(TAG, "OTA complete — rebooting");
        esp_restart();
    } else {
        ESP_LOGE(TAG, "OTA failed: %s — continuing with current firmware", esp_err_to_name(ret));
    }
}

#endif /* CONFIG_OTA_ENABLE */
