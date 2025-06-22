/*----------------------------------------------------------------------------
| File:
|   xcpQueue32.c
|
| Description:
|   XCP transport layer queue
|   Multi producer single consumer queue (producer side thread safe, not consumer side)
|   XCP transport layer specific:
|   Queue entries include XCP message header of WORD CTR and WORD LEN type, CTR incremented on pop, overflow indication via CTR
|
| Copyright (c) Vector Informatik GmbH. All rights reserved.
| See LICENSE file in the project root for details.
|
 ----------------------------------------------------------------------------*/

#include "platform.h" // for platform defines (WIN_, LINUX_, MACOS_) and specific implementation of sockets, clock, thread, mutex, spinlock

// Use xcpQueue32.c for 32 Bit platforms or on Windows
#if defined(PLATFORM_32BIT) || defined(_WIN)

#include "xcpQueue.h"

#include <assert.h>   // for assert
#include <inttypes.h> // for PRIu64
#include <stdbool.h>  // for bool
#include <stdint.h>   // for uint32_t, uint64_t, uint8_t, int64_t
#include <stdio.h>    // for NULL, snprintf
#include <stdlib.h>   // for free, malloc
#include <string.h>   // for memcpy, strcmp

#include "dbg_print.h" // for DBG_LEVEL, DBG_PRINT3, DBG_PRINTF4, DBG...
#include "platform.h"  // for platform defines (WIN_, LINUX_, MACOS_) and specific implementation of sockets, clock, thread, mutex, spinlock

#include "xcpEthTl.h"  // for XcpTlGetCtr
#include "xcpTl_cfg.h" // for XCPTL_TRANSPORT_LAYER_HEADER_SIZE, XCPTL_MAX_DTO_SIZE, XCPTL_MAX_SEGMENT_SIZE

/*

Transport Layer segment, message, packet:

    segment (UDP payload, MAX_SEGMENT_SIZE = UDP MTU) = message 1 + message 2 ... + message n
    message = WORD len + WORD ctr + (protocol layer packet) + fill

*/

// Check preconditions
#define MAX_ENTRY_SIZE (XCPTL_MAX_DTO_SIZE + XCPTL_TRANSPORT_LAYER_HEADER_SIZE + 8)
#if (MAX_ENTRY_SIZE % XCPTL_PACKET_ALIGNMENT) != 0
#error "MAX_ENTRY_SIZE should be aligned to XCPTL_PACKET_ALIGNMENT"
#endif

//-------------------------------------------------------------------------------------------------------------------------------------------------------
// Message types

typedef struct {
    uint16_t dlc;     // lenght
    uint16_t ctr;     // message counter
    uint8_t packet[]; // message data
} tXcpMessage;

static_assert(sizeof(tXcpMessage) == XCPTL_TRANSPORT_LAYER_HEADER_SIZE, "tXcpMessage size must be equal to XCPTL_TRANSPORT_LAYER_HEADER_SIZE");

typedef struct {
    uint16_t uncommited;                 // Number of uncommited messages in this segment
    uint16_t size;                       // Number of overall bytes in this segment
    uint8_t msg[XCPTL_MAX_SEGMENT_SIZE]; // Segment/MTU - concatenated transport layer messages
} tXcpMessageBuffer;

typedef struct {

    uint32_t queue_buffer_size; // Size of queue memory allocated in bytes

    uint32_t queue_size; // Size of queue in segments

    // Transmit segment queue
    tXcpMessageBuffer *queue;
    uint32_t queue_rp;          // rp = read index
    uint32_t queue_len;         // rp+len = write index (the next free entry), len=0 ist empty, len=XCPTL_QUEUE_SIZE is full
    tXcpMessageBuffer *msg_ptr; // current incomplete or not fully commited segment

    MUTEX Mutex_Queue;

} tQueue;

//-------------------------------------------------------------------------------------------------------------------------------------------------------

// Allocate a new transmit buffer (transmit queue entry)
// Not thread save!
static void getSegmentBuffer(tQueueHandle queueHandle) {
    tQueue *queue = (tQueue *)queueHandle;
    assert(queue != NULL);

    tXcpMessageBuffer *b;

    /* Check if there is space in the queue */
    if (queue->queue_len >= queue->queue_size) {
        /* Queue overflow */
        queue->msg_ptr = NULL;
    } else {
        unsigned int i = queue->queue_rp + queue->queue_len;
        if (i >= queue->queue_size)
            i -= queue->queue_size;
        b = &queue->queue[i];
        b->size = 0;
        b->uncommited = 0;
        queue->msg_ptr = b;
        queue->queue_len++;
    }
}

