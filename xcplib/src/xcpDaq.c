
/*****************************************************************************
| File:
|   xcpDaq.c
|
|  Description:
|    DAQ events for daemon multi app mode
|
| Copyright (c) Vector Informatik GmbH. All rights reserved.
| Licensed under the MIT license. See LICENSE file in the project root for details.
|***************************************************************************/


#include "main.h"
#include "platform.h"
#include "dbg_print.h"


// Configuration must match the daemon configuration
#ifdef __XCPTL_CFG_H__
#error "Include dependency error!"
#endif
#ifdef __XCP_CFG_H__
#error "Include dependency error!"
#endif
#include "xcptl_cfg.h"  // Transport layer configuration
#include "xcp_cfg.h"    // Protocol layer configuration

#undef XCP_ENABLE_MULTITHREAD_CAL_EVENTS
#undef XCP_ENABLE_DYN_ADDRESSING
#undef XCP_ENABLE_OVERRUN_INDICATION_PID



// External dependecies

// Transmit queue
extern uint8_t* XcpTlGetTransmitBuffer(void** handle, uint16_t size); // Get a buffer for a message with size
extern void XcpTlCommitTransmitBuffer(void* handle, BOOL flush); // Commit a buffer (by handle returned from XcpTlGetTransmitBuffer)

// DAQ runing state
extern BOOL XcpIsDaqRunning();

// Timestamps
extern uint64_t ApplXcpGetClock64();

// Base address for ABS addressing mode
extern uint8_t *ApplXcpGetBaseAddr(); // Get the base address for DAQ data access */



/****************************************************************************/
/* DAQ information                                                          */
/****************************************************************************/

#define DAQ_STATE_RUNNING                  ((uint8_t)0x02) /* Running */
#define DAQ_STATE_OVERRUN                  ((uint8_t)0x04) /* Overrun */


/* ODT */
/* Size must be even !!! */
typedef struct {
    uint16_t first_odt_entry;       /* Absolute odt entry number */
    uint16_t last_odt_entry;        /* Absolute odt entry number */
    uint16_t size;                /* Number of bytes */
} tXcpOdt;

/* DAQ list */
typedef struct {
    uint16_t last_odt;             /* Absolute odt number */
    uint16_t first_odt;            /* Absolute odt number */
    uint16_t event_channel;
#ifdef XCP_ENABLE_PACKED_MODE
    uint16_t sampleCount;         /* Packed mode */
#endif
    uint8_t mode;
    uint8_t state;
    uint8_t priority;
    uint8_t addr_ext;
} tXcpDaqList;


/* Dynamic DAQ list structure in a linear memory block with size XCP_DAQ_MEM_SIZE + 8 */
typedef struct {

    uint16_t odt_entry_count; // Total number of ODT entries in ODT entry addr and size arrays
    uint16_t odt_count; // Total number of ODTs in ODT array
    uint16_t daq_count; // Number of DAQ lists in DAQ list array
    uint16_t res;

    // Pointers to optimze access to DAQ lists, ODT and ODT entry array pointers
    const int32_t* odt_entry_addr; // ODT entry addr array
    const uint8_t* odt_entry_size; // ODT entry size array
    const tXcpOdt* odt; // ODT array

    // DAQ array
    union {
        // DAQ array
        tXcpDaqList daq_list[XCP_DAQ_MEM_SIZE / sizeof(tXcpDaqList)];
        // ODT array
        tXcpOdt odt[XCP_DAQ_MEM_SIZE / sizeof(tXcpOdt)];
        // ODT entry addr array
        uint32_t odt_entry_addr[XCP_DAQ_MEM_SIZE / sizeof(4)];
        // ODT entry size array
        uint8_t odt_entry_size[XCP_DAQ_MEM_SIZE];        

        // DAQ memory layout:
        //  tXcpDaqList[] - DAQ list array
        //  tXcpOdt[] - ODT array
        //  uint32_t[] - ODT entry addr array
        //  uint8_t[] - ODT entry size array
        uint8_t b[XCP_DAQ_MEM_SIZE];        
    } u;

} tXcpDaqLists;

