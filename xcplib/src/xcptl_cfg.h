#pragma once
#define __XCPTL_CFG_H__

/*----------------------------------------------------------------------------
| File:
|   xcptl_cfg.h
|
| Description:
|   Parameter configuration for XCP transport layer
|
| Code released into public domain, no attribution required
 ----------------------------------------------------------------------------*/

#include "main_cfg.h" // for OPTION_xxx

#if defined(OPTION_ENABLE_UDP)
#define XCPTL_ENABLE_UDP
#endif
#if defined(OPTION_ENABLE_TCP)
#define XCPTL_ENABLE_TCP
#endif

// Transport layer version
#define XCP_TRANSPORT_LAYER_VERSION 0x0104

// CTO size
// Maximum size of a XCP command packet (CRO,CRM)
#define XCPTL_MAX_CTO_SIZE (248) // Prefer %8=0 over the maximum value of 255 for better allignment and granularities

// DTO size
// Maximum size of a XCP data packet (DAQ,STIM)
#define XCPTL_MAX_DTO_SIZE (XCPTL_MAX_SEGMENT_SIZE - 8) // Segment size - XCP transport layer header size, size must be mod 8
// #define XCPTL_MAX_DTO_SIZE (248) // Segment size - XCP transport layer header size, size must be mod 8

// Segment size is the maximum data buffer size given to sockets send/sendTo, for UDP it is the UDP MTU
// Jumbo frames are supported, but it might be more efficient to use a smaller segment sizes
#ifdef OPTION_MTU
#define XCPTL_MAX_SEGMENT_SIZE (OPTION_MTU - 20 - 8) // UDP MTU (MTU - IP-header - UDP-header)
#else
#error "Please define XCPTL_MAX_SEGMENT_SIZE"
#define XCPTL_MAX_SEGMENT_SIZE (1500 - 20 - 8)
#endif

// Alignment for packet concatenation
#define XCPTL_PACKET_ALIGNMENT 4 // Packet alignment for multiple XCP transport layer packets in a XCP transport layer message

// Maximum queue (producer->consumer) event rate (Windows only, Linux uses polling on the consumer side)
#define XCPTL_QUEUE_TRANSMIT_CYCLE_TIME (1 * CLOCK_TICKS_PER_MS)

// Flush cycle
#define XCPTL_QUEUE_FLUSH_CYCLE_MS 100 // Send a DTO packet at least every x ms, XCPTL_TIMEOUT_INFINITE to turn off

// Transport layer message header size
// This is fixed, no other options supported
#define XCPTL_TRANSPORT_LAYER_HEADER_SIZE 4

// Multicast (GET_DAQ_CLOCK_MULTICAST)
// #define XCPTL_ENABLE_MULTICAST
/*
Use multicast time synchronisation to improve synchronisation of multiple XCP slaves
This option is available since XCP V1.3, but using it, needs to create an additional thread and socket for multicast reception
There is no benefit if PTP time synchronized is used or if there is only one XCP device
Older CANape versions expect this option is on by default -> turn it off in device/protocol/event/TIME_CORRELATION_GETDAQCLOCK by changing from "multicast" to "extendedresponse"
*/
#if defined(XCPTL_ENABLE_UDP) || defined(XCPTL_ENABLE_TCP)
#ifdef XCPTL_ENABLE_MULTICAST
// #define XCLTL_RESTRICT_MULTICAST
#define XCPTL_MULTICAST_PORT 5557
#endif
#endif
