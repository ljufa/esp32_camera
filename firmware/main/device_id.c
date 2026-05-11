#include "device_id.h"
#include "sdkconfig.h"
#include "esp_mac.h"
#include "esp_app_desc.h"
#include "esp_log.h"
#include <stdio.h>
#include <stdint.h>

static const char *TAG = "device_id";
static char s_device_id[64];

void device_id_init(void)
{
    uint8_t mac[6] = {0};
    esp_efuse_mac_get_default(mac);

    const esp_app_desc_t *app = esp_app_get_description();
    const char *ver = (app && app->version[0]) ? app->version : "unknown";

    snprintf(s_device_id, sizeof(s_device_id),
             "%s-%02x%02x%02x-%s",
             CONFIG_BOARD_TYPE, mac[3], mac[4], mac[5], ver);
    ESP_LOGI(TAG, "Device ID: %s", s_device_id);
}

const char *device_id_get(void)
{
    return s_device_id;
}
