/*----------------------------------------------------------------------------
| File:
|   A2L.c
|
| Description:
|   Create A2L file
|
| Copyright (c) Vector Informatik GmbH. All rights reserved.
| Licensed under the MIT license. See LICENSE file in the project root for details.
|
 ----------------------------------------------------------------------------*/

#include "a2l.h"

#include <assert.h>  // for assert
#include <stdarg.h>  // for va_
#include <stdbool.h> // for bool
#include <stdint.h>  // for uintxx_t
#include <stdio.h>   // for fclose, fopen, fread, fseek, ftell
#include <string.h>  // for strlen, strncpy

#include "dbg_print.h" // for DBG_PRINTF3, DBG_PRINT4, DBG_PRINTF4, DBG...
#include "main_cfg.h"  // for OPTION_xxx
#include "platform.h"  // for platform defines (WIN_, LINUX_, MACOS_) and specific implementation of sockets, clock, thread, mutex
#include "xcp.h"       // for CRC_XXX
#include "xcpLite.h"   // for tXcpDaqLists, XcpXxx, ApplXcpXxx, ...
#include "xcp_cfg.h"   // for XCP_xxx
#include "xcptl_cfg.h" // for XCPTL_xxx

MUTEX gA2lMutex = MUTEX_INTIALIZER; // Mutex for concurrent A2L create macros

static FILE *gA2lFile = NULL;

static bool gA2lUseTCP = false;
static uint16_t gA2lOptionPort = 5555;
static uint8_t gA2lOptionBindAddr[4] = {0, 0, 0, 0};

static tXcpEventId gA2lFixedEvent = XCP_UNDEFINED_EVENT_ID;
static tXcpEventId gA2lDefaultEvent = XCP_UNDEFINED_EVENT_ID;
static uint8_t gAl2AddrExt = XCP_ADDR_EXT_ABS; // Address extension
static const uint8_t *gA2lAddrBase = NULL;     // Event or calseg address for XCP_ADDR_EXT_REL, XCP_ADDR_EXT_SEG
static tXcpCalSegIndex gA2lAddrIndex = 0;      // Segment index for XCP_ADDR_EXT_SEG
static uint32_t gA2lMeasurements;
static uint32_t gA2lParameters;
static uint32_t gA2lTypedefs;
static uint32_t gA2lComponents;
static uint32_t gA2lInstances;
static uint32_t gA2lConversions;

//----------------------------------------------------------------------------------
static const char *gA2lHeader = "ASAP2_VERSION 1 71\n"
                                "/begin PROJECT %s \"\"\n\n"
                                "/begin HEADER \"\" VERSION \"1.0\" PROJECT_NO VECTOR /end HEADER\n\n"
                                "/begin MODULE %s \"\"\n\n"
                                "/include \"XCP_104.aml\"\n\n"

                                "/begin MOD_COMMON \"\"\n"
                                "BYTE_ORDER MSB_LAST\n"
                                "ALIGNMENT_BYTE 1\n"
                                "ALIGNMENT_WORD 1\n"
                                "ALIGNMENT_LONG 1\n"
                                "ALIGNMENT_FLOAT16_IEEE 1\n"
                                "ALIGNMENT_FLOAT32_IEEE 1\n"
                                "ALIGNMENT_FLOAT64_IEEE 1\n"
                                "ALIGNMENT_INT64 1\n"
                                "/end MOD_COMMON\n"
                                "\n";

//----------------------------------------------------------------------------------
static const char *gA2lMemorySegment = "/begin MEMORY_SEGMENT\n"
                                       "%s \"\" DATA FLASH INTERN 0x%08X 0x%08X -1 -1 -1 -1 -1\n" // name, start, size
                                       "/begin IF_DATA XCP\n"
                                       "/begin SEGMENT 0x01 0x02 0x00 0x00 0x00 \n"
                                       "/begin CHECKSUM XCP_ADD_44 MAX_BLOCK_SIZE 0xFFFF EXTERNAL_FUNCTION \"\" /end CHECKSUM\n"
                                       // 2 calibration pages, 0=working page (RAM), 1=initial readonly page (FLASH), independent access to ECU and XCP page possible
                                       "/begin PAGE 0x01 ECU_ACCESS_DONT_CARE XCP_READ_ACCESS_DONT_CARE XCP_WRITE_ACCESS_NOT_ALLOWED /end PAGE\n"
                                       "/begin PAGE 0x00 ECU_ACCESS_DONT_CARE XCP_READ_ACCESS_DONT_CARE XCP_WRITE_ACCESS_DONT_CARE /end PAGE\n"
                                       "/end SEGMENT\n"
                                       "/end IF_DATA\n"
                                       "/end MEMORY_SEGMENT\n";

static const char *gA2lEpkMemorySegment = "/begin MEMORY_SEGMENT epk  \"\" DATA FLASH INTERN 0x80000000 %u -1 -1 -1 -1 -1 /end MEMORY_SEGMENT\n";

//----------------------------------------------------------------------------------
static const char *const gA2lIfDataBegin = "\n/begin IF_DATA XCP\n";

//----------------------------------------------------------------------------------
static const char *gA2lIfDataProtocolLayer = // Parameter: XCP_PROTOCOL_LAYER_VERSION, MAX_CTO, MAX_DTO
    "/begin PROTOCOL_LAYER\n"
    " 0x%04X"                                        // XCP_PROTOCOL_LAYER_VERSION
    " 1000 2000 0 0 0 0 0"                           // Timeouts T1-T7
    " %u %u "                                        // MAX_CTO, MAX_DTO
    "BYTE_ORDER_MSB_LAST ADDRESS_GRANULARITY_BYTE\n" // Intel and BYTE pointers
    "OPTIONAL_CMD GET_COMM_MODE_INFO\n"              // Optional commands
    "OPTIONAL_CMD GET_ID\n"
    "OPTIONAL_CMD SET_REQUEST\n"
    "OPTIONAL_CMD SET_MTA\n"
    "OPTIONAL_CMD UPLOAD\n"
    "OPTIONAL_CMD SHORT_UPLOAD\n"
    "OPTIONAL_CMD DOWNLOAD\n"
    "OPTIONAL_CMD SHORT_DOWNLOAD\n"