// Reserve space for a XCP packet in a transmit segment buffer and return a pointer to packet data and a handle for the segment buffer for commit reference
// Flush the transmit segment buffer, if no space left
static uint8_t *getTransmitBuffer(tQueueHandle queueHandle, void **handlep, uint16_t packet_size) {

    tQueue *queue = (tQueue *)queueHandle;
    assert(queue != NULL);

    tXcpMessage *p;
    uint16_t msg_size;

#if XCPTL_PACKET_ALIGNMENT == 4
    packet_size = (uint16_t)((packet_size + 3) & 0xFFFC); // Add fill %4
#else
    assert(false);
#endif
    msg_size = (uint16_t)(packet_size + XCPTL_TRANSPORT_LAYER_HEADER_SIZE);
    if (msg_size > XCPTL_MAX_SEGMENT_SIZE) {
        return NULL; // Overflow, should never happen in correct DAQ setups
    }

    mutexLock(&queue->Mutex_Queue);

    // Get another message buffer from queue, when active buffer ist full
    if (queue->msg_ptr == NULL || (uint16_t)(queue->msg_ptr->size + msg_size) > XCPTL_MAX_SEGMENT_SIZE) {
        getSegmentBuffer(queueHandle);
    }

    if (queue->msg_ptr != NULL) {

        // Build XCP message header (ctr+dlc) and store in DTO buffer
        p = (tXcpMessage *)&queue->msg_ptr->msg[queue->msg_ptr->size];
        p->ctr = XcpTlGetCtr(); // Get next response packet counter
        p->dlc = (uint16_t)packet_size;
        queue->msg_ptr->size = (uint16_t)(queue->msg_ptr->size + msg_size);
        *((tXcpMessageBuffer **)handlep) = queue->msg_ptr;
        queue->msg_ptr->uncommited++;

    } else {
        p = NULL; // Overflow
    }

    mutexUnlock(&queue->Mutex_Queue);

    if (p == NULL)
        return NULL;      // Overflow
    return &p->packet[0]; // return pointer to XCP message DTO data
}

static void commitTransmitBuffer(tQueueHandle queueHandle, void *handle, BOOL flush) {

    tQueue *queue = (tQueue *)queueHandle;
    assert(queue != NULL);

    tXcpMessageBuffer *p = (tXcpMessageBuffer *)handle;
    if (handle != NULL) {
        mutexLock(&queue->Mutex_Queue);
        p->uncommited--;

        // Flush (high priority data commited)
        if (flush && queue->msg_ptr != NULL && queue->msg_ptr->size > 0) {

            getSegmentBuffer(queueHandle);
        }

        mutexUnlock(&queue->Mutex_Queue);
    }
}

//-------------------------------------------------------------------------------------------------------------------------------------------------------

// Clear the queue
void QueueClear(tQueueHandle queueHandle) {
    tQueue *queue = (tQueue *)queueHandle;
    assert(queue != NULL);
}

// Create and initialize a new queue with a given size
tQueueHandle QueueInit(uint32_t queue_buffer_size) {

    tQueue *queue = (tQueue *)malloc(sizeof(tQueue));
    assert(queue != NULL);

    queue->queue_buffer_size = queue_buffer_size;
    queue->queue_size = queue_buffer_size / sizeof(tXcpMessageBuffer);
    queue->queue = (tXcpMessageBuffer *)malloc(queue->queue_size);
    assert(queue->queue != NULL);

    mutexInit(&queue->Mutex_Queue, 0, 1000);

    mutexLock(&queue->Mutex_Queue);
    queue->queue_rp = 0;
    queue->queue_len = 0;
    queue->msg_ptr = NULL;
    getSegmentBuffer(queue);
    mutexUnlock(&queue->Mutex_Queue);

    assert(queue->msg_ptr);
    return (tQueueHandle)queue;
}

// Deinitialize and free the queue
void QueueDeinit(tQueueHandle queueHandle) {
    tQueue *queue = (tQueue *)queueHandle;
    assert(queue != NULL);
    mutexLock(&queue->Mutex_Queue);
    free(queue->queue);
    queue->queue = NULL;
    queue->queue_buffer_size = 0;
    queue->queue_size = 0;
    queue->queue_rp = 0;
    queue->queue_len = 0;
    queue->msg_ptr = NULL;
    mutexUnlock(&queue->Mutex_Queue);
    mutexDestroy(&queue->Mutex_Queue);
}

//-------------------------------------------------------------------------------------------------------------------------------------------------------
// Producer functions
// For multiple producers !!

// Get a buffer for a message with size
tQueueBuffer QueueAcquire(tQueueHandle queueHandle, uint16_t packet_len) {

    tQueue *queue = (tQueue *)queueHandle;
    assert(queue != NULL);
    assert(packet_len > 0 && packet_len <= XCPTL_MAX_DTO_SIZE);

    void *handle = NULL;
    uint8_t *buffer = getTransmitBuffer(queueHandle, &handle, packet_len);

    assert((uint8_t *)handle == buffer - 4); // @@@@ TODO: preliminary hack to adopt the new queue API

    tQueueBuffer ret = {
        .buffer = buffer,
        .size = packet_len,
    };
    return ret;
}

