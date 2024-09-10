#pragma once
#define __MAIN_CFG_H__

// main_cfg.h
// XCPlite

/* Copyright(c) Vector Informatik GmbH.All rights reserved.
   Licensed under the MIT license.See LICENSE file in the project root for details. */


// When static library xcplib is used for Rust xcp-lite, consider the following options which are compiled into it
/*

  main_cfg.h:
  XCP_ENABLE_DBG_PRINTS     // Enable debug prints
  XCP_DEFAULT_DEBUG_LEVEL   // Default debug level 1-5
  OPTION_MTU                // UDP MTU
  XCPTL_ENABLE_TCP
  XCPTL_ENABLE_UDP
  CLOCK_USE_APP_TIME_US or CLOCK_USE_UTC_TIME_NS // Clock resolution, TAI or ARB epoch

  xcptl_cfg.h:
  XCPTL_QUEUE_SIZE          // Allocate static memory for transmit queue, an entry has XCPTL_MAX_SEGMENT_SIZE bytes
  XCPTL_MAX_SEGMENT_SIZE    // Set to (OPTION_MTU-20-8) optimzed for the maximum possible UDP payload
   
  xcp_cfg.h:
  XCP_DAQ_MEM_SIZE          // Allocate static meory for DAQ tables
  CLOCK_TICKS_PER_S         // Resolution of the DAQ clock

*/

// Application configuration:
// XCP configuration is in xcp_cfg.h (Protocol Layer) and xcptl_cfg.h (Transport Layer)

#define ON 1
#define OFF 0

// Debug prints
#define XCP_ENABLE_DBG_PRINTS
#define XCP_DEFAULT_DEBUG_LEVEL 2 /*1 - Error, 2 - Warn, 3 - Info, 4 - Trace, 5 - Debug */

// Set clock resolution (for clock function in platform.c)
#define CLOCK_USE_APP_TIME_US
// #define CLOCK_USE_UTC_TIME_NS


// Ethernet Transport Layer
#define OPTION_MTU 8000 // Ethernet MTU 

// Ethernet Server
// TCP or/and UDP server enabled
#define XCPTL_ENABLE_TCP
#define XCPTL_ENABLE_UDP
// #define XCP_SERVER_FORCEFULL_TERMINATION // Otherwise use gracefull server thread termination in xcplib


// #define PLATFORM_ENABLE_GET_LOCAL_ADDR
// #define PLATFORM_ENABLE_KEYBOARD
