#pragma once

#include "esp_err.h"
#include <stdint.h>
#include <stddef.h>

esp_err_t http_send_frame(const uint8_t *data, size_t len);
