/*----------------------------------------------------------------------------
| File:
|   xcpAppl.c
|
| Description:
|   Application specific functions and callbacks for XCP
|   Additional functions for xcplib interface
|
| Copyright (c) Vector Informatik GmbH. All rights reserved.
| Licensed under the MIT license. See LICENSE file in the project root for details.
|
 ----------------------------------------------------------------------------*/

#include "xcpAppl.h"

#include <assert.h>  // for assert
#include <stdbool.h> // for bool
#include <stdint.h>  // for uint8_t, uint32_t, uint64_t
#include <stdio.h>   // for fclose, fopen, fread, fseek, ftell
#include <string.h>  // for strlen, strncpy

#include "dbg_print.h" // for DBG_PRINTF3, DBG_PRINT4, DBG_PRINTF4, DBG...
#include "main_cfg.h"  // for OPTION_xxx
#include "platform.h"  // for platform defines (WIN_, LINUX_, MACOS_) and specific implementation of sockets, clock, thread, mutex
#include "xcp.h"       // for CRC_XXX
#include "xcpLite.h"   // for tXcpDaqLists, XcpXxx, ApplXcpXxx, ...
#include "xcp_cfg.h"   // for XCP_ENABLE_xxx

#if !defined(_WIN) && !defined(_LINUX) && !defined(_MACOS)
#error "Please define platform _WIN, _MACOS or _LINUX"
#endif

// @@@@  TODO improve
#ifdef XCP_ENABLE_USER_COMMAND
static bool write_delay = false;
#endif

/**************************************************************************/
// Logging
/**************************************************************************/

#ifdef OPTION_ENABLE_DBG_PRINTS
uint8_t gDebugLevel = OPTION_DEFAULT_DBG_LEVEL;
#endif

// This is used by the Rust ffi to set the log level
void ApplXcpSetLogLevel(uint8_t level) {
#ifdef OPTION_ENABLE_DBG_PRINTS
    if (level > 3)
        DBG_PRINTF_WARNING("Set log level %u -> %u\n", gDebugLevel, level);
    gDebugLevel = level;
#else
    (void)level;
#endif
}

/**************************************************************************/
// Callbacks
/**************************************************************************/

static uint8_t (*callback_connect)(void) = NULL;
static uint8_t (*callback_prepare_daq)(void) = NULL;
static uint8_t (*callback_start_daq)(void) = NULL;
static void (*callback_stop_daq)(void) = NULL;
static uint8_t (*callback_freeze_daq)(uint8_t clear, uint16_t config_id) = NULL;
static uint8_t (*callback_get_cal_page)(uint8_t segment, uint8_t mode) = NULL;
static uint8_t (*callback_set_cal_page)(uint8_t segment, uint8_t page, uint8_t mode) = NULL;
static uint8_t (*callback_init_cal)(uint8_t src_page, uint8_t dst_page) = NULL;
static uint8_t (*callback_freeze_cal)(void) = NULL;
static uint8_t (*callback_read)(uint32_t src, uint8_t size, uint8_t *dst) = NULL;
static uint8_t (*callback_write)(uint32_t dst, uint8_t size, const uint8_t *src, uint8_t delay) = NULL;
static uint8_t (*callback_flush)(void) = NULL;

void ApplXcpRegisterCallbacks(uint8_t (*cb_connect)(void), uint8_t (*cb_prepare_daq)(void), uint8_t (*cb_start_daq)(void), void (*cb_stop_daq)(void),
                              uint8_t (*cb_freeze_daq)(uint8_t clear, uint16_t config_id), uint8_t (*cb_get_cal_page)(uint8_t segment, uint8_t mode),
                              uint8_t (*cb_set_cal_page)(uint8_t segment, uint8_t page, uint8_t mode), uint8_t (*cb_freeze_cal)(void),
                              uint8_t (*cb_init_cal)(uint8_t src_page, uint8_t dst_page),
#ifdef XCP_ENABLE_APP_ADDRESSING
                              uint8_t (*cb_read)(uint32_t src, uint8_t size, uint8_t *dst), uint8_t (*cb_write)(uint32_t dst, uint8_t size, const uint8_t *src, uint8_t delay),
                              uint8_t (*cb_flush)(void)
#endif
) {

    callback_connect = cb_connect;
    callback_prepare_daq = cb_prepare_daq;
    callback_start_daq = cb_start_daq;
    callback_stop_daq = cb_stop_daq;
    callback_freeze_daq = cb_freeze_daq;
    callback_get_cal_page = cb_get_cal_page;
    callback_set_cal_page = cb_set_cal_page;
    callback_freeze_cal = cb_freeze_cal;
    callback_init_cal = cb_init_cal;
#ifdef XCP_ENABLE_APP_ADDRESSING
    callback_read = cb_read;
    callback_write = cb_write;
    callback_flush = cb_flush;
#endif
}

