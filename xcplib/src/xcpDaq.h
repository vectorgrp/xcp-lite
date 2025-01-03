#pragma once
/* xcpDaq.h */

/* Copyright(c) Vector Informatik GmbH.All rights reserved.
   Licensed under the MIT license.See LICENSE file in the project root for details. */


/****************************************************************************/
/* Daemon DAQ event handler                                                 */
/****************************************************************************/

extern void XcpApplPrintDaqLists( const tXcpDaqLists* daq_lists );

extern void XcpApplEventAt(const tXcpDaqLists* daq_lists, uint16_t event, uint64_t clock);
extern void XcpApplEventExtAt(const tXcpDaqLists* daq_lists, uint16_t event, const uint8_t* base, uint64_t clock);

extern void XcpApplEvent(const tXcpDaqLists* daq_lists, uint16_t event);
extern uint8_t XcpApplEventExt(const tXcpDaqLists* daq_lists, uint16_t event, const uint8_t* base);



