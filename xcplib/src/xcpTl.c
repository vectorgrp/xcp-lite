/*----------------------------------------------------------------------------
| File:
|   xcpTl.c
|
| Description:
|   XCP transport layer
|
| Copyright (c) Vector Informatik GmbH. All rights reserved.
| Licensed under the MIT license. See LICENSE file in the project root for details.
|
 ----------------------------------------------------------------------------*/

#include <stddef.h>   // for NULL
#include <assert.h>   // for assert
#include <stdbool.h>  // for bool
#include <stdint.h>   // for uint32_t, uint64_t, uint8_t, int64_t
#include <stdio.h>    // for NULL, snprintf
#include <inttypes.h> // for PRIu64
#include <string.h>   // for memcpy, strcmp

#include "src/platform.h"  // for platform defines (WIN_, LINUX_, MACOS_) and specific implementation of sockets, clock, thread, mutex
#include "src/dbg_print.h" // for DBG_LEVEL, DBG_PRINT3, DBG_PRINTF4, DBG...

#include "src/xcptl_cfg.h" // for XCPTL_xxx

#include "src/xcp.h"      // for CRC_XXX
#include "src/xcpLite.h"  // for tXcpDaqLists, XcpXxx, ApplXcpXxx, ...
#include "src/xcpTl.h"    // for tXcpCtoMessage, tXcpDtoMessage, xcpTlXxxx
#include "src/xcpEthTl.h" // for xcpEthTlxxx
#include "src/xcpQueue.h"

#if !defined(_WIN) && !defined(_LINUX) && !defined(_MACOS)
#error "Please define platform _WIN, _MACOS or _LINUX"
#endif

#if defined(_WIN) // Windows
static struct {
    HANDLE queue_event;
    uint64_t queue_event_time;
} gXcpTl;
#endif

tQueueHandle gQueueHandle = NULL;

bool XcpTlInit(uint32_t queueSize) {

    gQueueHandle = QueueInit(queueSize);

    DBG_PRINT3("Init XCP transport layer\n");
    DBG_PRINTF3("  MAX_CTO_SIZE=%u\n", XCPTL_MAX_CTO_SIZE);
#ifdef XCPTL_ENABLE_MULTICAST
    DBG_PRINT3("        Option ENABLE_MULTICAST (not recommended)\n");
#endif

#if defined(_WIN) // Windows
    gXcpTl.queue_event = CreateEvent(NULL, true, false, NULL);
    assert(gXcpTl.queue_event != NULL);
    gXcpTl.queue_event_time = 0;
#endif

    return true;
}

void XcpTlShutdown(void) {

    // @@@@ gQueueHandle
    QueueDeinit(gQueueHandle);
    gQueueHandle = NULL;

#if defined(_WIN) // Windows
    CloseHandle(gXcpTl.queue_event);
#endif
}

// Execute XCP command
// Returns XCP error code
uint8_t XcpTlCommand(uint16_t msgLen, const uint8_t *msgBuf) {

    bool connected = XcpIsConnected();
    tXcpCtoMessage *p = (tXcpCtoMessage *)msgBuf;
    assert(msgLen >= p->dlc + XCPTL_TRANSPORT_LAYER_HEADER_SIZE);

    /* Connected */
    if (connected) {
        if (p->dlc > XCPTL_MAX_CTO_SIZE)
            return CRC_CMD_SYNTAX;
        return XcpCommand((const uint32_t *)&p->packet[0], p->dlc); // Handle command
    }

    /* Not connected yet */
    else {
        /* Check for CONNECT command ? */
        if (p->dlc == 2 && p->packet[0] == CC_CONNECT) {
            // @@@@ gQueueHandle
            QueueClear(gQueueHandle);
            return XcpCommand((const uint32_t *)&p->packet[0], (uint8_t)p->dlc); // Handle CONNECT command
        } else {
            DBG_PRINTF_WARNING("WARNING: XcpTlCommand: no valid CONNECT command, dlc=%u, data=%02X\n", p->dlc, p->packet[0]);
            return CRC_CMD_SYNTAX;
        }
    }
}

