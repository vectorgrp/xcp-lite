#pragma once
#define __XCP_LITE_H__

/* Copyright(c) Vector Informatik GmbH.All rights reserved.
   Licensed under the MIT license.See LICENSE file in the project root for details. */

#include <stdbool.h> // for bool
#include <stdint.h>  // for uint16_t, uint32_t, uint8_t

#include "xcpQueue.h"
#include "xcp_cfg.h" // for XCP_PROTOCOL_LAYER_VERSION, XCP_ENABLE_DY...

/****************************************************************************/
/* Calibration segments                                                     */
/****************************************************************************/

#define XCP_UNDEFINED_CALSEG 0xFFFF

#ifdef XCP_ENABLE_CALSEG_LIST

#ifndef XCP_MAX_CALSEG_COUNT
#error "Please define XCP_MAX_CALSEG_COUNT!"
#endif

#ifndef XCP_MAX_CALSEG_NAME
#define XCP_MAX_CALSEG_NAME 15
#endif

// Page numbers for calibration segments
#define XCP_CALSEG_DEFAULT_PAGE 1 // FLASH page
#define XCP_CALSEG_WORKING_PAGE 0 // RAM page
#define XCP_CALSEG_INVALID_PAGE 0xFF

// Calibration segment
typedef struct {
    uint8_t *default_page;
    uint8_t *ecu_page;
    uint8_t *xcp_page;
    uint16_t size;
    uint16_t xcp_ctr;
    uint16_t ecu_ctr;
    uint8_t xcp_access; // page number for XCP access
    uint8_t ecu_access; // page number for ECU access
    char name[XCP_MAX_CALSEG_NAME + 1];
} tXcpCalSeg;

// Calibration segment list
typedef struct {
    MUTEX mutex;
    bool write_delay;
    uint16_t count;
    tXcpCalSeg calseg[XCP_MAX_CALSEG_COUNT];
} tXcpCalSegList;

#endif

/****************************************************************************/
/* DAQ events                                                               */
/****************************************************************************/

#define XCP_UNDEFINED_EVENT_CHANNEL 0xFFFF

#ifdef XCP_ENABLE_DAQ_EVENT_LIST

#ifndef XCP_MAX_EVENT_COUNT
#error "Please define XCP_MAX_EVENT_COUNT!"
#endif
#if XCP_MAX_EVENT_COUNT & 1 != 0
#error "XCP_MAX_EVENT_COUNT must be even!"
#endif

#ifndef XCP_MAX_EVENT_NAME
#define XCP_MAX_EVENT_NAME 15
#endif

typedef struct {
    uint16_t daqList; // associated DAQ list
    uint16_t res1;
    uint8_t timeUnit;                  // timeCycle unit, 1ns=0, 10ns=1, 100ns=2, 1us=3, ..., 1ms=6, ...
    uint8_t timeCycle;                 // cycle time in units, 0 = sporadic or unknown
    uint8_t priority;                  // priority 0 = queued, 1 = pushing, 2 = realtime
    uint8_t res2;                      // reserved
    char name[XCP_MAX_EVENT_NAME + 1]; // event name
} tXcpEvent;

typedef struct {
    MUTEX mutex;
    uint16_t count;                       // number of events
    tXcpEvent event[XCP_MAX_EVENT_COUNT]; // event list
} tXcpEventList;

#endif

/****************************************************************************/
/* DAQ information                                                          */
/****************************************************************************/

#define XCP_UNDEFINED_DAQ_LIST 0xFFFF

// ODT
// size = 8 byte
#pragma pack(push, 1)
typedef struct {
    uint16_t first_odt_entry; /* Absolute odt entry number */
    uint16_t last_odt_entry;  /* Absolute odt entry number */
    uint16_t size;            /* Number of bytes */
    uint16_t res;
} tXcpOdt;
#pragma pack(pop)
// static_assert(sizeof(tXcpOdt) == 8, "Error: size of tXcpOdt is not equal to 8");

/* DAQ list */
// size = 12 byte
#pragma pack(push, 1)
typedef struct {
    uint16_t last_odt;      /* Absolute odt number */
    uint16_t first_odt;     /* Absolute odt number */
    uint16_t event_channel; /* Associated event */
#ifdef XCP_MAX_EVENT_COUNT
    uint16_t next; /* Next DAQ list associated to event_channel */
#else
    uint16_t res1;
#endif
    uint8_t mode;
    uint8_t state;
    uint8_t priority;
    uint8_t addr_ext;
} tXcpDaqList;
#pragma pack(pop)
// static_assert(sizeof(tXcpDaqList) == 12, "Error: size of tXcpDaqList is not equal to 12");

