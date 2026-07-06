#include "ota.h"
#include "sdkconfig.h"

#if CONFIG_OTA_ENABLE

#include "esp_log.h"
#include "esp_https_ota.h"
#include "esp_http_client.h"
#include "esp_crt_bundle.h"
#include "esp_app_desc.h"
#include <stdlib.h>
#include <stdio.h>
#include <ctype.h>

static const char *TAG = "ota";

void ota_check_and_update(void)
{
    const char *current_ver_str = esp_app_get_description()->version;
    long current_ver = strtol(current_ver_str, NULL, 10);

    /* Binary names come from the CMake project name, which sanitizes
       BOARD_TYPE (non-alphanumerics → '_'); mirror that here or a board
       type like "esp32s3-freenove" requests a file that never exists. */
    char board[64];
    snprintf(board, sizeof(board), "%s", CONFIG_BOARD_TYPE);
    for (char *p = board; *p; p++) {
        if (!isalnum((unsigned char)*p)) *p = '_';
    }

    char url[256];
    snprintf(url, sizeof(url), "%s/%s_v%ld.bin",
             CONFIG_OTA_FIRMWARE_URL, board, current_ver + 1);
    ESP_LOGI(TAG, "Checking OTA: %s", url);

    esp_http_client_config_t config = {
        .url               = url,
        .crt_bundle_attach = esp_crt_bundle_attach,
        .timeout_ms        = 10000,
#if CONFIG_OTA_BASIC_AUTH_ENABLE
        .username          = CONFIG_OTA_BASIC_AUTH_USERNAME,
        .password          = CONFIG_OTA_BASIC_AUTH_PASSWORD,
        .auth_type         = HTTP_AUTH_TYPE_BASIC,
#endif
    };
    esp_http_client_handle_t client = esp_http_client_init(&config);
    if (!client) return;

    esp_err_t ret = esp_http_client_open(client, 0);
    if (ret != ESP_OK) {
        esp_http_client_cleanup(client);
        ESP_LOGI(TAG, "No update (v%ld not reachable)", current_ver + 1);
        return;
    }
    esp_http_client_fetch_headers(client);
    int status = esp_http_client_get_status_code(client);
    esp_http_client_cleanup(client);

    if (status == 404) {
        ESP_LOGI(TAG, "Up to date (v%ld)", current_ver);
        return;
    }
    if (status != 200) {
        ESP_LOGW(TAG, "OTA check returned HTTP %d", status);
        return;
    }

    ESP_LOGI(TAG, "Updating v%ld → v%ld", current_ver, current_ver + 1);

    esp_http_client_config_t ota_http = {
        .url               = url,
        .crt_bundle_attach = esp_crt_bundle_attach,
        .timeout_ms        = 60000,
#if CONFIG_OTA_BASIC_AUTH_ENABLE
        .username          = CONFIG_OTA_BASIC_AUTH_USERNAME,
        .password          = CONFIG_OTA_BASIC_AUTH_PASSWORD,
        .auth_type         = HTTP_AUTH_TYPE_BASIC,
#endif
    };
    esp_https_ota_config_t ota_config = {
        .http_config = &ota_http,
    };

    ret = esp_https_ota(&ota_config);
    if (ret == ESP_OK) {
        ESP_LOGI(TAG, "OTA complete — rebooting");
        esp_restart();
    } else {
        ESP_LOGE(TAG, "OTA failed: %s", esp_err_to_name(ret));
    }
}

#endif /* CONFIG_OTA_ENABLE */
