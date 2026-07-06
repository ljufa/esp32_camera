#include "camera_init.h"
#include "sdkconfig.h"
#include "esp_log.h"
#include "driver/gpio.h"
#include "freertos/FreeRTOS.h"
#include "freertos/task.h"
#include "sensor.h"

static const char *TAG = "camera";

#define INIT_MAX_RETRIES 5
#define INIT_RETRY_DELAY_MS 500

static void power_cycle_camera(void)
{
    if (CONFIG_CAM_PIN_PWDN < 0) return;
    gpio_set_direction(CONFIG_CAM_PIN_PWDN, GPIO_MODE_OUTPUT);
    gpio_set_level(CONFIG_CAM_PIN_PWDN, 1);
    vTaskDelay(pdMS_TO_TICKS(100));
    gpio_set_level(CONFIG_CAM_PIN_PWDN, 0);
    vTaskDelay(pdMS_TO_TICKS(100));
}

/* Unstick a frozen I2C/SCCB bus by clocking out the stalled slave transaction.
   A stuck SDA is cleared by toggling SCL 9 times, then issuing a STOP condition. */
static void sccb_bus_recover(void)
{
    gpio_set_direction(CONFIG_CAM_PIN_SIOC, GPIO_MODE_OUTPUT_OD);
    gpio_set_direction(CONFIG_CAM_PIN_SIOD, GPIO_MODE_OUTPUT_OD);
    gpio_set_level(CONFIG_CAM_PIN_SIOD, 1);

    for (int i = 0; i < 9; i++) {
        gpio_set_level(CONFIG_CAM_PIN_SIOC, 0);
        vTaskDelay(pdMS_TO_TICKS(2));
        gpio_set_level(CONFIG_CAM_PIN_SIOC, 1);
        vTaskDelay(pdMS_TO_TICKS(2));
    }

    /* STOP condition: SDA low → SCL high → SDA high */
    gpio_set_level(CONFIG_CAM_PIN_SIOD, 0);
    vTaskDelay(pdMS_TO_TICKS(2));
    gpio_set_level(CONFIG_CAM_PIN_SIOC, 1);
    vTaskDelay(pdMS_TO_TICKS(2));
    gpio_set_level(CONFIG_CAM_PIN_SIOD, 1);
    vTaskDelay(pdMS_TO_TICKS(2));

    /* Release bus — let the camera driver reconfigure these pins */
    gpio_set_direction(CONFIG_CAM_PIN_SIOC, GPIO_MODE_INPUT);
    gpio_set_direction(CONFIG_CAM_PIN_SIOD, GPIO_MODE_INPUT);
    ESP_LOGD(TAG, "SCCB bus recovery done");
}

static esp_err_t camera_init_once(void)
{
    camera_config_t config = {
        .pin_pwdn       = CONFIG_CAM_PIN_PWDN,
        .pin_reset      = CONFIG_CAM_PIN_RESET,
        .pin_xclk       = CONFIG_CAM_PIN_XCLK,
        .pin_sccb_sda   = CONFIG_CAM_PIN_SIOD,
        .pin_sccb_scl   = CONFIG_CAM_PIN_SIOC,
        .pin_d7         = CONFIG_CAM_PIN_D7,
        .pin_d6         = CONFIG_CAM_PIN_D6,
        .pin_d5         = CONFIG_CAM_PIN_D5,
        .pin_d4         = CONFIG_CAM_PIN_D4,
        .pin_d3         = CONFIG_CAM_PIN_D3,
        .pin_d2         = CONFIG_CAM_PIN_D2,
        .pin_d1         = CONFIG_CAM_PIN_D1,
        .pin_d0         = CONFIG_CAM_PIN_D0,
        .pin_vsync      = CONFIG_CAM_PIN_VSYNC,
        .pin_href       = CONFIG_CAM_PIN_HREF,
        .pin_pclk       = CONFIG_CAM_PIN_PCLK,

        .xclk_freq_hz   = 20000000,
        .ledc_timer     = LEDC_TIMER_0,
        .ledc_channel   = LEDC_CHANNEL_0,

        .pixel_format   = PIXFORMAT_JPEG,
        .frame_size     = CONFIG_FRAME_SIZE,
        .jpeg_quality   = CONFIG_JPEG_QUALITY,
        /* 3 buffers: one being sent, one queued, one free for the driver —
           a stalled upload (dead connection, slow WAN) then drops frames
           instead of starving capture ("Failed to get frame: timeout"). */
        .fb_count       = 3,
        .fb_location    = CAMERA_FB_IN_PSRAM,
        .grab_mode      = CAMERA_GRAB_LATEST,
    };

    return esp_camera_init(&config);
}

esp_err_t camera_init(void)
{
    for (int attempt = 1; attempt <= INIT_MAX_RETRIES; attempt++) {
        ESP_LOGI(TAG, "Camera init attempt %d/%d", attempt, INIT_MAX_RETRIES);

        power_cycle_camera();

        esp_err_t ret = camera_init_once();
        if (ret == ESP_OK) {
            /* Let I2C bus fully settle before sensor register writes */
            vTaskDelay(pdMS_TO_TICKS(300));
            sensor_t *sensor = esp_camera_sensor_get();
            if (sensor) {
                sensor->set_vflip(sensor, 1);
                sensor->set_brightness(sensor, 1);
                sensor->set_saturation(sensor, -2);
                sensor->set_sharpness(sensor, 1);
                sensor->set_denoise(sensor, 1);
                sensor->set_bpc(sensor, 1);
                sensor->set_wpc(sensor, 1);
                sensor->set_lenc(sensor, 1);
                sensor->set_raw_gma(sensor, 1);
            }
            ESP_LOGI(TAG, "Camera ready (attempt %d)", attempt);
            return ESP_OK;
        }

        ESP_LOGW(TAG, "Camera init failed (%s), retrying in %d ms...",
                 esp_err_to_name(ret), INIT_RETRY_DELAY_MS);

        esp_camera_deinit();
        sccb_bus_recover();
        power_cycle_camera();
        vTaskDelay(pdMS_TO_TICKS(INIT_RETRY_DELAY_MS));
    }

    ESP_LOGE(TAG, "Camera init failed after %d attempts", INIT_MAX_RETRIES);
    return ESP_FAIL;
}