#ifdef XCP_ENABLE_CAL_PAGE
    "OPTIONAL_CMD GET_CAL_PAGE\n"
    "OPTIONAL_CMD SET_CAL_PAGE\n"
    "OPTIONAL_CMD COPY_CAL_PAGE\n"
//"OPTIONAL_CMD CC_GET_PAG_PROCESSOR_INFO\n"
//"OPTIONAL_CMD CC_GET_SEGMENT_INFO\n"
//"OPTIONAL_CMD CC_GET_PAGE_INFO\n"
//"OPTIONAL_CMD CC_SET_SEGMENT_MODE\n"
//"OPTIONAL_CMD CC_GET_SEGMENT_MODE\n"
#endif
#ifdef XCP_ENABLE_CHECKSUM
    "OPTIONAL_CMD BUILD_CHECKSUM\n"
#endif
    //"OPTIONAL_CMD TRANSPORT_LAYER_CMD\n"
    "OPTIONAL_CMD USER_CMD\n"
    "OPTIONAL_CMD GET_DAQ_RESOLUTION_INFO\n"
    "OPTIONAL_CMD GET_DAQ_PROCESSOR_INFO\n"
#ifdef XCP_ENABLE_DAQ_EVENT_INFO
    "OPTIONAL_CMD GET_DAQ_EVENT_INFO\n"
#endif
    //"OPTIONAL_CMD GET_DAQ_LIST_INFO\n"
    "OPTIONAL_CMD FREE_DAQ\n"
    "OPTIONAL_CMD ALLOC_DAQ\n"
    "OPTIONAL_CMD ALLOC_ODT\n"
    "OPTIONAL_CMD ALLOC_ODT_ENTRY\n"
    //"OPTIONAL_CMD CLEAR_DAQ_LIST\n"
    //"OPTIONAL_CMD READ_DAQ\n"
    "OPTIONAL_CMD SET_DAQ_PTR\n"
    "OPTIONAL_CMD WRITE_DAQ\n"
    "OPTIONAL_CMD GET_DAQ_LIST_MODE\n"
    "OPTIONAL_CMD SET_DAQ_LIST_MODE\n"
    "OPTIONAL_CMD START_STOP_SYNCH\n"
    "OPTIONAL_CMD START_STOP_DAQ_LIST\n"
    "OPTIONAL_CMD GET_DAQ_CLOCK\n"
#if XCP_TRANSPORT_LAYER_TYPE == XCP_TRANSPORT_LAYER_ETH
    "OPTIONAL_CMD WRITE_DAQ_MULTIPLE\n"
#if XCP_PROTOCOL_LAYER_VERSION >= 0x0103
    "OPTIONAL_CMD TIME_CORRELATION_PROPERTIES\n"
//"OPTIONAL_CMD DTO_CTR_PROPERTIES\n"
#endif
#if XCP_PROTOCOL_LAYER_VERSION >= 0x0104
    "OPTIONAL_LEVEL1_CMD GET_VERSION\n"
#ifdef XCP_ENABLE_PACKED_MODE
    "OPTIONAL_LEVEL1_CMD SET_DAQ_PACKED_MODE\n"
    "OPTIONAL_LEVEL1_CMD GET_DAQ_PACKED_MODE\n"
#endif
#endif
#if XCP_PROTOCOL_LAYER_VERSION >= 0x0150
//"OPTIONAL_LEVEL1_CMD SW_DBG_COMMAND_SPACE\n"
//"OPTIONAL_LEVEL1_CMD POD_COMMAND_SPACE\n"
#endif
#endif // ETH
    "/end PROTOCOL_LAYER\n"

#if XCP_PROTOCOL_LAYER_VERSION >= 0x0103
/*
"/begin TIME_CORRELATION\n" // TIME
"/end TIME_CORRELATION\n"
*/
#endif
    ;

//----------------------------------------------------------------------------------
static const char *gA2lIfDataBeginDAQ = // Parameter: %u max event, %s timestamp unit
    "/begin DAQ\n"
    "DYNAMIC 0 %u 0 OPTIMIZATION_TYPE_DEFAULT ADDRESS_EXTENSION_FREE IDENTIFICATION_FIELD_TYPE_RELATIVE_BYTE GRANULARITY_ODT_ENTRY_SIZE_DAQ_BYTE 0xF8 OVERLOAD_INDICATION_PID\n"
    "/begin TIMESTAMP_SUPPORTED\n"
    "0x01 SIZE_DWORD %s TIMESTAMP_FIXED\n"
    "/end TIMESTAMP_SUPPORTED\n";

// ... Event list follows, before EndDaq

//----------------------------------------------------------------------------------
static const char *const gA2lIfDataEndDAQ = "/end DAQ\n";

//----------------------------------------------------------------------------------
// XCP_ON_ETH
static const char *gA2lIfDataEth = // Parameter: %s TCP or UDP, %04X tl version, %u port, %s ip address string, %s TCP or UDP
    "/begin XCP_ON_%s_IP\n"        // Transport Layer
    "  0x%04X %u ADDRESS \"%s\"\n"
//"OPTIONAL_TL_SUBCMD GET_SERVER_ID\n"
//"OPTIONAL_TL_SUBCMD GET_DAQ_ID\n"
//"OPTIONAL_TL_SUBCMD SET_DAQ_ID\n"
#if defined(XCPTL_ENABLE_MULTICAST) && defined(XCP_ENABLE_DAQ_CLOCK_MULTICAST)
    "  OPTIONAL_TL_SUBCMD GET_DAQ_CLOCK_MULTICAST\n"
#endif
    "/end XCP_ON_%s_IP\n" // Transport Layer
    ;

//----------------------------------------------------------------------------------
static const char *const gA2lIfDataEnd = "/end IF_DATA\n\n";

//----------------------------------------------------------------------------------
static const char *const gA2lFooter = "/end MODULE\n"
                                      "/end PROJECT\n";

