#pragma once
#define __MAIN_CFG_H__

/*
| Build options for XCP or xcpliv
|
| Code released into public domain, no attribution required
*/

/*
  XCP library build options:

  // Logging
  #define OPTION_ENABLE_DBG_PRINTS    Enable debug prints
  #define OPTION_DEFAULT_DBG_LEVEL  Default log level: 1 - Error, 2 - Warn, 3 - Info, 4 - Trace, 5 - Debug

  // Clock
  #define OPTION_CLOCK_EPOCH_ARB      Arbitrary epoch or since 1.1.1970
  #define OPTION_CLOCK_EPOCH_PTP
  #define OPTION_CLOCK_TICKS_1NS      Resolution 1ns or 1us, granularity depends on platform
  #define OPTION_CLOCK_TICKS_1US

  // XCP
  #define OPTION_ENABLE_TCP
  #define OPTION_ENABLE_UDP
  #define OPTION_MTU                        UDP MTU
  #define OPTION_QUEUE_SIZE                 Size of the DAQ queue in XCP DTO/CRM packets (not messages as in V1.x)
  #define OPTION_DAQ_MEM_SIZE               Size of memory for DAQ setup in bytes
  #define OPTION_ENABLE_A2L_UPLOAD          Enable GET_ID A2L upload
  #define OPTION_ENABLE_GET_LOCAL_ADDR      Determine an existing IP address for A2L file, if bound to ANY
  #define OPTION_SERVER_FORCEFULL_TERMINATION  Terminate the server threads instead of waiting for the tasks to finish
*/

// Application configuration:
// (More specific XCP configuration is in xcp_cfg.h (Protocol Layer) and xcptl_cfg.h (Transport Layer))

// XCP options
#define OPTION_ENABLE_TCP
#define OPTION_ENABLE_UDP
#define OPTION_MTU 8000                // UDP MTU size - Jumbo frames supported
#define OPTION_QUEUE_SIZE 1024         // Max number of ODTs in transmit queue
#define OPTION_DAQ_MEM_SIZE (3000 * 5) // Max memory for DAQ tables - sufficient for about 3000 measurement signals
#define OPTION_ENABLE_A2L_UPLOAD
#define OPTION_SERVER_FORCEFULL_TERMINATION

// Platform options
// Clock
#define OPTION_CLOCK_EPOCH_ARB
#define OPTION_CLOCK_TICKS_1NS

// Enable socketGetLocalAddr and XcpEthTlGetInfo
// Used for convenience to get a correct ip addr in A2L, when bound to ANY 0.0.0.0
#define OPTION_ENABLE_GET_LOCAL_ADDR

// Logging
#define OPTION_ENABLE_DBG_PRINTS
#define OPTION_DEFAULT_DBG_LEVEL 3
