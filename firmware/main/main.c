#include "freertos/FreeRTOS.h"
#include "freertos/task.h"
#include "freertos/queue.h"
#include "esp_log.h"
#include "driver/gpio.h"
#include "sdkconfig.h"

#include "camera_init.h"
#include "wifi_connect.h"
#include "http_sender.h"
#include "pir.h"
#include "device_id.h"

static const char *TAG = "main";
static const char *TAG_PIR = "pir_state";


/* GPIO4 flash LED on AI-Thinker ESP32-CAM is active HIGH */
#if CONFIG_STATUS_LED_ENABLE
#define LED_ON()  gpio_set_level(CONFIG_STATUS_LED_GPIO, 1)
#define LED_OFF() gpio_set_level(CONFIG_STATUS_LED_GPIO, 0)
#else
#define LED_ON()  do {} while (0)
#define LED_OFF() do {} while (0)
#endif

/* Camera fb_count is 4 (camera_init.c). The driver needs at least one fb
 * free to capture into, so user code can hold at most fb_count - 1 = 3 fbs.
 * The sender holds 1 while POSTing, leaving room for 2 in the queue. */
#define FRAME_QUEUE_SIZE    2
#define SENDER_STACK_SIZE   6144
#define SENDER_PRIORITY     1

static QueueHandle_t s_frame_queue;

static void led_init(void)
{
#if CONFIG_STATUS_LED_ENABLE
    gpio_config_t cfg = {
        .pin_bit_mask = 1ULL << CONFIG_STATUS_LED_GPIO,
        .mode         = GPIO_MODE_OUTPUT,
        .pull_up_en   = GPIO_PULLUP_DISABLE,
        .pull_down_en = GPIO_PULLDOWN_DISABLE,
        .intr_type    = GPIO_INTR_DISABLE,
    };
    gpio_config(&cfg);
    LED_OFF();
#endif
}

static void sender_task(void *arg)
{
    (void)arg;
    camera_fb_t *fb;
    while (1) {
        if (xQueueReceive(s_frame_queue, &fb, portMAX_DELAY) != pdTRUE) {
            continue;
        }
        esp_err_t err = http_send_frame(fb->buf, fb->len);
        if (err != ESP_OK) {
            ESP_LOGW(TAG, "Send failed");
        }
        esp_camera_fb_return(fb);
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

    xTaskCreate(sender_task, "sender", SENDER_STACK_SIZE, NULL,
                SENDER_PRIORITY, NULL);

    led_init();
    ESP_ERROR_CHECK(wifi_connect());
    vTaskDelay(pdMS_TO_TICKS(500));
    ESP_ERROR_CHECK(camera_init());

#if CONFIG_PIR_ENABLE
    ESP_ERROR_CHECK(pir_init(CONFIG_PIR_GPIO));
    ESP_LOGI(TAG_PIR, "Idle — waiting for motion on GPIO%d", CONFIG_PIR_GPIO);

    bool pir_was_high = false;
#else
    ESP_LOGI(TAG, "PIR disabled — continuous streaming");
    LED_ON();
#endif

#if CONFIG_PIR_ENABLE
    while (1) {
        bool pir_high = pir_read(CONFIG_PIR_GPIO);

        if (pir_high && !pir_was_high) {
            ESP_LOGI(TAG_PIR, "Motion detected — capturing");
            LED_ON();
        } else if (!pir_high && pir_was_high) {
            ESP_LOGI(TAG_PIR, "PIR low — idle");
            LED_OFF();
        }
        pir_was_high = pir_high;

        if (!pir_high || !queue_frame()) {
            vTaskDelay(1);
        }
    }
#else
    while (1) {
        if (!queue_frame()) {
            vTaskDelay(1);
        }
    }
#endif
}
