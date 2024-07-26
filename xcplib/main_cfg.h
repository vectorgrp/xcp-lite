#pragma once

// main_cfg.h
// XCPlite

/* Copyright(c) Vector Informatik GmbH.All rights reserved.
   Licensed under the MIT license.See LICENSE file in the project root for details. */


// When static library is used for Rust xcp_lite, consider the following options which are compiled into it
/*

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
#define OPTION_ENABLE_DBG_PRINTS        ON
#define OPTION_DEBUG_LEVEL              0


// No A2L generator, Rust xcp_lite has its own
// #define OPTION_ENABLE_A2L_GEN ON // Enable A2L generation


// Set clock resolution (for clock function in platform.c)
#define CLOCK_USE_APP_TIME_US
//#define CLOCK_USE_UTC_TIME_NS


// Ethernet Transport Layer
#define OPTION_USE_TCP                  OFF
#define OPTION_MTU                      8000            // Ethernet MTU
#define OPTION_SERVER_PORT              5555            // Default UDP port
#define OPTION_SERVER_ADDR              {127,0,0,1}     // Default IP addr to bind, 0.0.0.0 = ANY