#include "xcpDaq.h"


/****************************************************************************/
/* Data Aquisition Processor                                                */
/****************************************************************************/

#define isDaqRunning()  XcpIsDaqRunning()


/****************************************************************************/
/* DAQ list access helper macros                                            */
/****************************************************************************/

/* Shortcuts */

/* j is absolute odt number */
#define DaqListOdtEntryCount(j) ((((tXcpOdt*)daq_lists->odt)[j].last_odt_entry-daq_lists->odt[j].first_odt_entry)+1)
#define DaqListOdtLastEntry(j)  (((tXcpOdt*)daq_lists->odt)[j].last_odt_entry)
#define DaqListOdtFirstEntry(j) (((tXcpOdt*)daq_lists->odt)[j].first_odt_entry)
#define DaqListOdtSize(j)       (((tXcpOdt*)daq_lists->odt)[j].size)

/* n is absolute odtEntry number */
#define OdtEntrySize(n)         (((uint8_t*)daq_lists->odt_entry_size)[n])
#define OdtEntryAddr(n)         (((uint32_t*)daq_lists->odt_entry_addr)[n])

/* i is daq number */
#define DaqListOdtCount(i)      ((daq_lists->u.daq_list[i].last_odt-daq_lists->u.daq_list[i].first_odt)+1)
#define DaqListLastOdt(i)       daq_lists->u.daq_list[i].last_odt
#define DaqListFirstOdt(i)      daq_lists->u.daq_list[i].first_odt
#define DaqListMode(i)          daq_lists->u.daq_list[i].mode
#define DaqListState(i)         daq_lists->u.daq_list[i].state
#define DaqListEventChannel(i)  daq_lists->u.daq_list[i].event_channel
#define DaqListAddrExt(i)       daq_lists->u.daq_list[i].addr_ext
#define DaqListPriority(i)      daq_lists->u.daq_list[i].priority
#ifdef XCP_ENABLE_PACKED_MODE
#define DaqListSampleCount(i)   daq_lists->u.daq_list[i].sampleCount
#endif



// Measurement data acquisition, sample and transmit measurement date associated to event