#define printPhysUnit(unit)                                                                                                                                                        \
    if (unit != NULL && strlen(unit) > 0)                                                                                                                                          \
        fprintf(gA2lFile, " PHYS_UNIT \"%s\"", unit);
#define printAddrExt(ext)                                                                                                                                                          \
    if (ext > 0)                                                                                                                                                                   \
        fprintf(gA2lFile, " ECU_ADDRESS_EXTENSION %u", ext);

const char *A2lGetSymbolName(const char *instance_name, const char *name) {
    static char s[256];
    if (instance_name != NULL && strlen(instance_name) > 0) {
        SNPRINTF(s, 256, "%s.%s", instance_name, name);
        return s;
    } else {
        return name;
    }
}

const char *A2lGetA2lTypeName(tA2lTypeId type) {

    switch (type) {
    case A2L_TYPE_INT8:
        return "SBYTE";
        break;
    case A2L_TYPE_INT16:
        return "SWORD";
        break;
    case A2L_TYPE_INT32:
        return "SLONG";
        break;
    case A2L_TYPE_INT64:
        return "A_INT64";
        break;
    case A2L_TYPE_UINT8:
        return "UBYTE";
        break;
    case A2L_TYPE_UINT16:
        return "UWORD";
        break;
    case A2L_TYPE_UINT32:
        return "ULONG";
        break;
    case A2L_TYPE_UINT64:
        return "A_UINT64";
        break;
    case A2L_TYPE_FLOAT:
        return "FLOAT32_IEEE";
        break;
    case A2L_TYPE_DOUBLE:
        return "FLOAT64_IEEE";
        break;
    default:
        return NULL;
    }
}

const char *A2lGetA2lTypeName_M(tA2lTypeId type) {
    switch (type) {
    case A2L_TYPE_INT8:
        return "M_I8";
        break;
    case A2L_TYPE_INT16:
        return "M_I16";
        break;
    case A2L_TYPE_INT32:
        return "M_I32";
        break;
    case A2L_TYPE_INT64:
        return "M_I64";
        break;
    case A2L_TYPE_UINT8:
        return "M_U8";
        break;
    case A2L_TYPE_UINT16:
        return "M_U16";
        break;
    case A2L_TYPE_UINT32:
        return "M_U32";
        break;
    case A2L_TYPE_UINT64:
        return "M_U64";
        break;
    case A2L_TYPE_FLOAT:
        return "M_F32";
        break;
    case A2L_TYPE_DOUBLE:
        return "M_F64";
        break;
    default:
        return NULL;
    }
}

const char *A2lGetA2lTypeName_C(tA2lTypeId type) {

    switch (type) {
    case A2L_TYPE_INT8:
        return "C_I8";
        break;
    case A2L_TYPE_INT16:
        return "C_I16";
        break;
    case A2L_TYPE_INT32:
        return "C_I32";
        break;
    case A2L_TYPE_INT64:
        return "C_I64";
        break;
    case A2L_TYPE_UINT8:
        return "C_U8";
        break;
    case A2L_TYPE_UINT16:
        return "C_U16";
        break;
    case A2L_TYPE_UINT32:
        return "C_U32";
        break;
    case A2L_TYPE_UINT64:
        return "C_U64";
        break;
    case A2L_TYPE_FLOAT:
        return "C_F32";
        break;
    case A2L_TYPE_DOUBLE:
        return "C_F64";
        break;
    default:
        return NULL;
    }
}

static const char *getRecordLayoutName(tA2lTypeId type) {

    switch (type) {
    case A2L_TYPE_INT8:
        return "I8";
        break;
    case A2L_TYPE_INT16:
        return "I16";
        break;
    case A2L_TYPE_INT32:
        return "I32";
        break;
    case A2L_TYPE_INT64:
        return "I64";
        break;
    case A2L_TYPE_UINT8:
        return "U8";
        break;
    case A2L_TYPE_UINT16:
        return "U16";
        break;
    case A2L_TYPE_UINT32:
        return "U32";
        break;
    case A2L_TYPE_UINT64:
        return "U64";
        break;
    case A2L_TYPE_FLOAT:
        return "F32";
        break;
    case A2L_TYPE_DOUBLE:
        return "F64";
        break;
    default:
        return NULL;
    }
}

static const char *getTypeMin(tA2lTypeId type) {
    const char *min;
    switch (type) {
    case A2L_TYPE_INT8:
        min = "-128";
        break;
    case A2L_TYPE_INT16:
        min = "-32768";
        break;
    case A2L_TYPE_INT32:
        min = "-2147483648";
        break;
    case A2L_TYPE_INT64:
        min = "-1E12";
        break;
    case A2L_TYPE_FLOAT:
        min = "-1E12";
        break;
    case A2L_TYPE_DOUBLE:
        min = "-1E12";
        break;
    default:
        min = "0";
    }
    return min;
}

static const char *getTypeMax(tA2lTypeId type) {
    const char *max;
    switch (type) {
    case A2L_TYPE_INT8:
        max = "127";
        break;
    case A2L_TYPE_INT16:
        max = "32767";
        break;
    case A2L_TYPE_INT32:
        max = "2147483647";
        break;
    case A2L_TYPE_UINT8:
        max = "255";
        break;
    case A2L_TYPE_UINT16:
        max = "65535";
        break;
    case A2L_TYPE_UINT32:
        max = "4294967295";
        break;
    default:
        max = "1E12";
    }
    return max;
}

static const char *getPhysMin(tA2lTypeId type, double factor, double offset) {
    double value = 0.0;
    switch (type) {
    case A2L_TYPE_INT8:
        value = -128;
        break;
    case A2L_TYPE_INT16:
        value = -32768;
        break;
    case A2L_TYPE_INT32:
        value = -(double)2147483648;
        break;
    case A2L_TYPE_INT64:
        value = -1E12;
        break;
    case A2L_TYPE_FLOAT:
        value = -1E12;
        break;
    case A2L_TYPE_DOUBLE:
        value = -1E12;
        break;
    default:
        value = 0.0;
    }

    static char str[20];
    SNPRINTF(str, 20, "%f", factor * value + offset);
    return str;
}

