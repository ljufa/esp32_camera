#include "pir.h"
#include "driver/gpio.h"
#include "esp_log.h"

static const char *TAG = "pir";

esp_err_t pir_init(int gpio_num)
{
    gpio_config_t cfg = {
        .pin_bit_mask   = 1ULL << gpio_num,
        .mode           = GPIO_MODE_INPUT,
        .pull_up_en     = GPIO_PULLUP_DISABLE,
        .pull_down_en   = GPIO_PULLDOWN_DISABLE,
        .intr_type      = GPIO_INTR_DISABLE,
    };
    esp_err_t ret = gpio_config(&cfg);
    if (ret == ESP_OK) {
        ESP_LOGI(TAG, "PIR sensor on GPIO%d", gpio_num);
    }
    return ret;
}

bool pir_read(int gpio_num)
{
    return gpio_get_level(gpio_num) == 1;
}
