#pragma once
#define __XCP_ETHTL_H__

/* Copyright(c) Vector Informatik GmbH.All rights reserved.
   Licensed under the MIT license.See LICENSE file in the project root for details. */

#include <stdbool.h>
#include <stdint.h>

#include "platform.h"  // for platform defines (WIN_, LINUX_, MACOS_) and specific implementation of sockets, clock, thread, mutex
#include "xcptl_cfg.h" // for XCPTL_xxx
#include "xcpQueue.h"  // for QueueXxxx, tQueueHandle

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

// Transport Layer functions called by protocol layer in xcpLite.c
bool XcpTlWaitForTransmitQueueEmpty(uint16_t timeout_ms);       // Wait (sleep) until transmit queue is empty, timeout after 1s return false
bool XcpTlNotifyTransmitQueueHandler(tQueueHandle queueHandle); // provider -> consumer event notificatoni

// Transport layer functions called by XCP server in xcpEthServer.c
uint8_t XcpTlCommand(uint16_t msgLen, const uint8_t *msgBuf); // Handle XCP message
bool XcpTlWaitForTransmitData(uint32_t timeout_ms);           // Wait for at least timeout_ms, until packets are pending in the transmit queue
int32_t XcpTlHandleTransmitQueue(void);                       // Send pending packets in the transmit queue
void XcpTlFlushTransmitQueue(void);

/* ETH transport Layer functions called by server */

bool XcpEthTlInit(const uint8_t *addr, uint16_t port, bool useTCP, bool blockingRx, tQueueHandle queue_handle); // Start transport layer
void XcpEthTlShutdown(void);
void XcpEthTlGetInfo(bool *isTCP, uint8_t *mac, uint8_t *addr, uint16_t *port);

/* Transmit a segment (contains multiple XCP DTO or CRO messages */
int XcpEthTlSend(const uint8_t *data, uint16_t size, const uint8_t *addr, uint16_t port);

/* ETH transport Layer functions called by server */
bool XcpEthTlHandleCommands(uint32_t timeout_ms); // Handle all incoming XCP commands, (wait for at least timeout_ms)

/* ETH transport Layer functions called by protocol layer */
#ifdef XCPTL_ENABLE_MULTICAST
void XcpEthTlSendMulticastCrm(const uint8_t *data, uint16_t n, const uint8_t *addr, uint16_t port); // Send multicast command response
void XcpEthTlSetClusterId(uint16_t clusterId);                                                      // Set cluster id for GET_DAQ_CLOCK_MULTICAST reception
#endif