/**************************************************************************/
// General notifications from XCPlite.c
/**************************************************************************/

bool ApplXcpConnect(void) {
    DBG_PRINT4("ApplXcpConnect\n");
#ifdef XCP_ENABLE_USER_COMMAND
    write_delay = false;
#endif
    if (callback_connect != NULL)
        return callback_connect();
    return true;
}

void ApplXcpDisconnect(void) { DBG_PRINT4("ApplXcpDisconnect\n"); }

#if XCP_PROTOCOL_LAYER_VERSION >= 0x0104
bool ApplXcpPrepareDaq() {
    DBG_PRINT4("ApplXcpPrepareDaq\n");
    if (callback_prepare_daq != NULL) {
        if (!callback_prepare_daq()) {
            DBG_PRINT_WARNING("DAQ start canceled by AppXcpPrepareDaq!\n");
            return false;
        };
    }
    return true;
}
#endif

void ApplXcpStartDaq() {
    DBG_PRINT4("ApplXcpStartDaq\n");
    if (callback_start_daq != NULL)
        callback_start_daq();
}

void ApplXcpStopDaq(void) {
    DBG_PRINT4("ApplXcpStartDaq\n");
    if (callback_stop_daq != NULL)
        callback_stop_daq();
}

/**************************************************************************/
// Clock
// Get clock for DAQ timestamps
/**************************************************************************/

uint64_t ApplXcpGetClock64(void) {

    /* Return value is clock with
        Clock timestamp resolution defined in xcp_cfg.h
        Clock must be monotonic !!!
    */
    return clockGet();
}

uint8_t ApplXcpGetClockState(void) {

    /* Return value may be one of the following:
        CLOCK_STATE_SYNCH, CLOCK_STATE_SYNCH_IN_PROGRESS, CLOCK_STATE_FREE_RUNNING, CLOCK_STATE_GRANDMASTER_STATE_SYNCH
    */
    return CLOCK_STATE_FREE_RUNNING; // Clock is a free running counter
}

bool ApplXcpGetClockInfoGrandmaster(uint8_t *uuid, uint8_t *epoch, uint8_t *stratum) {

    (void)uuid;
    (void)epoch;
    (void)stratum;
    /*
    Return value true, please set the following parameters:
        stratum: XCP_STRATUM_LEVEL_UNKNOWN, XCP_STRATUM_LEVEL_RTC,XCP_STRATUM_LEVEL_GPS
        epoch: XCP_EPOCH_TAI, XCP_EPOCH_UTC, XCP_EPOCH_ARB
    */
    return false; // No PTP support
}

/**************************************************************************/
// Pointer - Address conversion
/**************************************************************************/

// 64 Bit and 32 Bit platform pointer to XCP/A2L address conversions
// XCP memory access is limited to a 4GB address range (32 Bit)

// The XCP addresses with extension = 0 for Win32 and Win64 versions of XCPlite are defined as relative to the load address of the main module
// This allows using Microsoft linker PDB files for address update
// In Microsoft Visual Studio set option "Generate Debug Information" to "optimized for sharing and publishing (/DEBUG:FULL)"

#ifdef XCP_ENABLE_ABS_ADDRESSING
uint8_t *ApplXcpGetPointer(uint8_t addr_ext, uint32_t addr) {

    if (addr_ext != XCP_ADDR_EXT_ABS)
        return NULL;
    uint8_t *p;

#ifdef _WIN32 // on WIN32 check that XCP address is in range, because addr is relativ to baseaddr
    assert((uint64_t)ApplXcpGetBaseAddr() + addr <= 0xffffffff);
#endif
#ifdef _WIN64 // on WIN64 check that XCP address does not overflow
    assert((uint64_t)ApplXcpGetBaseAddr() + addr >= (uint64_t)ApplXcpGetBaseAddr());
#endif

    p = ApplXcpGetBaseAddr() + addr;
    return p;
}
#endif

