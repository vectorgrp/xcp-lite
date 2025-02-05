#pragma once
#define __XCP_TL_H__

/* Copyright(c) Vector Informatik GmbH.All rights reserved.
   Licensed under the MIT license.See LICENSE file in the project root for details. */

#include <stdbool.h>
#include <stdint.h>

#include "src/xcptl_cfg.h" // for XCPTL_xxx
#include "src/xcpQueue.h"  // for QueueXxxx, tQueueHandle

// Parameter checks
#if XCPTL_TRANSPORT_LAYER_HEADER_SIZE != 4
#error "Transportlayer supports only 4 byte headers!"
#endif
#if ((XCPTL_MAX_CTO_SIZE & 0x07) != 0)
#error "XCPTL_MAX_CTO_SIZE should be aligned to 8!"
#endif
#if ((XCPTL_MAX_DTO_SIZE & 0x03) != 0)
#error "XCPTL_MAX_DTO_SIZE should be aligned to 4!"
#endif

#pragma pack(push, 1)
typedef struct {
    uint16_t dlc; // XCP TL header lenght
    uint16_t ctr; // XCP TL Header message counter
    uint8_t data[];
} tXcpDtoMessage;
#pragma pack(pop)

#pragma pack(push, 1)
typedef struct {
    uint16_t dlc;
    uint16_t ctr;
    uint8_t packet[XCPTL_MAX_CTO_SIZE];
} tXcpCtoMessage;
#pragma pack(pop)

#define XCPTL_TIMEOUT_INFINITE 0xFFFFFFFF // Infinite timeout (blocking mode) for XcpTlHandleCommands, XcpTlWaitForTransmitData

extern tQueueHandle gQueueHandle; // @@@@

// Transport Layer functions called by protocol layer in XCPlite.c
extern bool XcpTlWaitForTransmitQueueEmpty(uint16_t timeout_ms); // Wait (sleep) until transmit queue is empty, timeout after 1s return false

// Transport layer functions called by the transport layer queue (provider -> consumer event)
extern bool XcpTlNotifyTransmitQueueHandler(void);

// Transport layer functions called by XCP server
extern bool XcpTlInit(uint32_t queueSize);                           // Start generic transport layer
extern void XcpTlShutdown(void);                                     // Stop generic transport layer
extern uint8_t XcpTlCommand(uint16_t msgLen, const uint8_t *msgBuf); // Handle XCP message
extern int32_t XcpTlHandleTransmitQueue(void);                       // Send all outgoing packets in the transmit queue
extern bool XcpTlWaitForTransmitData(uint32_t timeout_ms);           // Wait for at least timeout_ms, until packets are pending in the transmit queue