// Trigger daq list
void XcpDaemonTriggerDaq(tXcpDaqLists* daq_lists, uint16_t daq, const uint8_t* base, uint64_t clock) {

      uint8_t *d0;
      uint32_t e, el, odt, hs, n;
      void* handle = NULL;
#ifdef XCP_ENABLE_PACKED_MODE
      uint32_t sc;
#endif

#ifdef XCP_ENABLE_PACKED_MODE
      sc = DaqListSampleCount(daq); // Packed mode sample count, 0 if not packed
#endif

#define ODT_TIMESTAMP_SIZE 4
#if XCP_MAX_DAQ_COUNT>256
  #define ODT_HEADER_SIZE 4 // ODT,align,DAQ_WORD header 
#else
  #define ODT_HEADER_SIZE 2 // ODT,DAQ header
#endif

      // Loop over all ODTs of the current DAQ list
      for (hs=ODT_HEADER_SIZE+ODT_TIMESTAMP_SIZE,odt=DaqListFirstOdt(daq);odt<=DaqListLastOdt(daq);hs=ODT_HEADER_SIZE,odt++)  {

          // Mutex to ensure transmit buffers with time stamp in ascending order
#if defined(XCP_ENABLE_MULTITHREAD_DAQ_EVENTS) && defined(XCP_ENABLE_DAQ_EVENT_LIST)
          mutexLock(&ev->mutex);
#endif
          // Get clock, if not given as parameter
          if (clock==0) clock = ApplXcpGetClock64();

          // Get DTO buffer
          d0 = XcpTlGetTransmitBuffer(&handle, (uint16_t)(DaqListOdtSize(odt) + hs));

#if defined(XCP_ENABLE_MULTITHREAD_DAQ_EVENTS) && defined(XCP_ENABLE_DAQ_EVENT_LIST)
          mutexUnlock(&ev->mutex);
#endif

          // Check declining time stamps
          // Disable for maximal measurement performance
#ifdef XCP_ENABLE_DAQ_EVENT_LIST
  #if defined(XCP_ENABLE_TIMESTAMP_CHECK)
          if (ev->time > clock) { // declining time stamps
              DBG_PRINTF_ERROR("ERROR: Declining timestamp! event=%u, diff=%" PRIu64 "\n", event, ev->time-clock);
          }
          if (ev->time == clock) { // duplicate time stamps
              DBG_PRINTF_WARNING("WARNING: Duplicate timestamp! event=%u\n", event);
          }
  #endif
#endif

         // Buffer overrun
         if (d0 == NULL) {
            // gXcp.DaqOverflowCount++;
            // DBG_PRINTF4("DAQ queue overrun, daq=%u, odt=%u, overruns=%u\n", daq, odt, gXcp.DaqOverflowCount);
            DaqListState(daq) |= DAQ_STATE_OVERRUN;
            DBG_PRINTF4("DAQ queue overrun, daq=%u, odt=%u\n", daq, odt);
            return; // Skip rest of this event on queue overrun, to simplify resynchronisation of the client
        }

        // ODT header (ODT8,FIL8,DAQ16 or ODT8,DAQ8)
        d0[0] = (uint8_t)(odt-DaqListFirstOdt(daq)); /* Relative odt number as byte*/
#if ODT_HEADER_SIZE==4
        d0[1] = 0xAA; // Align byte 
        *((uint16_t*)&d0[2]) = daq;
#else
        d0[1] = (uint8_t)daq;
#endif
       
        // Use MSB of ODT to indicate overruns
#ifdef XCP_ENABLE_OVERRUN_INDICATION_PID
        if ( (DaqListState(daq) & DAQ_STATE_OVERRUN) != 0 ) {
          d0[0] |= 0x80; // Set MSB of ODT number
          DaqListState(daq) &= (uint8_t)(~DAQ_STATE_OVERRUN);
        }
#endif

        // Timestamp 32 or 64 bit
        if (hs == ODT_HEADER_SIZE+ODT_TIMESTAMP_SIZE) { // First ODT always has a 32 bit timestamp
#if ODT_TIMESTAMP_SIZE==8     
            *((uint64_t*)&d0[ODT_HEADER_SIZE]) = clock;
#else
            *((uint32_t*)&d0[ODT_HEADER_SIZE]) = (uint32_t)clock;
#endif
        }

        // Copy data 
        /* This is the inner loop, optimize here */
        e = DaqListOdtFirstEntry(odt);
        // Static length
        if (OdtEntrySize(e) != 0) {
            uint8_t *d = &d0[hs];
            el = DaqListOdtLastEntry(odt);
            while (e <= el) { // inner DAQ loop
                n = OdtEntrySize(e);
                if (n == 0) break;
#ifdef XCP_ENABLE_PACKED_MODE
                if (sc>1) n *= sc; // packed mode
#endif

              if (n==8) {
                *(uint64_t*)d = *(const uint64_t*)&base[OdtEntryAddr(e)];
                d += 8;
              }
              else if (n==4) {
                *(uint32_t*)d = *(const uint32_t*)&base[OdtEntryAddr(e)];
                d += 4;
              }
              else if (n<4) {
                const uint8_t *s = &base[OdtEntryAddr(e)];
                do { *d++ = *s++; } while (--n); 
              } else
              {
                memcpy((uint8_t*)d, &base[OdtEntryAddr(e)], n);
                d += n;
              }
              e++;
            } // ODT entry
        }
        // Dynamic length
        else {
            assert(FALSE);
        }

        XcpTlCommitTransmitBuffer(handle, DaqListPriority(daq)!=0 && odt==DaqListLastOdt(daq));
      } /* odt */

}