static const char *getPhysMax(tA2lTypeId type, double factor, double offset) {
    double value = 0.0;
    switch (type) {
    case A2L_TYPE_INT8:
        value = 127;
        break;
    case A2L_TYPE_INT16:
        value = 32767;
        break;
    case A2L_TYPE_INT32:
        value = 2147483647;
        break;
    case A2L_TYPE_UINT8:
        value = 255;
        break;
    case A2L_TYPE_UINT16:
        value = 65535;
        break;
    case A2L_TYPE_UINT32:
        value = 4294967295;
        break;
    default:
        value = 1E12;
    }
    static char str[20];
    SNPRINTF(str, 20, "%f", factor * value + offset);
    return str;
}

static bool A2lOpen(const char *filename, const char *projectname) {

    DBG_PRINTF3("A2L create %s\n", filename);

    gA2lFile = NULL;
    gA2lFixedEvent = XCP_UNDEFINED_EVENT_ID;
    gA2lMeasurements = gA2lParameters = gA2lTypedefs = gA2lInstances = gA2lConversions = gA2lComponents = 0;
    gA2lFile = fopen(filename, "w");
    if (gA2lFile == 0) {
        DBG_PRINTF_ERROR("Could not create A2L file %s!\n", filename);
        return false;
    }

    // Notify XCP that there is an A2L file available for upload by the XCP client tool
    ApplXcpSetA2lName(filename);

    // Create header
    fprintf(gA2lFile, gA2lHeader, projectname, projectname);

    // Create standard record layouts for elementary types
    for (int i = -10; i <= +10; i++) {
        tA2lTypeId id = (tA2lTypeId)i;
        const char *at = A2lGetA2lTypeName(id);
        if (at != NULL) {
            const char *t = getRecordLayoutName(id);
            fprintf(gA2lFile, "/begin RECORD_LAYOUT %s FNC_VALUES 1 %s ROW_DIR DIRECT /end RECORD_LAYOUT\n", t, at);
            fprintf(gA2lFile, "/begin TYPEDEF_MEASUREMENT M_%s \"\" %s NO_COMPU_METHOD 0 0 %s %s /end TYPEDEF_MEASUREMENT\n", t, at, getTypeMin(id), getTypeMax(id));
            fprintf(gA2lFile, "/begin TYPEDEF_CHARACTERISTIC C_%s \"\" VALUE %s 0 NO_COMPU_METHOD %s %s /end TYPEDEF_CHARACTERISTIC\n", t, t, getTypeMin(id), getTypeMax(id));
        }
    }
    fprintf(gA2lFile, "\n");

    return true;
}

// Memory segments
static void A2lCreate_MOD_PAR(void) {
    if (gA2lFile != NULL) {

#ifdef XCP_ENABLE_CALSEG_LIST
        fprintf(gA2lFile, "\n/begin MOD_PAR \"\"\n");
        const char *epk = XcpGetEpk();
        if (epk) {
            fprintf(gA2lFile, "EPK \"%s\" ADDR_EPK 0x80000000\n", epk);
            fprintf(gA2lFile, gA2lEpkMemorySegment, strlen(epk));
        }
        // Calibration segments are implicitly indexed
        // The segment number used in XCP commands XCP_SET_CAL_PAGE, GET_CAL_PAGE, XCP_GET_SEGMENT_INFO, ... are the indices of the segments starting with 0
        tXcpCalSegList const *calSegList = XcpGetCalSegList();
        for (uint32_t i = 0; i < calSegList->count; i++) {
            tXcpCalSeg const *calseg = &calSegList->calseg[i];
            fprintf(gA2lFile, gA2lMemorySegment, calseg->name, ((i + 1) << 16) | 0x80000000, calseg->size);
        }

        fprintf(gA2lFile, "/end MOD_PAR\n\n");
    }
#endif
}

static void A2lCreate_IF_DATA_DAQ(void) {

#if defined(XCP_ENABLE_DAQ_EVENT_LIST) && !defined(XCP_ENABLE_DAQ_EVENT_INFO)
    tXcpEventList *eventList;
#endif
    uint16_t eventCount = 0;

#if (XCP_TIMESTAMP_UNIT == DAQ_TIMESTAMP_UNIT_1NS)
#define XCP_TIMESTAMP_UNIT_S "UNIT_1NS"
#elif (XCP_TIMESTAMP_UNIT == DAQ_TIMESTAMP_UNIT_1US)
#define XCP_TIMESTAMP_UNIT_S "UNIT_1US"
#else
#error
#endif

    // Event list in A2L file (if event info by XCP is not active)
#if defined(XCP_ENABLE_DAQ_EVENT_LIST) && !defined(XCP_ENABLE_DAQ_EVENT_INFO)
    eventList = XcpGetEventList();
    eventCount = eventList->count;
#endif

    fprintf(gA2lFile, gA2lIfDataBeginDAQ, eventCount, XCP_TIMESTAMP_UNIT_S);

    // Eventlist
#if defined(XCP_ENABLE_DAQ_EVENT_LIST) && !defined(XCP_ENABLE_DAQ_EVENT_INFO)
    for (uint32_t id = 0; id < eventCount; id++) {
        tXcpEvent *event = &eventList->event[id];
        uint16_t index = event->index;
        const char *name = event->name;
        if (index == 0) {
            fprintf(gA2lFile, "/begin EVENT \"%s\" \"%s\" 0x%X DAQ 0xFF %u %u %u CONSISTENCY EVENT", name, name, id, event->timeCycle, event->timeUnit, event->priority);
        } else {
            fprintf(gA2lFile, "/begin EVENT \"%s_%u\" \"%s_%u\" 0x%X DAQ 0xFF %u %u %u CONSISTENCY EVENT", name, index, name, index, id, event->timeCycle, event->timeUnit,
                    event->priority);
        }

        fprintf(gA2lFile, " /end EVENT\n");
    }
#endif

    fprintf(gA2lFile, gA2lIfDataEndDAQ);
}

