#pragma once
#define __MAIN_CFG_H__

/* main.h */
/*
| Code released into public domain, no attribution required
*/


/*
  XCP library build options:

  // Logging
  #define OPTION_ENABLE_DBG_PRINTS    Enable debug prints
  #define OPTION_DEFAULT_DEBUG_LEVEL  Default log level: 1 - Error, 2 - Warn, 3 - Info, 4 - Trace, 5 - Debug

  // Clock
  #define OPTION_CLOCK_EPOCH_ARB      arbitrary epoch 
  #define OPTION_CLOCK_EPOCH_PTP      since 1.1.1970
  #define OPTION_CLOCK_TICKS_1NS      resolution 1ns or 1us, granularity depends on platform
  #define OPTION_CLOCK_TICKS_1US

  // XCP  
  #define OPTION_ENABLE_TCP
  #define OPTION_ENABLE_UDP
  #define OPTION_MTU                  UDP MTU
  #define OPTION_QUEUE_SIZE           Size of the DAQ queue in XCP DTO/CRM packets (not messages as in V1.x) 
  #define OPTION_DAQ_MEM_SIZE         Size of memory for DAQ setup in bytes
  
*/

// Application configuration:
// More specific XCP configuration is in xcp_cfg.h (Protocol Layer) and xcptl_cfg.h (Transport Layer)

// Logging
#define OPTION_ENABLE_DBG_PRINTS 
#define OPTION_DEFAULT_DBG_LEVEL 4 

// XCP options
#define OPTION_ENABLE_TCP
#define OPTION_ENABLE_UDP
#define OPTION_MTU 1500 
#define OPTION_QUEUE_SIZE 200     
#define OPTION_DAQ_MEM_SIZE (3000*5)  

// Platform options
#define OPTION_CLOCK_EPOCH_ARB
#define OPTION_CLOCK_TICKS_1NS
 