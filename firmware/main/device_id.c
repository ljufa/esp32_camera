#include "device_id.h"
#include "sdkconfig.h"
#include "esp_mac.h"
#include "esp_log.h"
#include <stdio.h>
#include <stdint.h>

static const char *TAG = "device_id";
static char s_device_id[32];

void device_id_init(void)
{
    uint8_t mac[6] = {0};
    esp_efuse_mac_get_default(mac);

    snprintf(s_device_id, sizeof(s_device_id),
             "%s-%02x%02x%02x",
             CONFIG_BOARD_TYPE, mac[3], mac[4], mac[5]);
    ESP_LOGI(TAG, "Device ID: %s", s_device_id);
}

const char *device_id_get(void)
{
    return s_device_id;
}