static void A2lCreate_ETH_IF_DATA(bool useTCP, const uint8_t *addr, uint16_t port) {
    if (gA2lFile != NULL) {

        fprintf(gA2lFile, gA2lIfDataBegin);

        // Protocol Layer info
        fprintf(gA2lFile, gA2lIfDataProtocolLayer, XCP_PROTOCOL_LAYER_VERSION, XCPTL_MAX_CTO_SIZE, XCPTL_MAX_DTO_SIZE);

        // DAQ info
        A2lCreate_IF_DATA_DAQ();

        // Transport Layer info
        uint8_t addr0[] = {127, 0, 0, 1}; // Use localhost if no other option
        if (addr != NULL && addr[0] != 0) {
            memcpy(addr0, addr, 4);
        } else {
            socketGetLocalAddr(NULL, addr0);
        }
        char addrs[17];
        SPRINTF(addrs, "%u.%u.%u.%u", addr0[0], addr0[1], addr0[2], addr0[3]);
        char *prot = useTCP ? (char *)"TCP" : (char *)"UDP";
        fprintf(gA2lFile, gA2lIfDataEth, prot, XCP_TRANSPORT_LAYER_VERSION, port, addrs, prot);

        fprintf(gA2lFile, gA2lIfDataEnd);

        DBG_PRINTF3("A2L IF_DATA XCP_ON_%s, ip=%s, port=%u\n", prot, addrs, port);
    }
}

static void A2lCreateMeasurement_IF_DATA(void) {
    if (gA2lFile != NULL) {
        if (gAl2AddrExt == XCP_ADDR_EXT_REL || gAl2AddrExt == XCP_ADDR_EXT_DYN) {
            if (gA2lFixedEvent != XCP_UNDEFINED_EVENT_ID) {
                fprintf(gA2lFile, " /begin IF_DATA XCP /begin DAQ_EVENT FIXED_EVENT_LIST EVENT 0x%X /end DAQ_EVENT /end IF_DATA", gA2lFixedEvent);
            } else {
                assert(false); // Fixed event must be set before calling this function
            }
        }
        if (gAl2AddrExt == XCP_ADDR_EXT_ABS) {
            if (gA2lFixedEvent != XCP_UNDEFINED_EVENT_ID) {
                fprintf(gA2lFile, " /begin IF_DATA XCP /begin DAQ_EVENT FIXED_EVENT_LIST EVENT 0x%X /end DAQ_EVENT /end IF_DATA", gA2lFixedEvent);
            } else if (gA2lDefaultEvent != XCP_UNDEFINED_EVENT_ID) {
                fprintf(gA2lFile, " /begin IF_DATA XCP /begin DAQ_EVENT VARIABLE DEFAULT_EVENT_LIST EVENT 0x%X /end DAQ_EVENT /end IF_DATA", gA2lDefaultEvent);
            }
        }
    }
}

//----------------------------------------------------------------------------------
// Mode

void A2lSetAbsAddrMode(void) {
    gAl2AddrExt = XCP_ADDR_EXT_ABS;
    A2lRstFixedEvent();
}

void A2lSetRelAddrMode(const tXcpEventId *event) {
    gA2lAddrBase = (uint8_t *)event;
    gAl2AddrExt = XCP_ADDR_EXT_REL;
    A2lSetFixedEvent(*event);
}

void A2lSetDynAddrMode(const tXcpEventId *event) {
    gA2lAddrBase = (uint8_t *)event;
    gAl2AddrExt = XCP_ADDR_EXT_DYN;
    A2lSetFixedEvent(*event);
}

void A2lSetSegAddrMode(tXcpCalSegIndex calseg_index, const uint8_t *calseg) {
    gA2lAddrIndex = calseg_index;
    gA2lAddrBase = calseg;
    gAl2AddrExt = XCP_ADDR_EXT_SEG;
}

void A2lRstAddrMode(void) {
    gA2lFixedEvent = XCP_UNDEFINED_EVENT_ID;
    gAl2AddrExt = XCP_UNDEFINED_ADDR_EXT;
    gA2lAddrBase = NULL;
    gA2lAddrIndex = 0;
}

//----------------------------------------------------------------------------------
// Set address mode by event name

// Absolute with fixed event by name
void A2lSetRelativeAddrMode_(const char *event_name, const uint8_t *base_addr) {

    assert(gA2lFile != NULL);

    tXcpEventId event = XcpFindEvent(event_name, NULL);
    if (event == XCP_UNDEFINED_EVENT_ID) {
        DBG_PRINTF_ERROR("SetRelativeAddrMode: Event %s not found!\n", event_name);
        return;
    }
    gAl2AddrExt = XCP_ADDR_EXT_DYN;
    gA2lAddrBase = base_addr;
    A2lSetFixedEvent(event);
    fprintf(gA2lFile, "\n/* Relative addressing mode: event=%s (%u), addr_ext=%u, addr_base=%p */\n", event_name, event, gAl2AddrExt, (void *)gA2lAddrBase);
}

// Relative with fixed event name and base address
void A2lSetAbsoluteAddrMode_(const char *event_name) {

    assert(gA2lFile != NULL);

    tXcpEventId event = XcpFindEvent(event_name, NULL);
    if (event == XCP_UNDEFINED_EVENT_ID) {
        DBG_PRINTF_ERROR("SetAbsoluteAddrMode: Event %s not found!\n", event_name);
        return;
    }
    gAl2AddrExt = XCP_ADDR_EXT_ABS;
    A2lSetFixedEvent(event);
    fprintf(gA2lFile, "\n/* Absolute addressing mode: event=%s (%u), addr_ext=%u, addr_base=%p */\n", event_name, event, gAl2AddrExt, (void *)ApplXcpGetBaseAddr());
}

//----------------------------------------------------------------------------------
// Address encoding

uint8_t A2lGetAddrExt_(void) { return gAl2AddrExt; }

