#pragma once

/* Compose the device ID from board type and eFuse MAC.
   Must be called once at startup before device_id_get(). */
void device_id_init(void);

/* Returns "<board>-<mac6>", e.g. "esp32cam-a1b2c3".
   Empty string if device_id_init() hasn't run yet. */
const char *device_id_get(void);
