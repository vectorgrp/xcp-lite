#pragma once
/* xcpDaq.h */

/* Copyright(c) Vector Informatik GmbH.All rights reserved.
   Licensed under the MIT license.See LICENSE file in the project root for details. */


/****************************************************************************/
/* Daemon DAQ event handler                                                 */
/****************************************************************************/


/* Trigger a XCP data acquisition or stimulation event */
extern void XcpDaemonEvent(tXcpDaqLists* daq_lists, uint16_t event);
extern uint8_t XcpDaemonEventExt(tXcpDaqLists* daq_lists, uint16_t event, const uint8_t* base);
extern void XcpDaemonEventAt(tXcpDaqLists* daq_lists, uint16_t event, uint64_t clock);



