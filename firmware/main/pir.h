#pragma once

#include "esp_err.h"
#include <stdbool.h>

esp_err_t pir_init(int gpio_num);
bool pir_read(int gpio_num);