uint32_t A2lGetAddr_(const void *p) {

    switch (gAl2AddrExt) {
    case XCP_ADDR_EXT_ABS: {
        return ApplXcpGetAddr(p); // Calculate the XCP address from a pointer
    }
    case XCP_ADDR_EXT_REL: {
        uint64_t addr_diff = (uint64_t)p - (uint64_t)gA2lAddrBase;
        // Ensure the relative address does not overflow the address space
        uint64_t addr_high = (addr_diff >> 32);
        if (addr_high != 0 && addr_high != 0xFFFFFFFF) {
            DBG_PRINTF_ERROR("A2L XCP_ADDR_EXT_REL relative address overflow detected! addr: %p, base: %p\n", p, (void *)gA2lAddrBase);
            assert(0); // Ensure the relative address does not overflow the 32 Bit A2L address space
        }
        return (uint32_t)(addr_diff & 0xFFFFFFFF);
    }
    case XCP_ADDR_EXT_DYN: {
        uint64_t addr_diff = (uint64_t)p - (uint64_t)gA2lAddrBase;

        // Ensure the relative address does not overflow the address space
        uint64_t addr_high = (addr_diff >> 16);
        if (addr_high != 0 && addr_high != 0xFFFFFFFFFFFF) {
            DBG_PRINTF_ERROR("A2L XCP_ADDR_EXT_DYN relative address overflow detected! addr: %p, base: %p\n", p, (void *)gA2lAddrBase);
            assert(0); // Ensure the relative address does not overflow the 32 Bit A2L address space
        }
        return (uint32_t)(((uint32_t)gA2lFixedEvent) << 16 | (addr_diff & 0xFFFF));
    }
    case XCP_ADDR_EXT_SEG: {
        uint64_t addr_diff = (uint64_t)p - (uint64_t)gA2lAddrBase;
        // Ensure the relative address does not overflow the 16 Bit A2L address offset for calibration segment relative addressing
        assert((addr_diff >> 16) == 0);
        return XcpGetCalSegBaseAddress(gA2lAddrIndex) + (addr_diff & 0xFFFF);
    }
    }
    DBG_PRINTF_ERROR("A2L address extension %u is not supported!\n", gAl2AddrExt);
    assert(0);
}

//----------------------------------------------------------------------------------
// Event

void A2lSetDefaultEvent(tXcpEventId event) {
    A2lRstFixedEvent();
    gA2lDefaultEvent = event;
}

void A2lSetFixedEvent(tXcpEventId event) { gA2lFixedEvent = event; }

uint16_t A2lGetFixedEvent(void) { return gA2lFixedEvent; }

void A2lRstDefaultEvent(void) { gA2lDefaultEvent = XCP_UNDEFINED_EVENT_ID; }

void A2lRstFixedEvent(void) { gA2lFixedEvent = XCP_UNDEFINED_EVENT_ID; }

//----------------------------------------------------------------------------------
// Typedefs

void A2lTypedefBegin_(const char *name, uint32_t size, const char *comment) {

    assert(gA2lFile != NULL);
    fprintf(gA2lFile, "/begin TYPEDEF_STRUCTURE %s \"%s\" 0x%X", name, comment, size);
#ifdef OPTION_ENABLE_A2L_SYMBOL_LINKS
    fprintf(gA2lFile, " SYMBOL_TYPE_LINK \"%s\"", name, 0);
#endif
    fprintf(gA2lFile, "\n");
    gA2lTypedefs++;
}

void A2lTypedefComponent_(const char *name, const char *type_name, uint16_t x_dim, uint32_t offset) {

    assert(gA2lFile != NULL);
    fprintf(gA2lFile, "  /begin STRUCTURE_COMPONENT %s %s 0x%X", name, type_name, offset);
    if (x_dim > 1)
        fprintf(gA2lFile, " MATRIX_DIM %u", x_dim);
#ifdef OPTION_ENABLE_A2L_SYMBOL_LINKS
    fprintf(gA2lFile, " SYMBOL_TYPE_LINK \"%s\"", name, 0);
#endif
    fprintf(gA2lFile, " /end STRUCTURE_COMPONENT\n");

    gA2lComponents++;
}

void A2lTypedefEnd_(void) {

    assert(gA2lFile != NULL);
    fprintf(gA2lFile, "/end TYPEDEF_STRUCTURE\n");
}

void A2lCreateTypedefInstance_(const char *instance_name, const char *typeName, uint16_t x_dim, uint8_t ext, uint32_t addr, const char *comment) {

    assert(gA2lFile != NULL);
    fprintf(gA2lFile, "/begin INSTANCE %s \"%s\" %s 0x%X", instance_name, comment, typeName, addr);
    printAddrExt(ext);
    if (x_dim > 1)
        fprintf(gA2lFile, " MATRIX_DIM %u", x_dim);
    A2lCreateMeasurement_IF_DATA();
    fprintf(gA2lFile, " /end INSTANCE\n");
    gA2lInstances++;
}

//----------------------------------------------------------------------------------
// Measurements

void A2lCreateMeasurement_(const char *instance_name, const char *name, tA2lTypeId type, uint8_t ext, uint32_t addr, double factor, double offset, const char *unit,
                           const char *comment) {

    assert(gA2lFile != NULL);

    const char *symbol_name = A2lGetSymbolName(instance_name, name);
    if (unit == NULL)
        unit = "";
    if (comment == NULL)
        comment = "";
    const char *conv = "NO_COMPU_METHOD";
    if (factor != 1.0 || offset != 0.0) {
        fprintf(gA2lFile, "/begin COMPU_METHOD %s \"\" LINEAR \"%%6.3\" \"%s\" COEFFS_LINEAR %g %g /end COMPU_METHOD\n", symbol_name, unit != NULL ? unit : "", factor, offset);
        conv = symbol_name;
        gA2lConversions++;
    }
    fprintf(gA2lFile, "/begin MEASUREMENT %s \"%s\" %s %s 0 0 %s %s ECU_ADDRESS 0x%X", symbol_name, comment, A2lGetA2lTypeName(type), conv, getPhysMin(type, factor, offset),
            getPhysMax(type, factor, offset), addr);
    printAddrExt(ext);
    printPhysUnit(unit);
    if (gAl2AddrExt == XCP_ADDR_EXT_ABS || gAl2AddrExt == XCP_ADDR_EXT_DYN) { // Absolute or dynamic address mode allows write access
        fprintf(gA2lFile, " READ_WRITE");
    }

#ifdef OPTION_ENABLE_A2L_SYMBOL_LINKS
    fprintf(gA2lFile, " SYMBOL_LINK \"%s\" %u", symbol_name, 0);
#endif
    A2lCreateMeasurement_IF_DATA();
    fprintf(gA2lFile, " /end MEASUREMENT\n");
    gA2lMeasurements++;
}

