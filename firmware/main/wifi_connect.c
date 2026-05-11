#include "wifi_connect.h"
#include "sdkconfig.h"
#include "esp_wifi.h"
#include "esp_event.h"
#include "esp_log.h"
#include "nvs_flash.h"
#include "freertos/FreeRTOS.h"
#include "freertos/event_groups.h"
#include "freertos/timers.h"
#include <string.h>

#define WIFI_CONNECTED_BIT BIT0
#define WIFI_FAIL_BIT      BIT1

/* Delay between reconnect attempts to prevent tight-loop hammering the WiFi
 * driver, which can cause it to stop issuing disconnect events entirely. */
#define RECONNECT_DELAY_MS 3000

static const char *TAG = "wifi";
static EventGroupHandle_t s_wifi_event_group;
static TimerHandle_t      s_reconnect_timer;
static int  s_initial_retries = 0;
static bool s_have_ip         = false;

static void reconnect_timer_cb(TimerHandle_t t)
{
    (void)t;
    esp_wifi_connect();
}

static void event_handler(void *arg, esp_event_base_t event_base,
                           int32_t event_id, void *event_data)
{
    if (event_base == WIFI_EVENT && event_id == WIFI_EVENT_STA_START) {
        esp_wifi_connect();
    } else if (event_base == WIFI_EVENT && event_id == WIFI_EVENT_STA_DISCONNECTED) {
        wifi_event_sta_disconnected_t *evt =
            (wifi_event_sta_disconnected_t *)event_data;
        /* Bound retries during initial connect so wifi_connect() can fail
         * fast on bad credentials. After we've ever obtained an IP, keep
         * reconnecting forever so a transient outage doesn't stop the cam. */
        if (s_have_ip) {
            ESP_LOGW(TAG, "Disconnected (reason=%d), reconnecting in %ds...",
                     evt->reason, RECONNECT_DELAY_MS / 1000);
            xTimerStart(s_reconnect_timer, 0);
        } else if (s_initial_retries < CONFIG_WIFI_MAX_RETRIES) {
            s_initial_retries++;
            ESP_LOGW(TAG, "Disconnected (reason=%d), retry %d/%d",
                     evt->reason, s_initial_retries, CONFIG_WIFI_MAX_RETRIES);
            xTimerStart(s_reconnect_timer, 0);
        } else {
            ESP_LOGE(TAG, "Initial connect failed after %d retries (reason=%d)",
                     CONFIG_WIFI_MAX_RETRIES, evt->reason);
            xEventGroupSetBits(s_wifi_event_group, WIFI_FAIL_BIT);
        }
    } else if (event_base == IP_EVENT && event_id == IP_EVENT_STA_GOT_IP) {
        ip_event_got_ip_t *event = (ip_event_got_ip_t *)event_data;
        ESP_LOGI(TAG, "Got IP: " IPSTR, IP2STR(&event->ip_info.ip));
        s_have_ip = true;
        s_initial_retries = 0;
        xTimerStop(s_reconnect_timer, 0);
        xEventGroupSetBits(s_wifi_event_group, WIFI_CONNECTED_BIT);
    }
}

esp_err_t wifi_connect(void)
{
    esp_err_t ret = nvs_flash_init();
    if (ret == ESP_ERR_NVS_NO_FREE_PAGES || ret == ESP_ERR_NVS_NEW_VERSION_FOUND) {
        nvs_flash_erase();
        ret = nvs_flash_init();
    }
    ESP_ERROR_CHECK(ret);

    s_wifi_event_group = xEventGroupCreate();
    s_reconnect_timer  = xTimerCreate("wifi_rc", pdMS_TO_TICKS(RECONNECT_DELAY_MS),
                                      pdFALSE, NULL, reconnect_timer_cb);
    assert(s_reconnect_timer);

    ESP_ERROR_CHECK(esp_netif_init());
    ESP_ERROR_CHECK(esp_event_loop_create_default());
    esp_netif_create_default_wifi_sta();

    wifi_init_config_t cfg = WIFI_INIT_CONFIG_DEFAULT();
    ESP_ERROR_CHECK(esp_wifi_init(&cfg));

    ESP_ERROR_CHECK(esp_event_handler_instance_register(
        WIFI_EVENT, ESP_EVENT_ANY_ID, &event_handler, NULL, NULL));
    ESP_ERROR_CHECK(esp_event_handler_instance_register(
        IP_EVENT, IP_EVENT_STA_GOT_IP, &event_handler, NULL, NULL));

    wifi_config_t wifi_config = {
        .sta = {
            .ssid     = CONFIG_WIFI_SSID,
            .password = CONFIG_WIFI_PASSWORD,
            .threshold.authmode = WIFI_AUTH_WPA2_PSK,
        },
    };

    ESP_ERROR_CHECK(esp_wifi_set_mode(WIFI_MODE_STA));
    ESP_ERROR_CHECK(esp_wifi_set_config(WIFI_IF_STA, &wifi_config));
    ESP_ERROR_CHECK(esp_wifi_start());

    ESP_LOGI(TAG, "Connecting to \"%s\"...", CONFIG_WIFI_SSID);

    EventBits_t bits = xEventGroupWaitBits(s_wifi_event_group,
                                            WIFI_CONNECTED_BIT | WIFI_FAIL_BIT,
                                            pdFALSE, pdFALSE,
                                            portMAX_DELAY);

    if (bits & WIFI_CONNECTED_BIT) {
        return ESP_OK;
    }

    ESP_LOGE(TAG, "Failed to connect to WiFi");
    return ESP_FAIL;
}

void wifi_disconnect(void)
{
    esp_wifi_disconnect();
    esp_wifi_stop();
}
