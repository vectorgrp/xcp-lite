#pragma once

/*----------------------------------------------------------------------------
| File:
|   xcp_cfg.h
|
| Description:
|   User configuration file for XCP protocol layer parameters
 ----------------------------------------------------------------------------*/


/*----------------------------------------------------------------------------*/
/* Version */

// Driver version (GET_COMM_MODE_INFO)
#define XCP_DRIVER_VERSION 0x01

// Protocol layer version
//#define XCP_PROTOCOL_LAYER_VERSION 0x0101
//#define XCP_PROTOCOL_LAYER_VERSION 0x0103  // GET_DAQ_CLOCK_MULTICAST, GET_TIME_CORRELATION_PROPERTIES
#define XCP_PROTOCOL_LAYER_VERSION 0x0104  // PACKED_MODE, CC_START_STOP_SYNCH prepare

/*----------------------------------------------------------------------------*/
/* Adress, address extension coding */

#define XCP_ADDR_EXT_ABS 0x01 // Absolute address format 

// Use addr_ext XCP_ADDR_EXT_DYN to indicate relative addr format (event<<16)|offset 
#define XCP_ENABLE_DYN_ADDRESSING
#define XCP_ADDR_EXT_DYN 0x02 // Relative address format

// Use addr_ext XCP_ADDR_EXT_APP to indicate application specific addr format and use ApplXcpReadMemory and ApplXcpWriteMemory
#define XCP_ENABLE_APP_ADDRESSING
#define XCP_ADDR_EXT_APP 0x00 // Address format handled by application

// Use addr_ext XCP_ADDR_EXT_A2L to indicate A2L upload memory space
#define XCP_ADDR_EXT_A2L 0xFF // Upload A2L address space

// Undefined
#define XCP_ADDR_EXT_UNDEFINED 0xFE // Undefined address extension

/*----------------------------------------------------------------------------*/
/* Protocol features */

#define XCP_ENABLE_CAL_PAGE // Enable calibration page switching commands
#ifdef XCP_ENABLE_CAL_PAGE
  #define XCP_ENABLE_COPY_CAL_PAGE // // Enable calibration page initialization (FLASH->RAM copy)
  #define XCP_ENABLE_FREEZE_CAL_PAGE // Enable calibration freeze command
#endif

#define XCP_ENABLE_CHECKSUM // Enable checksum calculation command

// #define XCP_ENABLE_SEED_KEY // Enable seed/key command

#define XCP_ENABLE_SERV_TEXT

/*----------------------------------------------------------------------------*/
/* GET_ID command */

// Uses addr_ext=0xFF to indicate addr space to upload A2L  
#define XCP_ENABLE_IDT_A2L_UPLOAD // Upload A2L via XCP enabled


/*----------------------------------------------------------------------------*/
/* User defined command */

// Used for begin and end atomic calibration operation
#define XCP_ENABLE_USER_COMMAND

/*----------------------------------------------------------------------------*/
/* DAQ features and parameters */

// No event list
// Rust xcp_lite has its own event handling
// #define XCP_ENABLE_DAQ_EVENT_INFO // Enable XCP_GET_EVENT_INFO, if this is enabled, A2L file event information will be ignored
// #define XCP_ENABLE_DAQ_EVENT_LIST // Enable event list
// #define XCP_MAX_EVENT 16 // Maximum number of events, size of event table
#ifdef XCP_ENABLE_DAQ_EVENT_LIST
  // #define XCP_ENABLE_MULTITHREAD_DAQ_EVENTS // Make XcpEventExt thread safe for same DAQ event coming from different threads
  // This should be very unusual, XcpEvent performance will be decreased
  // Requires event list, mutex is located in XcpEvent
#endif 

// Make XcpEvent thread safe for same CAL event coming from different threads
// Needed for xcp_lite, because CalSeg cal sync events may come from different threads
#define XCP_ENABLE_MULTITHREAD_CAL_EVENTS 

// Enable packed mode 
// #define XCP_ENABLE_PACKED_MODE 

// Memory available for DAQ
// Create static memory for DAQ tables
#define XCP_DAQ_MEM_SIZE (4096*2) // Amount of memory for DAQ tables, each ODT entry (e.g. measurement variable) needs 5 bytes

// Clock resolution
#define XCP_DAQ_CLOCK_32BIT  // Use 32 Bit time stamps

#if CLOCK_TICKS_PER_S == 1000000  // Settings for 32 bit us since application start (CLOCK_USE_APP_TIME_US)

  #define XCP_TIMESTAMP_UNIT DAQ_TIMESTAMP_UNIT_1US // unit DAQ_TIMESTAMP_UNIT_xxx
  #define XCP_TIMESTAMP_TICKS 1  // ticks per unit

#endif
#if CLOCK_TICKS_PER_S == 1000000000  // Settings for 32 bit ns since application start (CLOCK_USE_UTC_TIME_NS)

  #define XCP_TIMESTAMP_UNIT DAQ_TIMESTAMP_UNIT_1NS // unit DAQ_TIMESTAMP_UNIT_xxx
  #define XCP_TIMESTAMP_TICKS 1  // ticks per unit

#endif


//-------------------------------------------------------------------------------
// Debug 

// Enable extended error checks, performance penalty !!!
#define XCP_ENABLE_TEST_CHECKS

