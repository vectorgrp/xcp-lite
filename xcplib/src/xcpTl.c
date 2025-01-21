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

#include "main.h"
#include "platform.h"
#include "dbg_print.h"
#include "xcpLite.h"
#include "xcpTlQueue.h"

#if defined(_WIN) // Windows
static struct
{
    HANDLE queue_event;
    uint64_t queue_event_time;
} gXcpTl;
#endif

BOOL XcpTlInit()
{

    XcpTlInitTransmitQueue();

    DBG_PRINT3("Init XCP transport layer\n");
    DBG_PRINTF3("  MAX_CTO_SIZE=%u\n", XCPTL_MAX_CTO_SIZE);
#ifdef XCPTL_ENABLE_MULTICAST
    DBG_PRINT3("        Option ENABLE_MULTICAST (not recommended)\n");
#endif

#if defined(_WIN) // Windows
    gXcpTl.queue_event = CreateEvent(NULL, TRUE, FALSE, NULL);
    assert(gXcpTl.queue_event != NULL);
    gXcpTl.queue_event_time = 0;
#endif

    return TRUE;
}

void XcpTlShutdown()
{

    XcpTlFreeTransmitQueue();

#if defined(_WIN) // Windows
    CloseHandle(gXcpTl.queue_event);
#endif
}

// Queue a response or event packet
// If transmission fails, when queue is full, tool times out, retries or take appropriate action
// Note: CANape cancels measurement, when answer to GET_DAQ_CLOCK times out
void XcpTlSendCrm(const uint8_t *packet, uint16_t packet_size)
{

    void *handle = NULL;
    uint8_t *p;

    // Queue the response packet
    if ((p = XcpTlGetTransmitBuffer(&handle, packet_size)) != NULL)
    {
        memcpy(p, packet, packet_size);
        XcpTlCommitTransmitBuffer(handle, TRUE /* flush */);
    }
    else
    { // Buffer overflow
        DBG_PRINT_WARNING("WARNING: queue overflow\n");
        // Ignore, handled by tool
    }
}

// Execute XCP command
// Returns XCP error code
uint8_t XcpTlCommand(uint16_t msgLen, const uint8_t *msgBuf)
{

    BOOL connected = XcpIsConnected();
    tXcpCtoMessage *p = (tXcpCtoMessage *)msgBuf;
    assert(msgLen >= p->dlc + XCPTL_TRANSPORT_LAYER_HEADER_SIZE);

    /* Connected */
    if (connected)
    {
        if (p->dlc > XCPTL_MAX_CTO_SIZE)
            return CRC_CMD_SYNTAX;
        return XcpCommand((const uint32_t *)&p->packet[0], p->dlc); // Handle command
    }

    /* Not connected yet */
    else
    {
        /* Check for CONNECT command ? */
        if (p->dlc == 2 && p->packet[0] == CC_CONNECT)
        {
            XcpTlResetTransmitQueue();
            return XcpCommand((const uint32_t *)&p->packet[0], (uint8_t)p->dlc); // Handle CONNECT command
        }
        else
        {
            DBG_PRINTF_WARNING("WARNING: XcpTlCommand: no valid CONNECT command, dlc=%u, data=%02X\n", p->dlc, p->packet[0]);
            return CRC_CMD_SYNTAX;
        }
    }
}

// Transmit all completed and fully commited UDP frames
// Returns number of bytes sent or -1 on error
int32_t XcpTlHandleTransmitQueue()
{

    const uint32_t max_loops = 20; // maximum number of packets to send without sleep(0)

    int32_t n = 0;
    const uint8_t *b = NULL;

    for (;;)
    {
        for (uint32_t i = 0; i < max_loops; i++)
        {

            // Check
            uint16_t l = 0;
            b = XcpTlTransmitQueuePeekMsg(&l);
            if (b == NULL)
                break; // Ok, queue is empty or not fully commited

            // Send this frame
            int r = XcpEthTlSend(b, l, NULL, 0);
            if (r == (-1))
            { // would block
                b = NULL;
                break;
            }
            if (r == 0)
            { // error
                return -1;
            }
            n += l;

            // Free this buffer when succesfully sent
            XcpTlTransmitQueueNextMsg();

        } // for (max_loops)

        if (b == NULL)
            break; // queue is empty
        sleepMs(0);

    } // for (ever)

    return n; // Ok, queue empty now
}

//-------------------------------------------------------------------------------------------------------

// Notify transmit queue handler thread
BOOL XcpTlNotifyTransmitQueueHandler()
{

    // Windows only, Linux version uses polling
#if defined(_WIN) // Windows
    // Notify when there is finalized buffer in the queue
    // Notify at most every XCPTL_QUEUE_TRANSMIT_CYCLE_TIME to save CPU load
    uint64_t clock = clockGetLast();
    if (clock == gXcpTl.queue_event_time)
        clock = clockGet();
    if (XcpTlTransmitQueueHasMsg() && clock >= gXcpTl.queue_event_time + XCPTL_QUEUE_TRANSMIT_CYCLE_TIME)
    {
        gXcpTl.queue_event_time = clock;
        SetEvent(gXcpTl.queue_event);
        return TRUE;
    }
#endif
    return FALSE;
}

// Wait for outgoing data or timeout after timeout_us
// Return FALSE in case of timeout
BOOL XcpTlWaitForTransmitData(uint32_t timeout_ms)
{

#if defined(_WIN) // Windows

    // Use event triggered for Windows
    if (WAIT_OBJECT_0 == WaitForSingleObject(gXcpTl.queue_event, timeout_ms))
    {
        ResetEvent(gXcpTl.queue_event);
        return TRUE;
    }
    return FALSE;

#elif defined(_LINUX) // Linux

// Use polling for Linux
#define XCPTL_QUEUE_TRANSMIT_POLLING_TIME_MS 1
    uint32_t t = 0;
    while (!XcpTlTransmitQueueHasMsg())
    {
        sleepMs(XCPTL_QUEUE_TRANSMIT_POLLING_TIME_MS);
        t = t + XCPTL_QUEUE_TRANSMIT_POLLING_TIME_MS;
        if (t >= timeout_ms)
            return FALSE;
    }
    return TRUE;

#endif
}
