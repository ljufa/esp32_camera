#pragma once
#include "sdkconfig.h"

#if CONFIG_OTA_ENABLE
void ota_check_and_update(void);
#else
static inline void ota_check_and_update(void) {}
#endif
