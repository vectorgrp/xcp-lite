#pragma once
/* xcpLite.h */

/* Copyright(c) Vector Informatik GmbH.All rights reserved.
   Licensed under the MIT license.See LICENSE file in the project root for details. */

#ifdef __XCPTL_CFG_H__
#error "Include dependency error!"
#endif
#ifdef __XCP_CFG_H__
#error "Include dependency error!"
#endif

#include "xcptl_cfg.h"  // Transport layer configuration

// Transport layer definitions and configuration
#include "xcpTl.h" 
#include "xcpEthTl.h"  // Ethernet transport layer specific functions

// Protocol layer definitions and configuration
#include "xcp_cfg.h"    // Protocol layer configuration
#include "xcp.h"        // XCP protocol defines



/****************************************************************************/
/* DAQ event channel information                                            */
/****************************************************************************/

#define XCP_UNDEFINED_EVENT_CHANNEL 0xFFFF

#ifdef XCP_ENABLE_DAQ_EVENT_LIST
  #ifndef XCP_MAX_EVENT_COUNT
    #define XCP_MAX_EVENT_COUNT 16
  #elif XCP_MAX_EVENT_COUNT > 16
    #warning "Memory consumption of event list is high, consider reducing XCP_MAX_EVENT_COUNT or XCP_MAX_EVENT_NAME"
  #endif
  #ifndef XCP_MAX_EVENT_NAME
    #define XCP_MAX_EVENT_NAME 8
  #endif

typedef struct {
    char shortName[XCP_MAX_EVENT_NAME+1]; // A2L XCP IF_DATA short event name, long name not supported
    uint32_t size; // ext event size
    uint8_t timeUnit; // timeCycle unit, 1ns=0, 10ns=1, 100ns=2, 1us=3, ..., 1ms=6, ...
    uint8_t timeCycle; // cycletime in units, 0 = sporadic or unknown
    uint16_t sampleCount; // packed event sample count
    uint16_t daqList; // associated DAQ list
    uint8_t priority; // priority 0 = queued, 1 = pushing, 2 = realtime

#ifdef XCP_ENABLE_MULTITHREAD_DAQ_EVENTS
    MUTEX mutex;
#endif
#ifdef XCP_ENABLE_TIMESTAMP_CHECK
    uint64_t time; // last event time stamp
#endif
} tXcpEvent;

#endif

/****************************************************************************/
/* DAQ information                                                          */
/****************************************************************************/

#define XCP_UNDEFINED_DAQ_LIST 0xFFFF

// ODT
// size = 8 byte
#pragma pack(push, 1)
typedef struct {
    uint16_t first_odt_entry;       /* Absolute odt entry number */
    uint16_t last_odt_entry;        /* Absolute odt entry number */
    uint16_t size;                  /* Number of bytes */
    uint16_t res;
} tXcpOdt;
#pragma pack(pop)

/* DAQ list */
// size = 12 byte
#pragma pack(push, 1)
typedef struct {
    uint16_t last_odt;        /* Absolute odt number */
    uint16_t first_odt;       /* Absolute odt number */
    uint16_t event_channel;   /* Associated event */
#ifdef XCP_MAX_EVENT_COUNT
  #if XCP_MAX_EVENT_COUNT & 1 != 0
    #error "XCP_MAX_EVENT_COUNT must be even!"
  #endif
    uint16_t next;            /* Next DAQ list associated to event_channel */
#else
    uint16_t res1;
#endif
    uint8_t mode;
    uint8_t state;
    uint8_t priority;
    uint8_t addr_ext;
} tXcpDaqList;
#pragma pack(pop)

