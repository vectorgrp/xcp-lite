#pragma once
/* xcpTl.h */

/* Copyright(c) Vector Informatik GmbH.All rights reserved.
   Licensed under the MIT license.See LICENSE file in the project root for details. */



// Parameter checks
#if XCPTL_TRANSPORT_LAYER_HEADER_SIZE != 4
#error "Transportlayer supports only 4 byte headers!"
#endif
#if ((XCPTL_MAX_CTO_SIZE&0x07) != 0)
#error "XCPTL_MAX_CTO_SIZE should be aligned to 8!"
#endif
#if ((XCPTL_MAX_DTO_SIZE&0x03) != 0)
#error "XCPTL_MAX_DTO_SIZE should be aligned to 4!"
#endif

#pragma pack(push, 1)
typedef struct {
    uint16_t dlc;    // XCP TL header lenght
    uint16_t ctr;    // XCP TL Header message counter
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

// Transport Layer functions called by protocol layer in XCPlite.c 
extern void XcpTlSendCrm(const uint8_t* data, uint16_t n); // Queue a command response
extern uint8_t* XcpTlGetTransmitBuffer(void** handle, uint16_t size); // Get a buffer for a message with size
extern void XcpTlCommitTransmitBuffer(void* handle, BOOL flush); // Commit a buffer (by handle returned from XcpTlGetTransmitBuffer)
extern void XcpTlFlushTransmitBuffer(); // Finalize the current transmit packet (ETH only)
extern BOOL XcpTlWaitForTransmitQueueEmpty(uint16_t timeout_ms); // Wait (sleep) until transmit queue is empty, timeout after 1s return FALSE 

// Transport layer functions called by the transport layer queue (provider -> consumer event)
extern BOOL XcpTlNotifyTransmitQueueHandler();

// Transport layer functions called by XCP server
extern BOOL XcpTlInit(); // Start generic transport layer
extern void XcpTlShutdown(); // Stop generic transport layer
extern uint8_t XcpTlCommand(uint16_t msgLen, const uint8_t* msgBuf); // Handle XCP message
extern const uint8_t* XcpTlTransmitQueuePeekMsg(uint16_t* msg_len);  // Check if there is a fully commited message segment buffer in the transmit queue
extern void XcpTlTransmitQueueNextMsg(); // Remove the next transmit queue entry
extern BOOL XcpTlTransmitQueueHasMsg(); // Check if there is enough data in the transmit queue for a transport layer message

extern int32_t XcpTlHandleTransmitQueue(); // Send all outgoing packets in the transmit queue
extern BOOL XcpTlWaitForTransmitData(uint32_t timeout_ms); // Wait for at least timeout_ms, until packets are pending in the transmit queue





