#pragma once
#define __XCP_APPL_H__

// Additional functions for users of the XCP library xcplib

#include <stdbool.h> // for bool
#include <stdint.h>  // for uint32_t, uint64_t, uint8_t, int64_t

#include "xcpLite.h" // for tXcpDaqLists

extern void ApplXcpSetLogLevel(uint8_t level);

extern void ApplXcpRegisterCallbacks(uint8_t (*cb_connect)(void), uint8_t (*cb_prepare_daq)(const tXcpDaqLists *daq), uint8_t (*cb_start_daq)(const tXcpDaqLists *daq),
                                     void (*cb_stop_daq)(void), uint8_t (*cb_get_cal_page)(uint8_t segment, uint8_t mode),
                                     uint8_t (*cb_set_cal_page)(uint8_t segment, uint8_t page, uint8_t mode), uint8_t (*cb_freeze_cal)(void),
                                     uint8_t (*cb_init_cal)(uint8_t src_page, uint8_t dst_page), uint8_t (*cb_read)(uint32_t src, uint8_t size, uint8_t *dst),
                                     uint8_t (*cb_write)(uint32_t dst, uint8_t size, const uint8_t *src, uint8_t delay), uint8_t (*cb_flush)(void));

extern void ApplXcpSetA2lName(const char *name);
extern void ApplXcpSetEpk(const char *name);