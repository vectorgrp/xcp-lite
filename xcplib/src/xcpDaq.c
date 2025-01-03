
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
#include "xcpLite.h"    // Protocol layer interface
#include "xcpDaq.h"

#ifndef XCP_ENABLE_DYN_ADDRESSING
#error "DYN addressing mode not supported"
#endif

#ifdef XCP_ENABLE_OVERRUN_INDICATION_PID
#error "Overrun indication PID not supported"
#endif

#ifdef XCP_ENABLE_PACKED_MODE
#error "Packed mode not supported"
#endif

#if defined(XCP_ENABLE_MULTITHREAD_DAQ_EVENTS) && defined(XCP_ENABLE_DAQ_EVENT_LIST)
#error "Not supported"
#endif

#if !defined(XCP_MAX_DAQ_COUNT)
#error "Define XCP_MAX_DAQ_COUNT"
#endif

#if !defined(XCP_MAX_DAQ_COUNT)
#error "Define XCP_MAX_DAQ_COUNT"
#endif


/****************************************************************************/


#if XCP_MAX_DAQ_COUNT>256
  #define ODT_HEADER_SIZE 4 // ODT,align,DAQ_WORD header 
#else
  #define ODT_HEADER_SIZE 2 // ODT,DAQ header
#endif

#define ODT_TIMESTAMP_SIZE 4

/****************************************************************************/
/* DAQ list access helper macros                                            */
/****************************************************************************/

/* Shortcuts */


/* j is absolute odt number */
#define DaqListOdtEntryCount(j) (((daq_lists->odt)[j].last_odt_entry-daq_lists->odt[j].first_odt_entry)+1)
#define DaqListOdtLastEntry(j)  ((daq_lists->odt)[j].last_odt_entry)
#define DaqListOdtFirstEntry(j) ((daq_lists->odt)[j].first_odt_entry)
#define DaqListOdtSize(j)       ((daq_lists->odt)[j].size)

/* n is absolute odtEntry number */
#define OdtEntrySize(n)         ((daq_lists->odt_entry_size)[n])
#define OdtEntryAddr(n)         ((daq_lists->odt_entry_addr)[n])

/* i is daq number */
#define DaqListOdtCount(i)      ((daq_lists->u.daq_list[i].last_odt-daq_lists->u.daq_list[i].first_odt)+1)
#define DaqListLastOdt(i)       daq_lists->u.daq_list[i].last_odt
#define DaqListFirstOdt(i)      daq_lists->u.daq_list[i].first_odt
#define DaqListMode(i)          daq_lists->u.daq_list[i].mode
#define DaqListState(i)         daq_lists->u.daq_list[i].state
#define DaqListEventChannel(i)  daq_lists->u.daq_list[i].event_channel
#define DaqListAddrExt(i)       daq_lists->u.daq_list[i].addr_ext
#define DaqListPriority(i)      daq_lists->u.daq_list[i].priority

#define DaqListCount()          daq_lists->daq_count


void XcpApplPrintDaqLists( const tXcpDaqLists* daq_lists )
{
  int i,e;
  uint16_t daq;

  assert(daq_lists!=NULL);
  assert(daq_lists->res==0xBEAC);

  for (daq=0; daq<DaqListCount(); daq++) {

    printf("DAQ %u:",daq);
    printf(" eventchannel=%04Xh,",DaqListEventChannel(daq));
    printf(" ext=%02Xh,",DaqListAddrExt(daq));
    printf(" firstOdt=%u,",DaqListFirstOdt(daq));
    printf(" lastOdt=%u,",DaqListLastOdt(daq));
    printf(" mode=%02Xh,", DaqListMode(daq));
    printf(" state=%02Xh\n", DaqListState(daq));
    for (i=DaqListFirstOdt(daq);i<=DaqListLastOdt(daq);i++) {
      printf("  ODT %u (%u):",i-DaqListFirstOdt(daq),i);
      printf(" firstOdtEntry=%u, lastOdtEntry=%u, size=%u:\n", DaqListOdtFirstEntry(i), DaqListOdtLastEntry(i),DaqListOdtSize(i));
      for (e=DaqListOdtFirstEntry(i);e<=DaqListOdtLastEntry(i);e++) {
        printf("   ODT_ENTRY %u (%u): %08X,%u\n",e-DaqListOdtFirstEntry(i),e,OdtEntryAddr(e), OdtEntrySize(e));
      }
      
    } /* j */

  }
}





// Trigger daq list
static void XcpApplTriggerDaq(const tXcpDaqLists* daq_lists, uint16_t daq, const uint8_t* base, uint64_t clock) {

      uint8_t *d0;
      uint32_t e, el, odt, hs, n;
      void* handle = NULL;

      // Loop over all ODTs of the current DAQ list
      for (hs=ODT_HEADER_SIZE+ODT_TIMESTAMP_SIZE,odt=DaqListFirstOdt(daq);odt<=DaqListLastOdt(daq);hs=ODT_HEADER_SIZE,odt++)  {

          // Get DTO buffer
          d0 = XcpTlGetTransmitBuffer(&handle, (uint16_t)(DaqListOdtSize(odt) + hs));

         // Buffer overrun
         if (d0 == NULL) {
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
        uint8_t *d = &d0[hs];
        el = DaqListOdtLastEntry(odt);
        while (e <= el) { // inner DAQ loop
          n = OdtEntrySize(e);
          if (n==0) {
            break;
          }
          else if (n==8) {
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
          }
          else {
            memcpy((uint8_t*)d, &base[OdtEntryAddr(e)], n);
            d += n;
          }
          e++;
        } // ODT entry
        
        XcpTlCommitTransmitBuffer(handle, DaqListPriority(daq)!=0 && odt==DaqListLastOdt(daq));
      } /* odt */

}

// Trigger event
void XcpApplEventExtAt(const tXcpDaqLists* daq_lists, uint16_t event, const uint8_t* base, uint64_t clock) {

  uint16_t daq;
  
  assert(daq_lists!=NULL);
  assert(daq_lists->res==0xBEAC);

  // Loop over all active DAQ lists associated to the current event
  for (daq=0; daq<DaqListCount(); daq++) {

      if ((DaqListState(daq) & DAQ_STATE_RUNNING) == 0) continue; // DAQ list not active
      if (DaqListEventChannel(daq) != event) continue; // DAQ list not associated with this event
      XcpApplTriggerDaq(daq_lists, daq,base,clock); // Trigger DAQ list

  } /* daq */
}

// ABS adressing mode event with clock
// Base is ApplXcpGetBaseAddr()
#ifdef XCP_ENABLE_ABS_ADDRESSING
void XcpApplEventAt(const tXcpDaqLists* daq_lists, uint16_t event, uint64_t clock) {
    
    XcpApplEventExtAt(daq_lists, event, ApplXcpGetBaseAddr(), clock);
}
#endif

// ABS addressing mode event
// Base is ApplXcpGetBaseAddr()
#ifdef XCP_ENABLE_ABS_ADDRESSING
void XcpApplEvent(const tXcpDaqLists* daq_lists, uint16_t event) {
    
    XcpApplEventExtAt(daq_lists, event, ApplXcpGetBaseAddr(), ApplXcpGetClock64());
}
#endif

// Dyn addressing mode event
// Base is given as parameter
uint8_t XcpApplEventExt(const tXcpDaqLists* daq_lists, uint16_t event, const uint8_t* base) {

    XcpApplEventExtAt(daq_lists, event, base, ApplXcpGetClock64());
    return 0; 
}