#ifdef _WIN

static uint8_t *baseAddr = NULL;
static uint8_t baseAddrValid = 0;

// Get base pointer for the XCP address range
// This function is time sensitive, as it is called once on every XCP event
uint8_t *ApplXcpGetBaseAddr(void) {

    if (!baseAddrValid) {
        baseAddr = (uint8_t *)GetModuleHandle(NULL);
        baseAddrValid = 1;
        DBG_PRINTF4("ApplXcpGetBaseAddr() = 0x%I64X\n", (uint64_t)baseAddr);
    }
    return baseAddr;
}

uint32_t ApplXcpGetAddr(const uint8_t *p) {

    assert(p >= ApplXcpGetBaseAddr());
#ifdef _WIN64
    assert(((uint64_t)p - (uint64_t)ApplXcpGetBaseAddr()) <= 0xffffffff); // be sure that XCP address range is sufficient
#endif
    return (uint32_t)(p - ApplXcpGetBaseAddr());
}

#if defined(_LINUX64) && !defined(_MACOS)

#ifndef __USE_GNU
#define __USE_GNU
#endif
#include <link.h>

uint8_t *baseAddr = NULL;
uint8_t baseAddrValid = 0;

static int dump_phdr(struct dl_phdr_info *pinfo, size_t size, void *data) {
    // DBG_PRINTF3("name=%s (%d segments)\n", pinfo->dlpi_name, pinfo->dlpi_phnum);

    // Application modules has no name
    if (0 == strlen(pinfo->dlpi_name)) {
        baseAddr = (uint8_t *)pinfo->dlpi_addr;
    }

    (void)size;
    (void)data;
    return 0;
}

uint8_t *ApplXcpGetBaseAddr(void) {

    if (!baseAddrValid) {
        dl_iterate_phdr(dump_phdr, NULL);
        assert(baseAddr != NULL);
        baseAddrValid = 1;
        DBG_PRINTF4("ApplXcpGetBaseAddr = %lX\n", (uint64_t)baseAddr);
    }

    return baseAddr;
}

uint32_t ApplXcpGetAddr(const uint8_t *p) {
    uint8_t *b = ApplXcpGetBaseAddr();
    assert(p >= b);
    assert(((uint64_t)p - (uint64_t)b) <= 0xffffffff); // be sure that XCP address range is sufficient
    return (uint32_t)(p - b);
}

#endif

#ifdef _MACOS

#include <mach-o/dyld.h>

/*
static int dump_so(void) {
    uint32_t i;
    uint32_t count = _dyld_image_count();
    for (i = 0; i < count; i++) {
        const char* name = _dyld_get_image_name(i);
        const struct mach_header* header = _dyld_get_image_header(i);
        printf("Library %d: %s, Header at: %p\n", i, name, header);
    }
    return 0;
}
*/

static uint8_t *baseAddr = NULL;
static uint8_t baseAddrValid = 0;

uint8_t *ApplXcpGetBaseAddr(void) {

    if (!baseAddrValid) {
        // dump_so();
        baseAddr = (uint8_t *)_dyld_get_image_header(0); // Module addr
        assert(baseAddr != NULL);
        baseAddrValid = 1;
        DBG_PRINTF4("ApplXcpGetBaseAddr = %llX\n", (uint64_t)baseAddr);
    }

    return baseAddr;
}

uint32_t ApplXcpGetAddr(const uint8_t *p) {
    uint8_t *b = ApplXcpGetBaseAddr();
    if (p < b || ((uint64_t)p - (uint64_t)b) > 0xffffffff) { // be sure that XCP address range is sufficient
        DBG_PRINTF_ERROR("Address out of range! base = %llX, addr = %llX\n", (uint64_t)b, (uint64_t)p);
        // assert(0);
    }
    return (uint32_t)(p - b);
}

#endif

#ifdef _LINUX32

uint8_t *ApplXcpGetBaseAddr(void) { return ((uint8_t *)0); }

uint32_t ApplXcpGetAddr(const uint8_t *p) { return ((uint32_t)(p)); }

#endif

#endif // XCP_ENABLE_ABS_ADDRESSING

/**************************************************************************/
// Memory access
/**************************************************************************/

#ifdef XCP_ENABLE_USER_COMMAND