/* Dynamic DAQ list structure in a linear memory block with size XCP_DAQ_MEM_SIZE + 8  */
#pragma pack(push, 1)
typedef struct {
    uint16_t odt_entry_count; // Total number of ODT entries in ODT entry addr and size arrays
    uint16_t odt_count;       // Total number of ODTs in ODT array
    uint16_t daq_count;       // Number of DAQ lists in DAQ list array
    uint16_t res;
#ifdef XCP_ENABLE_DAQ_RESUME
    uint16_t config_id;
    uint16_t res1;
#endif
#ifdef XCP_MAX_EVENT_COUNT
    uint16_t daq_first[XCP_MAX_EVENT_COUNT]; // Event channel to DAQ list mapping
#endif

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

        uint64_t b[XCP_DAQ_MEM_SIZE / 8 + 1];
    } u;

} tXcpDaqLists;
#pragma pack(pop)

/****************************************************************************/
/* Protocol layer interface                                                 */
/****************************************************************************/

/* Initialization for the XCP Protocol Layer */
void XcpInit(void);
bool XcpIsInitialized(void);
void XcpStart(tQueueHandle queueHandle, bool resumeMode);
void XcpReset(void);

/* XCP command processor */
uint8_t XcpCommand(const uint32_t *pCommand, uint8_t len);

/* Disconnect, stop DAQ, flush queue */
void XcpDisconnect(void);

/* Trigger a XCP data acquisition event */
void XcpTriggerDaqEventAt(const tXcpDaqLists *daq_lists, tQueueHandle queueHandle, uint16_t event, const uint8_t *base, uint64_t clock);
uint8_t XcpEventExtAt(uint16_t event, const uint8_t *base, uint64_t clock);
uint8_t XcpEventExt(uint16_t event, const uint8_t *base);
void XcpEventAt(uint16_t event, uint64_t clock);
void XcpEvent(uint16_t event);

/* Send an XCP event message */
void XcpSendEvent(uint8_t evc, const uint8_t *d, uint8_t l);

/* Send terminate session signal event */
void XcpSendTerminateSessionEvent(void);

/* Print log message via XCP */
#ifdef XCP_ENABLE_SERV_TEXT
void XcpPrint(const char *str);
#endif

/* Check status */
bool XcpIsStarted(void);
bool XcpIsConnected(void);
uint16_t XcpGetSessionStatus(void);
bool XcpIsDaqRunning(void);
bool XcpIsDaqEventRunning(uint16_t event);
uint64_t XcpGetDaqStartTime(void);
uint32_t XcpGetDaqOverflowCount(void);

/* Time synchronisation */
#ifdef XCP_ENABLE_DAQ_CLOCK_MULTICAST
#if XCP_PROTOCOL_LAYER_VERSION < 0x0103
#error "Protocol layer version must be >=0x0103"
#endif
uint16_t XcpGetClusterId(void);
#endif

/* Event list */
#ifdef XCP_ENABLE_DAQ_EVENT_LIST

// Add a measurement event to event list, return event number (0..MAX_EVENT-1)
uint16_t XcpCreateEvent(const char *name, uint32_t cycleTimeNs /* ns */, uint8_t priority /* 0-normal, >=1 realtime*/);

// Get event list
tXcpEventList *XcpGetEventList(void);

// Lookup event
tXcpEvent *XcpGetEvent(uint16_t event);

#endif

/* Calibration segment list */
#ifdef XCP_ENABLE_CALSEG_LIST

// Get a pointer to the list
tXcpCalSegList *XcpGetCalSegList(void);

// Create a calibration segmment
// Thread safe
// Returns the handle or XCP_UNDEFINED_CALSEG when out of memory
uint16_t XcpCreateCalSeg(const char *name, void *default_page, uint16_t size);

// Lock a calibration segment and return a pointer to the ECU page
uint8_t *XcpLockCalSeg(uint16_t calseg);

// Unlock a calibration segment
void XcpUnlockCalSeg(uint16_t calseg);

#endif

/****************************************************************************/
/* Protocol layer external dependencies                                     */
/****************************************************************************/

// All callback functions supplied by the application
// Must be thread save

/* Callbacks on connect, disconnect, measurement prepare, start and stop */
bool ApplXcpConnect(void);
void ApplXcpDisconnect(void);
#if XCP_PROTOCOL_LAYER_VERSION >= 0x0104
bool ApplXcpPrepareDaq(void);
#endif
void ApplXcpStartDaq(void);
void ApplXcpStopDaq(void);