/* Dynamic DAQ list structure in a linear memory block with size XCP_DAQ_MEM_SIZE + 8  */
#pragma pack(push, 1)
typedef struct {
    uint16_t odt_entry_count; // Total number of ODT entries in ODT entry addr and size arrays
    uint16_t odt_count; // Total number of ODTs in ODT array
    uint16_t daq_count; // Number of DAQ lists in DAQ list array
    uint16_t res;
#ifdef XCP_MAX_EVENT_COUNT
    uint16_t daq_first[XCP_MAX_EVENT_COUNT]; // Event channel to DAQ list mapping
#endif

    // Pointers to optimize access to DAQ lists, ODT and ODT entry array pointers
    int32_t* odt_entry_addr; // ODT entry addr array
    uint8_t* odt_entry_size; // ODT entry size array
    tXcpOdt* odt; // ODT array
    void *res2;

    // DAQ array
    // size and alignment % 8 
    // memory layout:
    //  tXcpDaqList[] - DAQ list array
    //  tXcpOdt[]     - ODT array
    //  uint32_t[]    - ODT entry addr array
    //  uint8_t[]     - ODT entry size array
    union {
        // DAQ array
        tXcpDaqList daq_list[XCP_DAQ_MEM_SIZE / sizeof(tXcpDaqList)];
        // ODT array
        tXcpOdt odt[XCP_DAQ_MEM_SIZE / sizeof(tXcpOdt)];
        // ODT entry addr array
        uint32_t odt_entry_addr[XCP_DAQ_MEM_SIZE / 4];
        // ODT entry size array
        uint8_t odt_entry_size[XCP_DAQ_MEM_SIZE / 1];        

        uint64_t b[XCP_DAQ_MEM_SIZE/8+1];        
    } u;

} tXcpDaqLists;
#pragma pack(pop)


/****************************************************************************/
/* Protocol layer interface                                                 */
/****************************************************************************/

/* Initialization for the XCP Protocol Layer */
extern void XcpInit();
extern void XcpStart();
extern void XcpReset();

/* XCP command processor */
extern uint8_t XcpCommand( const uint32_t* pCommand, uint8_t len );

/* Disconnect, stop DAQ, flush queue */
extern void XcpDisconnect();

/* Trigger a XCP data acquisition event */
extern void XcpTriggerDaqEventAt(const tXcpDaqLists* daq_lists, uint16_t event, const uint8_t* base, uint64_t clock);
extern uint8_t XcpEventExtAt(uint16_t event, const uint8_t* base, uint64_t clock);
extern uint8_t XcpEventExt(uint16_t event, const uint8_t* base);
extern void XcpEventAt(uint16_t event, uint64_t clock); 
extern void XcpEvent(uint16_t event); 

/* Send an XCP event message */
extern void XcpSendEvent(uint8_t evc, const uint8_t* d, uint8_t l);

/* Send terminate session signal event */
extern void XcpSendTerminateSessionEvent();

/* Print log message via XCP */
#ifdef XCP_ENABLE_SERV_TEXT
extern void XcpPrint( const char *str);
#endif

/* Check status */
extern BOOL XcpIsStarted();
extern BOOL XcpIsConnected();
extern uint16_t XcpGetSessionStatus();
extern BOOL XcpIsDaqRunning();
extern BOOL XcpIsDaqEventRunning(uint16_t event);
extern uint64_t XcpGetDaqStartTime();
extern uint32_t XcpGetDaqOverflowCount();

/* Time synchronisation */
#ifdef XCP_ENABLE_DAQ_CLOCK_MULTICAST
  #if XCP_PROTOCOL_LAYER_VERSION < 0x0103
    #error "Protocol layer version must be >=0x0103"
  #endif
extern uint16_t XcpGetClusterId();
#endif

/* Event list */
#ifdef XCP_ENABLE_DAQ_EVENT_LIST

// Clear event list
extern void XcpClearEventList();
// Add a measurement event to event list, return event number (0..MAX_EVENT-1)
extern uint16_t XcpCreateEvent(const char* name, uint32_t cycleTimeNs /* ns */, uint8_t priority /* 0-normal, >=1 realtime*/, uint16_t sampleCount, uint32_t size);
// Get event list
extern tXcpEvent* XcpGetEventList(uint16_t* eventCount);
// Lookup event
extern tXcpEvent* XcpGetEvent(uint16_t event);

#endif


/****************************************************************************/
/* Protocol layer external dependencies                                     */
/****************************************************************************/

// All callback functions supplied by the application
// Must be thread save

/* Callbacks on connect, disconnect, measurement prepare, start and stop */
extern BOOL ApplXcpConnect();
extern void ApplXcpDisconnect();
#if XCP_PROTOCOL_LAYER_VERSION >= 0x0104
extern BOOL ApplXcpPrepareDaq(const tXcpDaqLists *daq);
#endif
extern void ApplXcpStartDaq(const tXcpDaqLists *daq);
extern void ApplXcpStopDaq();