void A2lCreateMeasurementArray_(const char *instance_name, const char *name, tA2lTypeId type, int x_dim, int y_dim, uint8_t ext, uint32_t addr, double factor, double offset,
                                const char *unit, const char *comment) {

    assert(gA2lFile != NULL);
    const char *symbol_name = A2lGetSymbolName(instance_name, name);
    if (unit == NULL)
        unit = "";
    if (comment == NULL)
        comment = "";
    const char *conv = "NO_COMPU_METHOD";
    if (factor != 1.0 || offset != 0.0) {
        fprintf(gA2lFile, "/begin COMPU_METHOD %s.Conversion \"\" LINEAR \"%%6.3\" \"%s\" COEFFS_LINEAR %g %g /end COMPU_METHOD\n", symbol_name, unit != NULL ? unit : "", factor,
                offset);
        conv = symbol_name;
        gA2lConversions++;
    }
    fprintf(gA2lFile, "/begin CHARACTERISTIC %s \"%s\" VAL_BLK 0x%X %s 0 %s %s %s MATRIX_DIM %u %u", symbol_name, comment, addr, getRecordLayoutName(type), conv, getTypeMin(type),
            getTypeMax(type), x_dim, y_dim);
    printAddrExt(ext);
#ifdef OPTION_ENABLE_A2L_SYMBOL_LINKS
    fprintf(gA2lFile, " SYMBOL_LINK \"%s\" %u", symbol_name, 0);
#endif
    A2lCreateMeasurement_IF_DATA();
    fprintf(gA2lFile, " /end CHARACTERISTIC\n");
    gA2lMeasurements++;
}

//----------------------------------------------------------------------------------
// Parameters

void A2lCreateParameterWithLimits_(const char *name, tA2lTypeId type, uint8_t ext, uint32_t addr, const char *comment, const char *unit, double min, double max) {

    assert(gA2lFile != NULL);
    fprintf(gA2lFile, "/begin CHARACTERISTIC %s \"%s\" VALUE 0x%X %s 0 NO_COMPU_METHOD %g %g", name, comment, addr, getRecordLayoutName(type), min, max);
    printPhysUnit(unit);
    printAddrExt(ext);
#ifdef OPTION_ENABLE_A2L_SYMBOL_LINKS
    fprintf(gA2lFile, " SYMBOL_LINK \"%s\" %u", name, 0);
#endif
    A2lCreateMeasurement_IF_DATA();
    fprintf(gA2lFile, " /end CHARACTERISTIC\n");
    gA2lParameters++;
}

void A2lCreateParameter_(const char *name, tA2lTypeId type, uint8_t ext, uint32_t addr, const char *comment, const char *unit) {

    assert(gA2lFile != NULL);
    fprintf(gA2lFile, "/begin CHARACTERISTIC %s \"%s\" VALUE 0x%X %s 0 NO_COMPU_METHOD %s %s", name, comment, addr, getRecordLayoutName(type), getTypeMin(type), getTypeMax(type));
    printPhysUnit(unit);
    printAddrExt(ext);
#ifdef OPTION_ENABLE_A2L_SYMBOL_LINKS
    fprintf(gA2lFile, " SYMBOL_LINK \"%s\" %u", name, 0);
#endif
    A2lCreateMeasurement_IF_DATA();

    fprintf(gA2lFile, " /end CHARACTERISTIC\n");
    gA2lParameters++;
}

void A2lCreateMap_(const char *name, tA2lTypeId type, uint8_t ext, uint32_t addr, uint32_t xdim, uint32_t ydim, const char *comment, const char *unit) {

    assert(gA2lFile != NULL);
    fprintf(gA2lFile,
            "/begin CHARACTERISTIC %s \"%s\" MAP 0x%X %s 0 NO_COMPU_METHOD %s %s"
            " /begin AXIS_DESCR FIX_AXIS NO_INPUT_QUANTITY NO_COMPU_METHOD  %u 0 %u FIX_AXIS_PAR_DIST 0 1 %u /end AXIS_DESCR"
            " /begin AXIS_DESCR FIX_AXIS NO_INPUT_QUANTITY NO_COMPU_METHOD  %u 0 %u FIX_AXIS_PAR_DIST 0 1 %u /end AXIS_DESCR",
            name, comment, addr, getRecordLayoutName(type), getTypeMin(type), getTypeMax(type), xdim, xdim - 1, xdim, ydim, ydim - 1, ydim);
    printPhysUnit(unit);
    printAddrExt(ext);
#ifdef OPTION_ENABLE_A2L_SYMBOL_LINKS
    fprintf(gA2lFile, " SYMBOL_LINK \"%s\" %u", name, 0);
#endif
    A2lCreateMeasurement_IF_DATA();

    fprintf(gA2lFile, " /end CHARACTERISTIC\n");
    gA2lParameters++;
}

void A2lCreateCurve_(const char *name, tA2lTypeId type, uint8_t ext, uint32_t addr, uint32_t xdim, const char *comment, const char *unit) {

    assert(gA2lFile != NULL);
    fprintf(gA2lFile,
            "/begin CHARACTERISTIC %s \"%s\" CURVE 0x%X %s 0 NO_COMPU_METHOD %s %s"
            " /begin AXIS_DESCR FIX_AXIS NO_INPUT_QUANTITY NO_COMPU_METHOD  %u 0 %u FIX_AXIS_PAR_DIST 0 1 %u /end AXIS_DESCR",
            name, comment, addr, getRecordLayoutName(type), getTypeMin(type), getTypeMax(type), xdim, xdim - 1, xdim);
    printPhysUnit(unit);
    printAddrExt(ext);
#ifdef OPTION_ENABLE_A2L_SYMBOL_LINKS
    fprintf(gA2lFile, " SYMBOL_LINK \"%s\" %u", name, 0);
#endif
    A2lCreateMeasurement_IF_DATA();

    fprintf(gA2lFile, " /end CHARACTERISTIC\n");
    gA2lParameters++;
}

