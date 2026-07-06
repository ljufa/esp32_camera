#include "freertos/FreeRTOS.h"
#include "freertos/task.h"
#include "freertos/queue.h"
#include "freertos/semphr.h"
#include "esp_log.h"
#include "esp_timer.h"
#include "driver/gpio.h"
#include "sdkconfig.h"

#include "camera_init.h"
#include "esp_camera.h"
#include "sensor.h"
#include "wifi_connect.h"
#include "http_sender.h"
#include "device_id.h"
#include "ota.h"

static const char *TAG = "main";

#define FRAME_QUEUE_SIZE    2
#define SENDER_STACK_SIZE   6144
#define SENDER_PRIORITY     1

static QueueHandle_t    s_frame_queue;
static SemaphoreHandle_t s_sender_idle;  /* given when sender holds no fb */


static void sender_task(void *arg)
{
    (void)arg;
    camera_fb_t *fb;
    while (1) {
        if (xQueueReceive(s_frame_queue, &fb, portMAX_DELAY) != pdTRUE) {
            continue;
        }
        xSemaphoreTake(s_sender_idle, portMAX_DELAY);
        esp_err_t err = http_send_frame(fb->buf, fb->len);
        if (err != ESP_OK) {
            ESP_LOGW(TAG, "Send failed");
        }
        esp_camera_fb_return(fb);
        xSemaphoreGive(s_sender_idle);
    }
}

static bool queue_frame(void)
{
    if (uxQueueSpacesAvailable(s_frame_queue) == 0) {
        return false;
    }
    camera_fb_t *fb = esp_camera_fb_get();
    if (!fb) {
        ESP_LOGW(TAG, "Frame capture failed");
        return false;
    }

    if (fb->len < 2 || fb->buf[0] != 0xFF || fb->buf[1] != 0xD8) {
        ESP_LOGW(TAG, "Invalid JPEG frame discarded (%zu bytes)", fb->len);
        esp_camera_fb_return(fb);
        return false;
    }

    if (xQueueSend(s_frame_queue, &fb, 0) != pdTRUE) {
        esp_camera_fb_return(fb);
        return false;
    }
    return true;
}

void app_main(void)
{
    ESP_LOGI(TAG, "ESP32 Security Camera starting...");
    device_id_init();

    s_frame_queue = xQueueCreate(FRAME_QUEUE_SIZE, sizeof(camera_fb_t *));
    assert(s_frame_queue);
    s_sender_idle = xSemaphoreCreateBinary();
    assert(s_sender_idle);
    xSemaphoreGive(s_sender_idle);

    xTaskCreate(sender_task, "sender", SENDER_STACK_SIZE, NULL, SENDER_PRIORITY, NULL);

    ESP_ERROR_CHECK(wifi_connect());
    ota_check_and_update();
    vTaskDelay(pdMS_TO_TICKS(500));

    ESP_ERROR_CHECK(camera_init());
    while (1) {
        queue_frame();
    }
}