/* Address conversions from A2L address to pointer and vice versa in absolute addressing mode */
#ifdef XCP_ENABLE_ABS_ADDRESSING
uint8_t *ApplXcpGetPointer(uint8_t xcpAddrExt, uint32_t xcpAddr); /* Create a pointer (uint8_t*) from xcpAddrExt and xcpAddr, returns NULL if no access */
uint32_t ApplXcpGetAddr(const uint8_t *p);                        // Calculate the xcpAddr address from a pointer
uint8_t *ApplXcpGetBaseAddr(void);                                // Get the base address for DAQ data access */
#endif

/* Read and write memory */
#ifdef XCP_ENABLE_APP_ADDRESSING
uint8_t ApplXcpReadMemory(uint32_t src, uint8_t size, uint8_t *dst);
uint8_t ApplXcpWriteMemory(uint32_t dst, uint8_t size, const uint8_t *src);
#endif

/* User command */
#ifdef XCP_ENABLE_USER_COMMAND
uint8_t ApplXcpUserCommand(uint8_t cmd);
#endif

/*
 Note 1:
   For DAQ performance and memory optimization:
   XCPlite DAQ tables do not store address extensions and do not use ApplXcpGetPointer(void), addr is stored as 32 Bit value and access is hardcoded by *(baseAddr+xcpAddr)
   All accessible DAQ data is within a 4GByte range starting at ApplXcpGetBaseAddr(void)
   Attempting to setup an ODT entry with address extension != XCP_ADDR_EXT_ABS, XCP_ADDR_EXT_DYN or XCP_ADDR_EXT_REL gives a CRC_ACCESS_DENIED error message

 Note 2:
   ApplXcpGetPointer may do address transformations according to active calibration page
*/

/* Switch calibration pages */
#ifdef XCP_ENABLE_CAL_PAGE
uint8_t ApplXcpSetCalPage(uint8_t segment, uint8_t page, uint8_t mode);
uint8_t ApplXcpGetCalPage(uint8_t segment, uint8_t mode);
uint8_t ApplXcpSetCalPage(uint8_t segment, uint8_t page, uint8_t mode);
#ifdef XCP_ENABLE_COPY_CAL_PAGE
uint8_t ApplXcpCopyCalPage(uint8_t srcSeg, uint8_t srcPage, uint8_t destSeg, uint8_t destPage);
#endif
#ifdef XCP_ENABLE_FREEZE_CAL_PAGE
uint8_t ApplXcpCalFreeze();
#endif
#endif

/* DAQ clock */
uint64_t ApplXcpGetClock64(void);
#define CLOCK_STATE_SYNCH_IN_PROGRESS (0)
#define CLOCK_STATE_SYNCH (1)
#define CLOCK_STATE_FREE_RUNNING (7)
#define CLOCK_STATE_GRANDMASTER_STATE_SYNCH (1 << 3)
uint8_t ApplXcpGetClockState(void);

#ifdef XCP_ENABLE_PTP
#define CLOCK_STRATUM_LEVEL_UNKNOWN 255
#define CLOCK_STRATUM_LEVEL_ARB 16 // unsychronized
#define CLOCK_STRATUM_LEVEL_UTC 0  // Atomic reference clock
#define CLOCK_EPOCH_TAI 0          // Atomic monotonic time since 1.1.1970 (TAI)
#define CLOCK_EPOCH_UTC 1          // Universal Coordinated Time (with leap seconds) since 1.1.1970 (UTC)
#define CLOCK_EPOCH_ARB 2          // Arbitrary (epoch unknown)
bool ApplXcpGetClockInfoGrandmaster(uint8_t *uuid, uint8_t *epoch, uint8_t *stratum);
#endif

/* DAQ resume */
#ifdef XCP_ENABLE_DAQ_RESUME

uint8_t ApplXcpDaqResumeStore(uint16_t config_id);
uint8_t ApplXcpDaqResumeClear(void);

#endif

/* Get info for GET_ID command (pointer to and length of data) */
/* Supports IDT_ASCII, IDT_ASAM_NAME, IDT_ASAM_PATH, IDT_ASAM_URL, IDT_ASAM_EPK and IDT_ASAM_UPLOAD */
/* Returns 0 if not available */
uint32_t ApplXcpGetId(uint8_t id, uint8_t *buf, uint32_t bufLen);

/* Read a chunk (offset,size) of the A2L file for upload */
/* Return false if out of bounds */
#ifdef XCP_ENABLE_IDT_A2L_UPLOAD // Enable A2L content upload to host (IDT_ASAM_UPLOAD)
bool ApplXcpReadA2L(uint8_t size, uint32_t offset, uint8_t *data);
#endif
