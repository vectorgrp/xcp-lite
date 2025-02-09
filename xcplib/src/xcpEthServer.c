/*----------------------------------------------------------------------------
| File:
|   xcpEthServer.c
|
| Description:
|   XCP on UDP Server
|   SHows how to integrate the XCP driver in an application
|   Creates threads for cmd handling and data transmission
|
| Copyright (c) Vector Informatik GmbH. All rights reserved.
| Licensed under the MIT license. See LICENSE file in the project root for details.
|
-----------------------------------------------------------------------------*/

#include "xcpEthServer.h"

#include <assert.h>   // for assert
#include <stdbool.h>  // for bool
#include <stdint.h>   // for uint32_t, uint64_t, uint8_t, int64_t
#include <stdio.h>    // for printf
#include <inttypes.h> // for PRIu64

#include "platform.h"  // for platform defines (WIN_, LINUX_, MACOS_) and specific implementation of sockets, clock, thread, mutex
#include "dbg_print.h" // for DBG_LEVEL, DBG_PRINT3, DBG_PRINTF4, DBG...
#include "xcp.h"       // for CRC_XXX
#include "xcpLite.h"   // for tXcpDaqLists, XcpXxx, ApplXcpXxx, ...
#include "xcptl_cfg.h" // for XCPTL_xxx
#include "xcpTl.h"     // for tXcpCtoMessage, tXcpDtoMessage, xcpTlXxxx
#include "xcpEthTl.h"  // for xcpEthTlxxx
#include "xcpQueue.h"

#if defined(XCPTL_ENABLE_UDP) || defined(XCPTL_ENABLE_TCP)

#if !defined(_WIN) && !defined(_LINUX) && !defined(_MACOS)
#error "Please define platform _WIN, _MACOS or _LINUX"
#endif

#if defined(_WIN) // Windows
static DWORD WINAPI XcpServerReceiveThread(LPVOID lpParameter);
#elif defined(_LINUX) // Linux
static void *XcpServerReceiveThread(void *par);
#endif
#if defined(_WIN) // Windows
static DWORD WINAPI XcpServerTransmitThread(LPVOID lpParameter);
#elif defined(_LINUX) // Linux
static void *XcpServerTransmitThread(void *par);
#endif

static struct {

    bool isInit;

    // Threads
    THREAD TransmitThreadHandle;
    volatile bool TransmitThreadRunning;
    THREAD ReceiveThreadHandle;
    volatile bool ReceiveThreadRunning;

    MUTEX TransmitQueueMutex;

} gXcpServer;

// Check XCP server status
bool XcpEthServerStatus(void) { return gXcpServer.isInit && gXcpServer.TransmitThreadRunning && gXcpServer.ReceiveThreadRunning; }

// XCP server init
bool XcpEthServerInit(const uint8_t *addr, uint16_t port, bool useTCP, uint32_t queueSize) {

    // Check that the XCP singleton has been explicitly initialized
    if (!XcpIsInitialized()) {
        DBG_PRINT_ERROR("XCP not initialized!\n");
        return false;
    }

    // Check if already initialized and running
    if (gXcpServer.isInit) {
        DBG_PRINT_WARNING("XCP server already running!\n");
        return false;
    }

    DBG_PRINT3("Start XCP server\n");

    // Init network sockets
    if (!socketStartup())
        return false;

    gXcpServer.TransmitThreadRunning = false;
    gXcpServer.ReceiveThreadRunning = false;

    // Initialize XCP transport layer
    if (!XcpEthTlInit(addr, port, useTCP, true /*blocking rx*/, queueSize))
        return false;

    // Start XCP protocol layer
    // @@@@ gQueueHandle
    XcpStart(gQueueHandle, false);

    // Create threads
    mutexInit(&gXcpServer.TransmitQueueMutex, false, 0);
    create_thread(&gXcpServer.TransmitThreadHandle, XcpServerTransmitThread);
    create_thread(&gXcpServer.ReceiveThreadHandle, XcpServerReceiveThread);

    gXcpServer.isInit = true;
    return true;
}