uint8_t ApplXcpUserCommand(uint8_t cmd) {
    switch (cmd) {
    case 0x01: // Begin atomic calibration operation
        write_delay = true;
        break;
    case 0x02: // End atomic calibration operation;
        write_delay = false;
        if (callback_flush != NULL)
            return callback_flush();
        break;
    default:
        return CRC_CMD_UNKNOWN;
    }
    return CRC_CMD_OK;
}

#endif

#ifdef XCP_ENABLE_APP_ADDRESSING
uint8_t ApplXcpReadMemory(uint32_t src, uint8_t size, uint8_t *dst) {
    if (callback_read != NULL)
        return callback_read(src, size, dst);
    return CRC_ACCESS_DENIED;
}
uint8_t ApplXcpWriteMemory(uint32_t dst, uint8_t size, const uint8_t *src) {
    if (callback_write != NULL)
        return callback_write(dst, size, src, write_delay);
    return CRC_ACCESS_DENIED;
}
#endif

/**************************************************************************/
// Calibration page switching callbacks
/**************************************************************************/

#ifdef XCP_ENABLE_CAL_PAGE

uint8_t ApplXcpGetCalPage(uint8_t segment, uint8_t mode) {
    if (callback_get_cal_page != NULL)
        return callback_get_cal_page(segment, mode); // return cal page number
    return 0;                                        // page 0 is default
}

uint8_t ApplXcpSetCalPage(uint8_t segment, uint8_t page, uint8_t mode) {
    if (callback_set_cal_page != NULL)
        return callback_set_cal_page(segment, page, mode); // return CRC_CMD_xxx return code
    return CRC_CMD_OK;
}

#ifdef XCP_ENABLE_COPY_CAL_PAGE
uint8_t ApplXcpCopyCalPage(uint8_t srcSeg, uint8_t srcPage, uint8_t dstSeg, uint8_t dstPage) {
    if (srcSeg != dstSeg && srcSeg > 0)
        return CRC_PAGE_NOT_VALID; // Only one segment supported
    if (callback_init_cal != NULL)
        return callback_init_cal(srcPage, dstPage); // return CRC_CMD_xxx return code
    return CRC_CMD_UNKNOWN;
}
#endif

#ifdef XCP_ENABLE_FREEZE_CAL_PAGE
uint8_t ApplXcpCalFreeze() {
    if (callback_freeze_cal != NULL)
        return callback_freeze_cal(); // return CRC_CMD_xxx return code
    return CRC_CMD_UNKNOWN;
}
#endif

#endif

/**************************************************************************/
// DAQ resume
/**************************************************************************/

#ifdef XCP_ENABLE_DAQ_RESUME

uint8_t ApplXcpDaqResumeStore(uint16_t config_id) {

    DBG_PRINTF3("ApplXcpResumeStore config-id=%u\n", config_id);

    //   FILE *f = fopen("XCPsim.DAQ","wb");
    //   if (f) {
    //     fwrite(&xcp.Daq,sizeof(xcp.Daq),1,f);
    //     fwrite(&gRemoteAddr,sizeof(gRemoteAddr),1,f);
    //     fclose(f);
    //   }
    // return CRC_CMD_IGNORED;

    return CRC_CMD_OK;
}

uint8_t ApplXcpDaqResumeClear(void) {

    DBG_PRINT3("ApplXcpResumeClear\n");

    // remove("XCPsim.DAQ");
    // return CRC_CMD_IGNORED;
    return CRC_CMD_OK;
}

#endif

/**************************************************************************/
// Functions for upload of A2L file
/**************************************************************************/

static const char *gXcpA2lName = NULL; // A2L filename (without extension .a2l)
static const char *gXcpEpk = NULL;     // EPK

// This is used by the Rust ffi to set the A2L name generated by the registry
void ApplXcpSetA2lName(const char *name) {
    DBG_PRINTF3("ApplXcpSetA2lName name='%s'\n", name);
    gXcpA2lName = (char *)name; // must be static lifetime
}
void ApplXcpSetEpk(const char *epk) {
    DBG_PRINTF3("ApplXcpSetEpk EPK='%s'\n", epk);
    gXcpEpk = (char *)epk; // must be static lifetime
}

#ifdef XCP_ENABLE_IDT_A2L_UPLOAD // Enable GET_ID A2L content upload to host

static FILE *gXcpFile = NULL;       // A2l file content
static uint32_t gXcpFileLength = 0; // A2L file length