// Transmit all completed and fully commited UDP frames
// Returns number of bytes sent or -1 on error
int32_t XcpTlHandleTransmitQueue(void) {

    const uint32_t max_loops = 20; // maximum number of packets to send without sleep(0)

    int32_t n = 0;
    const uint8_t *b = NULL;

    for (;;) {
        for (uint32_t i = 0; i < max_loops; i++) {

            // Check
            uint16_t l = 0;
            // @@@@ gQueueHandle
            tQueueBuffer queueBuffer = QueuePeek(gQueueHandle);
            l = queueBuffer.size;
            b = queueBuffer.buffer;
            if (b == NULL)
                break; // Ok, queue is empty or not fully commited

            // Send this frame
            int r = XcpEthTlSend(b, l, NULL, 0);
            if (r == (-1)) { // would block
                b = NULL;
                break;
            }
            if (r == 0) { // error
                return -1;
            }
            n += l;

            // Free this buffer when successfully sent
            QueueRelease(gQueueHandle, &queueBuffer);

        } // for (max_loops)

        if (b == NULL)
            break; // queue is empty
        sleepMs(0);

    } // for (ever)

    return n; // Ok, queue empty now
}

// Wait (sleep) until transmit queue is empty
// This function is thread safe, any thread can wait for transmit queue empty
// Timeout after 1s
bool XcpTlWaitForTransmitQueueEmpty(uint16_t timeout_ms) {

    if (gQueueHandle == NULL)
        return true;

    do {
        QueueFlush(gQueueHandle); // Flush the current message
        sleepMs(20);
        if (timeout_ms < 20) { // Wait max timeout_ms until the transmit queue is empty
            DBG_PRINTF_ERROR("XcpTlWaitForTransmitQueueEmpty: timeout! (level=%u)\n", QueueLevel(gQueueHandle));
            return false;
        };
        timeout_ms -= 20;
        // @@@@ gQueueHandle
    } while (QueueLevel(gQueueHandle) != 0);
    return true;
}

//-------------------------------------------------------------------------------------------------------

// Notify transmit queue handler thread
bool XcpTlNotifyTransmitQueueHandler(void) {

    // Windows only, Linux version uses polling
#if defined(_WIN) // Windows
    // Notify when there is finalized buffer in the queue
    // Notify at most every XCPTL_QUEUE_TRANSMIT_CYCLE_TIME to save CPU load
    uint64_t clock = clockGetLast();
    if (clock == gXcpTl.queue_event_time)
        clock = clockGet();
    if (XcpTlTransmitQueueHasMsg() && clock >= gXcpTl.queue_event_time + XCPTL_QUEUE_TRANSMIT_CYCLE_TIME) {
        gXcpTl.queue_event_time = clock;
        SetEvent(gXcpTl.queue_event);
        return true;
    }
#endif
    return false;
}

// Wait for outgoing data or timeout after timeout_us
// Return false in case of timeout
bool XcpTlWaitForTransmitData(uint32_t timeout_ms) {

#if defined(_WIN) // Windows

    // Use event triggered for Windows
    if (WAIT_OBJECT_0 == WaitForSingleObject(gXcpTl.queue_event, timeout_ms)) {
        ResetEvent(gXcpTl.queue_event);
        return true;
    }
    return false;

#elif defined(_LINUX) // Linux

// Use polling for Linux
#define XCPTL_QUEUE_TRANSMIT_POLLING_TIME_MS 1
    uint32_t t = 0;
    // @@@@ gQueueHandle
    while (0 == QueueLevel(gQueueHandle)) {
        sleepMs(XCPTL_QUEUE_TRANSMIT_POLLING_TIME_MS);
        t = t + XCPTL_QUEUE_TRANSMIT_POLLING_TIME_MS;
        if (t >= timeout_ms)
            return false;
    }
    return true;

#endif
}