bool XcpEthServerShutdown(void) {

#ifdef OPTION_SERVER_FORCEFULL_TERMINATION
    // Forcefull termination
    if (gXcpServer.isInit) {
        DBG_PRINT3("Disconnect, cancel threads and shutdown XCP!\n");
        XcpDisconnect();
        cancel_thread(gXcpServer.ReceiveThreadHandle);
        cancel_thread(gXcpServer.TransmitThreadHandle);
        XcpEthTlShutdown();
        mutexDestroy(&gXcpServer.TransmitQueueMutex);
        gXcpServer.isInit = false;
        socketCleanup();
        XcpReset();
    }
#else
    // Gracefull termination
    if (gXcpServer.isInit) {
        XcpDisconnect();
        gXcpServer.ReceiveThreadRunning = false;
        gXcpServer.TransmitThreadRunning = false;
        XcpEthTlShutdown();
        join_thread(gXcpServer.ReceiveThreadHandle);
        join_thread(gXcpServer.TransmitThreadHandle);
        mutexDestroy(&gXcpServer.TransmitQueueMutex);
        gXcpServer.isInit = false;
        socketCleanup();
        XcpReset();
    }
#endif
    return true;
}

// XCP server unicast command receive thread
#if defined(_WIN) // Windows
DWORD WINAPI XcpServerReceiveThread(LPVOID par)
#elif defined(_LINUX) // Linux
extern void *XcpServerReceiveThread(void *par)
#endif
{
    (void)par;
    DBG_PRINT3("Start XCP CMD thread\n");

    // Receive XCP unicast commands loop
    gXcpServer.ReceiveThreadRunning = true;
    while (gXcpServer.ReceiveThreadRunning) {
        if (!XcpEthTlHandleCommands(XCPTL_TIMEOUT_INFINITE)) { // Timeout Blocking
            DBG_PRINT_ERROR("ERROR: XcpEthTlHandleCommands failed!\n");
            break; // error -> terminate thread
        } else {
            // Handle transmit queue after each command, to keep the command latency short
            mutexLock(&gXcpServer.TransmitQueueMutex);
            int32_t n = XcpTlHandleTransmitQueue();
            mutexUnlock(&gXcpServer.TransmitQueueMutex);
            if (n < 0) {
                DBG_PRINT_ERROR("ERROR: XcpTlHandleTransmitQueue failed!\n");
                break; // error - terminate thread
            }
        }
    }
    gXcpServer.ReceiveThreadRunning = false;

    DBG_PRINT3("XCP receive thread terminated!\n");
    return 0;
}

// XCP server transmit thread
#if defined(_WIN) // Windows
DWORD WINAPI XcpServerTransmitThread(LPVOID par)
#elif defined(_LINUX) // Linux
extern void *XcpServerTransmitThread(void *par)
#endif
{
    (void)par;
    int32_t n;

    DBG_PRINT3("Start XCP DAQ thread\n");

    // Transmit loop
    gXcpServer.TransmitThreadRunning = true;
    while (gXcpServer.TransmitThreadRunning) {

        // Wait for transmit data available, time out at least for required flush cycle
        if (!XcpTlWaitForTransmitData(XCPTL_QUEUE_FLUSH_CYCLE_MS))
            // @@@@ gQueueHandle
            QueueFlush(gQueueHandle); // Flush after timeout to keep data visualization going

        // Transmit all completed messages from the transmit queue
        mutexLock(&gXcpServer.TransmitQueueMutex);
        n = XcpTlHandleTransmitQueue();
        mutexUnlock(&gXcpServer.TransmitQueueMutex);
        if (n < 0) {
            DBG_PRINT_ERROR("ERROR: XcpTlHandleTransmitQueue failed!\n");
            break; // error - terminate thread
        }

    } // for (;;)
    gXcpServer.TransmitThreadRunning = false;

    DBG_PRINT3("XCP transmit thread terminated!\n");
    return 0;
}

#endif