/* Address conversions from A2L address to pointer and vice versa in absolute addressing mode */
#ifdef XCP_ENABLE_ABS_ADDRESSING
extern uint8_t* ApplXcpGetPointer(uint8_t xcpAddrExt, uint32_t xcpAddr); /* Create a pointer (uint8_t*) from xcpAddrExt and xcpAddr, returns NULL if no access */
extern uint32_t ApplXcpGetAddr(const uint8_t* p); // Calculate the xcpAddr address from a pointer
extern uint8_t *ApplXcpGetBaseAddr(); // Get the base address for DAQ data access */
#endif

/* Read and write memory */
#ifdef XCP_ENABLE_APP_ADDRESSING
extern uint8_t ApplXcpReadMemory(uint32_t src, uint8_t size, uint8_t* dst);
extern uint8_t ApplXcpWriteMemory(uint32_t dst, uint8_t size, const uint8_t* src);
#endif

/* User command */
#ifdef XCP_ENABLE_USER_COMMAND
extern uint8_t ApplXcpUserCommand(uint8_t cmd);
#endif

/*
 Note 1:
   For DAQ performance and memory optimization:
   XCPlite DAQ tables do not store address extensions and do not use ApplXcpGetPointer(), addr is stored as 32 Bit value and access is hardcoded by *(baseAddr+xcpAddr)
   All accesible DAQ data is within a 4GByte range starting at ApplXcpGetBaseAddr()
   Attempting to setup an ODT entry with address extension != XCP_ADDR_EXT_ABS or XCP_ADDR_EXT_DYN gives a CRC_ACCESS_DENIED error message

 Note 2:
   ApplXcpGetPointer may do address transformations according to active calibration page
*/


/* Switch calibration pages */
#ifdef XCP_ENABLE_CAL_PAGE
extern uint8_t ApplXcpSetCalPage(uint8_t segment, uint8_t page, uint8_t mode);
extern uint8_t ApplXcpGetCalPage(uint8_t segment, uint8_t mode);
extern uint8_t ApplXcpSetCalPage(uint8_t segment, uint8_t page, uint8_t mode);
#ifdef XCP_ENABLE_COPY_CAL_PAGE
extern uint8_t ApplXcpCopyCalPage(uint8_t srcSeg, uint8_t srcPage, uint8_t destSeg, uint8_t destPage);
#endif
#ifdef XCP_ENABLE_FREEZE_CAL_PAGE
extern uint8_t ApplXcpFreezeCalPage(uint8_t segment);
#endif
#endif
 
 
/* DAQ clock */
extern uint64_t ApplXcpGetClock64();
#define CLOCK_STATE_SYNCH_IN_PROGRESS                  (0)
#define CLOCK_STATE_SYNCH                              (1)
#define CLOCK_STATE_FREE_RUNNING                       (7)
#define CLOCK_STATE_GRANDMASTER_STATE_SYNCH             (1 << 3)
extern uint8_t ApplXcpGetClockState();

#ifdef XCP_ENABLE_PTP
#define CLOCK_STRATUM_LEVEL_UNKNOWN   255
#define CLOCK_STRATUM_LEVEL_ARB       16   // unsychronized
#define CLOCK_STRATUM_LEVEL_UTC       0    // Atomic reference clock
#define CLOCK_EPOCH_TAI 0 // Atomic monotonic time since 1.1.1970 (TAI)
#define CLOCK_EPOCH_UTC 1 // Universal Coordinated Time (with leap seconds) since 1.1.1970 (UTC)
#define CLOCK_EPOCH_ARB 2 // Arbitrary (epoch unknown)
extern BOOL ApplXcpGetClockInfoGrandmaster(uint8_t* uuid, uint8_t* epoch, uint8_t* stratum);
#endif

/* Get info for GET_ID command (pointer to and length of data) */
/* Supports IDT_ASCII, IDT_ASAM_NAME, IDT_ASAM_PATH, IDT_ASAM_URL, IDT_ASAM_EPK and IDT_ASAM_UPLOAD */
/* Returns 0 if not available */
extern uint32_t ApplXcpGetId(uint8_t id, uint8_t* buf, uint32_t bufLen);

/* Read a chunk (offset,size) of the A2L file for upload */
/* Return FALSE if out of bounds */
#ifdef XCP_ENABLE_IDT_A2L_UPLOAD // Enable A2L content upload to host (IDT_ASAM_UPLOAD)
extern BOOL ApplXcpReadA2L(uint8_t size, uint32_t offset, uint8_t* data);
#endif