void closeA2lFile(void) {
    assert(gXcpFile != NULL);
    fclose(gXcpFile);
    gXcpFile = NULL;
    DBG_PRINT3("Close A2L file\n");
}

uint32_t openA2lFile(void) {
    char filename[256];
    if (gXcpA2lName == NULL)
        return 0; // A2L file is not available
    SNPRINTF((char *)filename, 255, "%s.a2l", gXcpA2lName);

    assert(gXcpFile == NULL);
    gXcpFile = fopen(filename, "rb");
    if (gXcpFile == NULL) {
        DBG_PRINTF_ERROR("File %s not found!\n", filename);
        return 0;
    }

    fseek(gXcpFile, 0, SEEK_END);
    gXcpFileLength = (uint32_t)ftell(gXcpFile);
    rewind(gXcpFile);
    assert(gXcpFileLength > 0);

    DBG_PRINTF4("A2L file %s ready for upload, size=%u\n", filename, gXcpFileLength);
    return gXcpFileLength;
}

bool ApplXcpReadA2L(uint8_t size, uint32_t addr, uint8_t *data) {
    if (gXcpFile == NULL)
        return false;
    if (addr + size > gXcpFileLength)
        return false;
    if (size != fread(data, 1, (uint32_t)size, gXcpFile))
        return false;
    if (addr + size == gXcpFileLength)
        closeA2lFile(); // Close file after complete sequential read
    return true;
}

#endif // XCP_ENABLE_IDT_A2L_UPLOAD

/**************************************************************************/
// Provide infos for GET_ID
// The XCP command GET_ID provides different types of identification information
// Returns the length in bytes or 0, when the requested information is not available
/**************************************************************************/

uint32_t ApplXcpGetId(uint8_t id, uint8_t *buf, uint32_t bufLen) {

    uint32_t len = 0;
    switch (id) {

    case IDT_ASCII:
    case IDT_ASAM_NAME:
        if (gXcpA2lName == NULL)
            return 0;
        len = (uint32_t)strlen(gXcpA2lName);
        if (buf) {
            if (len >= bufLen - 1)
                return 0; // Insufficient buffer space
            strncpy((char *)buf, gXcpA2lName, len);
        }
        DBG_PRINTF3("ApplXcpGetId GET_ID%u name=%s\n", id, gXcpA2lName);
        break;

    case IDT_ASAM_PATH:
        if (gXcpA2lName == NULL)
            return 0;
        len = (uint32_t)strlen(gXcpA2lName) + 4;
        if (buf) {
            if (len > bufLen - 1)
                return 0; // Insufficient buffer space
            SNPRINTF((char *)buf, bufLen, "%s.a2l", gXcpA2lName);
        }
        DBG_PRINTF3("ApplXcpGetId GET_ID%u A2L path=%s\n", id, buf);
        break;

#ifdef XCP_ENABLE_IDT_A2L_UPLOAD
    case IDT_ASAM_EPK:
        if (gXcpEpk == NULL)
            return 0;
        len = (uint32_t)strlen(gXcpEpk);
        if (buf) {
            if (len > bufLen - 1)
                return 0; // Insufficient buffer space
            strncpy((char *)buf, gXcpEpk, len);
            DBG_PRINTF3("ApplXcpGetId GET_ID%u EPK=%s\n", id, gXcpEpk);
        } else {
            DBG_PRINTF3("ApplXcpGetId GET_ID%u EPK as upload (len=%u,value=%s)\n", id, len, gXcpEpk);
        }
        break;

    case IDT_ASAM_UPLOAD:
        assert(buf == NULL); // Not implemented
        len = openA2lFile();
        DBG_PRINTF3("ApplXcpGetId GET_ID%u A2L as upload (len=%u)\n", id, len);
        break;
#endif

#ifdef XCP_ENABLE_IDT_A2L_HTTP_GET
    case IDT_ASAM_URL:
        if (buf) {
            uint8_t addr[4];
            if (socketGetLocalAddr(NULL, addr)) {
                SNPRINTF((char *)buf, bufLen - 1, "http://%u.%u.%u.%u:%u/file/%s.a2l", addr[0], addr[1], addr[2], addr[3], gOptionHTTPPort, gXcpA2lName);
                len = (uint32_t)strlen((char *)buf);
            }
        }
        break;
#endif
    }
    return len;
}