//----------------------------------------------------------------------------------
// Groups

void A2lParameterGroup(const char *name, int count, ...) {

    va_list ap;

    assert(gA2lFile != NULL);
    fprintf(gA2lFile, "/begin GROUP %s \"\" ROOT", name);
    fprintf(gA2lFile, " /begin REF_CHARACTERISTIC\n");
    va_start(ap, count);
    for (int i = 0; i < count; i++) {
        fprintf(gA2lFile, " %s", va_arg(ap, char *));
    }
    va_end(ap);
    fprintf(gA2lFile, "\n/end REF_CHARACTERISTIC ");
    fprintf(gA2lFile, "/end GROUP\n\n");
}

void A2lParameterGroupFromList(const char *name, const char *pNames[], int count) {

    assert(gA2lFile != NULL);
    fprintf(gA2lFile, "/begin GROUP %s \"\" ROOT", name);
    fprintf(gA2lFile, " /begin REF_CHARACTERISTIC\n");
    for (int i = 0; i < count; i++) {
        fprintf(gA2lFile, " %s", pNames[i]);
    }
    fprintf(gA2lFile, "\n/end REF_CHARACTERISTIC ");
    fprintf(gA2lFile, "/end GROUP\n\n");
}

void A2lMeasurementGroup(const char *name, int count, ...) {

    va_list ap;

    assert(gA2lFile != NULL);
    fprintf(gA2lFile, "/begin GROUP %s \"\" ROOT", name);
    fprintf(gA2lFile, " /begin REF_MEASUREMENT");
    va_start(ap, count);
    for (int i = 0; i < count; i++) {
        fprintf(gA2lFile, " %s", va_arg(ap, char *));
    }
    va_end(ap);
    fprintf(gA2lFile, " /end REF_MEASUREMENT");
    fprintf(gA2lFile, " /end GROUP\n\n");
}

void A2lMeasurementGroupFromList(const char *name, char *names[], uint32_t count) {

    assert(gA2lFile != NULL);
    fprintf(gA2lFile, "/begin GROUP %s \"\" ROOT", name);
    fprintf(gA2lFile, " /begin REF_MEASUREMENT");
    for (uint32_t i1 = 0; i1 < count; i1++) {
        fprintf(gA2lFile, " %s", names[i1]);
    }
    fprintf(gA2lFile, " /end REF_MEASUREMENT");
    fprintf(gA2lFile, "\n/end GROUP\n\n");
}

//----------------------------------------------------------------------------------

bool A2lOnce_(atomic_bool *value) {
    bool old_value = false;
    if (atomic_compare_exchange_strong_explicit(value, &old_value, true, memory_order_relaxed, memory_order_relaxed)) {
        return gA2lFile != NULL; // Return true if A2L file is open
    } else {
        return false;
    }
}

//-----------------------------------------------------------------------------------------------------
// A2L file generation and finalization on XCP connect

// Finalize A2L file generation
bool A2lFinalize(void) {

    if (gA2lFile != NULL) {

        // @@@@ TODO: Improve EPK generation
        // A different A2L EPK version is  be required for the same build, if the order of event or calibration segment creation  is different and leads to different ids !!!!
        // Set the EPK (software version number) for the A2L file
        char epk[64];
        sprintf(epk, "EPK_%s_%s", __DATE__, __TIME__);
        XcpSetEpk(epk);

        // Create MOD_PAR section with EPK and calibration segments
        A2lCreate_MOD_PAR();

        // Create IF_DATA section with event list and transport layer info
        A2lCreate_ETH_IF_DATA(gA2lUseTCP, gA2lOptionBindAddr, gA2lOptionPort);

        fprintf(gA2lFile, "%s", gA2lFooter);
        fclose(gA2lFile);
        gA2lFile = NULL;
        DBG_PRINTF3("A2L created: %u measurements, %u params, %u typedefs, %u components, %u instances, %u conversions\n\n", gA2lMeasurements, gA2lParameters, gA2lTypedefs,
                    gA2lComponents, gA2lInstances, gA2lConversions);
    }
    return true;
}

static bool file_exists(const char *path) {
    FILE *file = fopen(path, "r");
    if (file) {
        fclose(file);
        return true;
    }
    return false;
}

// Open the A2L file and register the finalize callback
bool A2lInit(const char *a2l_filename, const char *a2l_projectname, const uint8_t *addr, uint16_t port, bool useTCP, bool finalize_on_connect) {

    assert(gA2lFile == NULL);
    assert(a2l_filename != NULL);
    assert(a2l_projectname != NULL);
    assert(addr != NULL);

    // Save transport layer parameters for A2l finalization
    memcpy(&gA2lOptionBindAddr, addr, 4);
    gA2lOptionPort = port;
    gA2lUseTCP = useTCP;

    // Check if A2L file already exists and rename it to 'name.old' if it does
    if (file_exists(a2l_filename)) {
        char old_filename[256];
        SNPRINTF(old_filename, sizeof(old_filename), "%s.old", a2l_filename);
        if (rename(a2l_filename, old_filename) != 0) {
            DBG_PRINTF_ERROR("Failed to rename existing A2L file %s to %s\n", a2l_filename, old_filename);
            return false;
        } else {
            DBG_PRINTF3("Renamed existing A2L file %s to %s\n", a2l_filename, old_filename);
        }
    }

    // Open A2L file
    if (!A2lOpen(a2l_filename, a2l_projectname)) {
        printf("Failed to open A2L file %s\n", a2l_filename);
        return false;
    }

    // Register finalize callback on XCP connect
    if (finalize_on_connect)
        ApplXcpRegisterConnectCallback(A2lFinalize);
    return true;
}