// Commit a buffer (returned from XcpTlGetTransmitBuffer)
void QueuePush(tQueueHandle queueHandle, tQueueBuffer *const queueBuffer, bool flush) {

    tQueue *queue = (tQueue *)queueHandle;
    assert(queue != NULL);

    commitTransmitBuffer(queueHandle, (uint8_t *)queueBuffer->buffer - 4, flush);
}

//-------------------------------------------------------------------------------------------------------------------------------------------------------
// Consumer functions
// Single consumer thread !!!!!!!!!!

//-------------------------------------------------------------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------------------------------------------------------------

// Transmit all completed and fully commited UDP frames
// Returns number of bytes sent or -1 on error
// int32_t QueueHandleTransmitQueue(tQueueHandle queueHandle) {
//     tQueue *queue = (tQueue *)queueHandle;
//     assert(queue != NULL);

//     const uint32_t max_loops = 20; // maximum number of packets to send without sleep(0)

//     tXcpMessageBuffer *b = NULL;
//     int32_t n = 0;

//     for (;;) {
//         for (uint32_t i = 0; i < max_loops; i++) {

//             // Check
//             mutexLock(&queue->Mutex_Queue);
//             if (queue->queue_len > 1) {
//                 b = &queue->queue[queue->queue_rp];
//                 if (b->uncommited > 0)
//                     b = NULL; // return when reaching a not fully commited segment buffer
//             } else {
//                 b = NULL;
//             }
//             mutexUnlock(&queue->Mutex_Queue);
//             if (b == NULL)
//                 break; // Ok, queue is empty or not fully commited
//             assert(b->size != 0);

//             // Send this frame
//             int r = sendDatagram(&b->msg[0], b->size, NULL, 0);
//             if (r == (-1)) { // would block
//                 b = NULL;
//                 break;
//             }
//             if (r == 0) { // error
//                 return -1;
//             }
//             n += b->size;

//             // Free this buffer when succesfully sent
//             mutexLock(&queue->Mutex_Queue);
//             if (++queue->queue_rp >= queue->queue_size)
//                 queue->queue_rp = 0;
//             queue->queue_len--;
//             mutexUnlock(&queue->Mutex_Queue);

//         } // for (max_loops)

//         if (b == NULL)
//             break; // queue is empty
//         sleepMs(0);

//     } // for (ever)

// #ifdef XCPTL_ENABLE_SELF_TEST
//     if (n > 0) {
//         queue->last_bytes_written = n;
//         queue->total_bytes_written += n;
//         if (queue->last_bytes_written > 0 && gXcpTl_test_event != XCP_INVALID_EVENT) {
//             XcpEvent(gXcpTl_test_event); // Test event, trigger every time the queue is emptied
//         }
//     }
// #endif

//     return n; // Ok, queue empty now
// }

// Flush the current transmit segment buffer, used on high prio event data
// void QueueFlush(tQueueHandle queueHandle) {

//     tQueue *queue = (tQueue *)queueHandle;
//     assert(queue != NULL);

//     // Complete the current buffer if non empty
//     mutexLock(&queue->Mutex_Queue);
//     if (queue->msg_ptr != NULL && queue->msg_ptr->size > 0)
//         getSegmentBuffer(queueHandle);
//     mutexUnlock(&queue->Mutex_Queue);
// }

// Wait until transmit segment queue is empty, used when measurement is stopped
// void QueueWaitUntilEmpty(tQueueHandle queueHandle) {

//     tQueue *queue = (tQueue *)queueHandle;
//     assert(queue != NULL);

//     uint16_t timeout = 0;
//     XcpTlFlushTransmitBuffer(); // Flush the current segment buffer
//     do {
//         sleepMs(20);
//         timeout++;
//     } while (queue->queue_len > 1 && timeout <= 50); // Wait max 1s until the transmit queue is empty
// }

// Get transmit queue level in segments
// This function is thread safe, any thread can ask for the queue level
// Not used by the queue implementation itself
uint32_t QueueLevel(tQueueHandle queueHandle) {
    tQueue *queue = (tQueue *)queueHandle;
    if (queue == NULL)
        return 0;
    return queue->queue_len;
}

// Check if there is a message segment in the transmit queue with at least one committed packet
// Return the message length and a pointer to the message
// Returns the number of packets lost since the last call to QueuePeek
// May not be called twice, each buffer must be released with QueueRelease
// Is not thread safe, must be called from the consumer thread only
tQueueBuffer QueuePeek(tQueueHandle queueHandle, bool flush, uint32_t *packets_lost) {
    tQueue *queue = (tQueue *)queueHandle;
    assert(queue != NULL);

    tQueueBuffer ret = {
        .buffer = NULL,
        .size = 0,
    };
    return ret;
}

// Advance the transmit queue tail by the message length obtained from the last QueuePeek call
void QueueRelease(tQueueHandle queueHandle, tQueueBuffer *const queueBuffer) {
    tQueue *queue = (tQueue *)queueHandle;
    assert(queue != NULL);
    assert(queueBuffer->size > 0 && queueBuffer->size <= XCPTL_MAX_SEGMENT_SIZE);
}

#endif