// Trigger event
static void XcpDaemonTriggerEvent(tXcpDaqLists* daq_lists, uint16_t event, const uint8_t* base, uint64_t clock) {

  if (!isDaqRunning()) return; // DAQ not running

  // Experimental
  // Optimize for large daq list count, when there is a 1:1 relation between DAQ lists and events
  // Not much benefit - optimize the contemption of the transmit queue mutex first
  // assert(DaqListEventChannel(event) == event);
  // assert(event<daq_lists->daq_count);
  // if ((DaqListState(event) & DAQ_STATE_RUNNING) == 0) return; // DAQ list not active
  // XcpTriggerDaq(event,base,clock);

  uint16_t daq;

  // Loop over all active DAQ lists associated to the current event
  for (daq=0; daq<daq_lists->daq_count; daq++) {

      if ((DaqListState(daq) & DAQ_STATE_RUNNING) == 0) continue; // DAQ list not active
      if (DaqListEventChannel(daq) != event) continue; // DAQ list not associated with this event

      XcpDaemonTriggerDaq(daq_lists, daq,base,clock); // Trigger DAQ list

  } /* daq */

  #if defined(XCP_ENABLE_TIMESTAMP_CHECK)
  ev->time = clock;
  #endif

}

// ABS adressing mode event with clock
// Base is ApplXcpGetBaseAddr()
#ifdef XCP_ENABLE_ABS_ADDRESSING
void XcpDaemonEventAt(tXcpDaqLists* daq_lists, uint16_t event, uint64_t clock) {
    if (!isDaqRunning()) return; // DAQ not running
    XcpDaemonTriggerEvent(daq_lists, event, ApplXcpGetBaseAddr(), clock);
}
#endif

// ABS addressing mode event
// Base is ApplXcpGetBaseAddr()
#ifdef XCP_ENABLE_ABS_ADDRESSING
void XcpDaemonEvent(tXcpDaqLists* daq_lists, uint16_t event) {
    if (!isDaqRunning()) return; // DAQ not running
    XcpDaemonTriggerEvent(daq_lists, event, ApplXcpGetBaseAddr(), 0);
}
#endif

// Dyn addressing mode event
// Base is given as parameter
uint8_t XcpDaemonEventExt(tXcpDaqLists* daq_lists, uint16_t event, const uint8_t* base) {

    // Cal
#ifdef XCP_ENABLE_DYN_ADDRESSING
    if (!isStarted()) return CRC_CMD_OK;

    // Check if a pending command can be executed in this context
    // @@@@ ToDo: Optimize with atomics, this is performance critical as cal events may come from different threads 
#if defined(XCP_ENABLE_MULTITHREAD_CAL_EVENTS) 
    mutexLock(&gXcp.CmdPendingMutex);
#endif
    BOOL cmdPending = FALSE;
    if ((gXcp.SessionStatus & SS_CMD_PENDING) != 0) {
        if (gXcp.MtaExt == XCP_ADDR_EXT_DYN && (uint16_t)(gXcp.MtaAddr >> 16) == event) {
            gXcp.SessionStatus &= ~SS_CMD_PENDING;
            cmdPending = TRUE;
        }
    }
#if defined(XCP_ENABLE_MULTITHREAD_CAL_EVENTS) 
    mutexUnlock(&gXcp.CmdPendingMutex);
#endif
    if (cmdPending) {
        // Convert relative signed 16 bit addr in MtaAddr to pointer MtaPtr
        gXcp.MtaPtr = (uint8_t*)(base + (int16_t)(gXcp.MtaAddr & 0xFFFF));
        gXcp.MtaExt = XCP_ADDR_EXT_PTR;
        if (CRC_CMD_OK==XcpAsyncCommand(TRUE,(const uint32_t*)&gXcp.CmdPending, gXcp.CmdPendingLen)) {
          uint8_t cmd = gXcp.CmdPending.b[0];
          if (cmd==CC_SHORT_DOWNLOAD||cmd==CC_DOWNLOAD) return CRC_CMD_PENDING; // Write operation done
        }
        return CRC_CMD_OK; // Another pending operation done
    }
#endif // XCP_ENABLE_DYN_ADDRESSING

    // Daq
    if (!isDaqRunning()) return 0; // DAQ not running
    XcpDaemonTriggerEvent(daq_lists, event, base, 0);
    return 0; 
}


