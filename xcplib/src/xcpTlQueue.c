/*----------------------------------------------------------------------------
| File:
|   xcpTlQueue.c
|
| Description:
|   XCP transport layer queue
|   Multi producer single consumer queue
|
| Copyright (c) Vector Informatik GmbH. All rights reserved.
| See LICENSE file in the project root for details.
|
 ----------------------------------------------------------------------------*/

#include <assert.h>   // for assert
#include <stdbool.h>  // for bool
#include <stdint.h>   // for uint32_t, uint64_t, uint8_t, int64_t
#include <stdio.h>    // for NULL, snprintf
#include <inttypes.h> // for PRIu64
#include <stdlib.h>   // for free, malloc
#include <string.h>   // for memcpy, strcmp
#include <stdatomic.h>

#include "src/platform.h"  // for platform defines (WIN_, LINUX_, MACOS_) and specific implementation of sockets, clock, thread, mutex
#include "src/dbg_print.h" // for DBG_LEVEL, DBG_PRINT3, DBG_PRINTF4, DBG...

#include "src/xcpTl.h" // for tXcpCtoMessage, tXcpDtoMessage, xcpTlXxxx

// #define TEST_LOCK_TIMING

#ifdef TEST_LOCK_TIMING
static uint64_t lockTimeMax = 0;
static uint64_t lockTimeSum = 0;
static uint64_t lockCount = 0;
#define HISTOGRAM_SIZE 20 // 200us in 10us steps
#define HISTOGRAM_STEP 10
static uint64_t lockTimeHistogram[HISTOGRAM_SIZE] = {0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0};
#endif

#if !defined(_WIN) && !defined(_LINUX) && !defined(_MACOS)
#error "Please define platform _WIN, _MACOS or _LINUX"
#endif

#ifndef _WIN

#include <stdatomic.h>

#else

#ifdef _WIN32_
#error "Windows32 not implemented yet"
#else

// On Windows 64 we rely on the x86-64 strong memory model and assume atomic 64 bit load/store
// and a mutex for thread safety when incrementing the tail
#define atomic_uint_fast64_t uint64_t
#define atomic_store_explicit(a, b, c) (*(a)) = (b)
#define atomic_load_explicit(a, b) (*(a))
#define atomic_fetch_add_explicit(a, b, c)                                                                                                                                         \
    {                                                                                                                                                                              \
        mutexLock(&gXcpTlQueue->h.mutex);                                                                                                                                          \
        (*(a)) += (b);                                                                                                                                                             \
        mutexUnlock(&gXcpTlQueue->h.mutex);                                                                                                                                        \
    }

#endif

#endif

// Queue entry states
#define RESERVED 0  // Reserved by producer
#define COMMITTED 1 // Committed by producer

#define MPSC_QUEUE_TRANSMIT_THRESHOLD ((XCPTL_MAX_SEGMENT_SIZE * 100) / 80) // Enough data for transmit, if queue level is 80% of a message

#define ENTRY_SIZE (XCPTL_MAX_DTO_SIZE + XCPTL_TRANSPORT_LAYER_HEADER_SIZE)
#define BUFFER_SIZE ((XCPTL_QUEUE_SIZE + 1) * ENTRY_SIZE) // Buffer size must be one entry larger than queue size

typedef struct {
    atomic_uint_fast64_t head; // Consumer reads from head
    atomic_uint_fast64_t tail; // Producers write to tail
    uint32_t queue_size;       // Size of queue in bytes
    uint32_t buffer_size;      // Size of buffer in bytes
    uint16_t tail_len;         // Length of the next message in the queue (determined by peek)
    uint16_t ctr;              // Next DTO data transmit message packet counter
    uint16_t overruns;         // Overrun counter
    uint16_t flush;            // There is a packet in the queue which has priority
    MUTEX mutex;               // Mutex for queue producers
} tXcpTlQueueHeader;

typedef struct {
    tXcpTlQueueHeader h;
    char buffer[BUFFER_SIZE];
} tXcpTlQueue;

static tXcpTlQueue *gXcpTlQueue = NULL;
static bool gXcpTlQueueExternal = false;

void XcpTlInitTransmitQueue(void *queue, uint32_t queueSize) {

    assert(gXcpTlQueue == NULL);

    // Allocate the queue memory
    if (queue == NULL) {
        gXcpTlQueue = (tXcpTlQueue *)malloc(sizeof(tXcpTlQueue));
        assert(gXcpTlQueue != NULL);
        gXcpTlQueueExternal = false;
        gXcpTlQueue->h.buffer_size = BUFFER_SIZE;
        gXcpTlQueue->h.queue_size = BUFFER_SIZE - ENTRY_SIZE;
    }
    // Queue memory is provided by the application
    else {
        gXcpTlQueue = (tXcpTlQueue *)queue;
        gXcpTlQueueExternal = true;
        gXcpTlQueue->h.buffer_size = queueSize - sizeof(tXcpTlQueueHeader);
        gXcpTlQueue->h.queue_size = queueSize - sizeof(tXcpTlQueueHeader) - ENTRY_SIZE;
    }

    DBG_PRINT3("Init XCP transport layer queue\n");
    DBG_PRINTF3("  XCPTL_MAX_SEGMENT_SIZE=%u, XCPTL_PACKET_ALIGNMENT=%u, queue: %u DTOs of max %u bytes, %uKiB\n", XCPTL_MAX_SEGMENT_SIZE, XCPTL_PACKET_ALIGNMENT,
                gXcpTlQueue->h.queue_size / ENTRY_SIZE, ENTRY_SIZE, (uint32_t)((gXcpTlQueue->h.buffer_size + sizeof(tXcpTlQueueHeader)) / 1024));

    gXcpTlQueue->h.overruns = 0;
    gXcpTlQueue->h.ctr = 0;
    gXcpTlQueue->h.flush = false;
    mutexInit(&gXcpTlQueue->h.mutex, false, 1000);
    atomic_store_explicit(&gXcpTlQueue->h.head, 0, memory_order_relaxed);
    atomic_store_explicit(&gXcpTlQueue->h.tail, 0, memory_order_relaxed);
    gXcpTlQueue->h.tail_len = 0;
}

void XcpTlResetTransmitQueue(void) {
    assert(gXcpTlQueue != NULL);
    gXcpTlQueue->h.tail_len = 0;
    gXcpTlQueue->h.overruns = 0;
    atomic_store_explicit(&gXcpTlQueue->h.head, 0, memory_order_relaxed);
    atomic_store_explicit(&gXcpTlQueue->h.tail, 0, memory_order_relaxed);
}

void XcpTlFreeTransmitQueue(void) {
    assert(gXcpTlQueue != NULL);

#ifdef TEST_LOCK_TIMING
    printf("XcpTlFreeTransmitQueue: overruns=%u, lockCount=%" PRIu64 ", maxLockTime=%" PRIu64 "ns,  avgLockTime=%" PRIu64 "ns\n", gXcpTlQueue->h.overruns, lockCount, lockTimeMax,
           lockTimeSum / lockCount);
    for (int i = 0; i < HISTOGRAM_SIZE - 1; i++) {
        if (lockTimeHistogram[i])
            printf("%dus: %" PRIu64 "\n", i * 10, lockTimeHistogram[i]);
    }
    if (lockTimeHistogram[HISTOGRAM_SIZE - 1])
        printf(">: %" PRIu64 "\n", lockTimeHistogram[HISTOGRAM_SIZE - 1]);
#endif

    XcpTlResetTransmitQueue();
    mutexDestroy(&gXcpTlQueue->h.mutex);

    if (gXcpTlQueueExternal) {
        gXcpTlQueueExternal = false;
    } else {
        free(gXcpTlQueue);
    }
    gXcpTlQueue = NULL;
}

//-------------------------------------------------------------------------------------------------------------------------------------------------------
// Producer functions
// For multiple producers !!

// Get a buffer for a message with size
uint8_t *XcpTlGetTransmitBuffer(void **handle, uint16_t packet_len) {

    tXcpDtoMessage *entry = NULL;

    if (gXcpTlQueue == NULL)
        return NULL;

    // Align the message length
    uint16_t msg_len = packet_len + XCPTL_TRANSPORT_LAYER_HEADER_SIZE;
#if XCPTL_PACKET_ALIGNMENT == 2
    msg_len = (uint16_t)((msg_len + 1) & 0xFFFE); // Add fill %2
#endif
#if XCPTL_PACKET_ALIGNMENT == 4
    msg_len = (uint16_t)((msg_len + 3) & 0xFFFC); // Add fill %4
#endif
#if XCPTL_PACKET_ALIGNMENT == 8
    msg_len = (uint16_t)((msg_len + 7) & 0xFFF8); // Add fill %8
#endif

    DBG_PRINTF5("XcpTlGetTransmitBuffer: len=%d\n", packet_len);

#ifdef TEST_LOCK_TIMING
    uint64_t c = clockGet();
#endif

    // Producer lock
    mutexLock(&gXcpTlQueue->h.mutex);

    uint64_t head = atomic_load_explicit(&gXcpTlQueue->h.head, memory_order_relaxed);
    uint64_t tail = atomic_load_explicit(&gXcpTlQueue->h.tail, memory_order_relaxed);
    if (gXcpTlQueue->h.queue_size - (uint32_t)(head - tail) >= msg_len) {

        // Prepare a new entry
        // Use the ctr as commmit state
        uint32_t offset = head % gXcpTlQueue->h.queue_size;
        entry = (tXcpDtoMessage *)(gXcpTlQueue->buffer + offset);
        entry->ctr = RESERVED;

        atomic_store_explicit(&gXcpTlQueue->h.head, head + msg_len, memory_order_relaxed);
    }

    mutexUnlock(&gXcpTlQueue->h.mutex);

#ifdef TEST_LOCK_TIMING
    uint64_t d = clockGet() - c;
    mutexLock(&gXcpTlQueue->h.mutex);
    if (d > lockTimeMax)
        lockTimeMax = d;
    int i = (d / 1000) / 10;
    if (i < HISTOGRAM_SIZE)
        lockTimeHistogram[i]++;
    else
        lockTimeHistogram[HISTOGRAM_SIZE - 1]++;
    lockTimeSum += d;
    lockCount++;
    mutexUnlock(&gXcpTlQueue->h.mutex);
#endif

    if (entry == NULL) {
        gXcpTlQueue->h.overruns++;
        return NULL;
    }

    entry->dlc = msg_len - XCPTL_TRANSPORT_LAYER_HEADER_SIZE;
    *handle = entry;
    assert((((uint64_t)entry) & 0x3) == 0); // Check alignment
    return entry->data;
}

// Commit a buffer (by handle returned from XcpTlGetTransmitBuffer)
void XcpTlCommitTransmitBuffer(void *handle, bool flush) {

    tXcpDtoMessage *entry = (tXcpDtoMessage *)handle;
    if (flush)
        gXcpTlQueue->h.flush = true;
    entry->ctr = COMMITTED;

#if defined(_WIN) // Windows has event driven transmit queue handler, Linux uses transmit queue polling
    XcpTlNotifyTransmitQueueHandler();
#endif

    DBG_PRINTF5("XcpTlCommitTransmitBuffer: dlc=%d, pid=%u, flush=%u, overruns=%u\n", entry->dlc, entry->data[0], flush, gXcpTlQueue->h.overruns);
}

// Empy the queue, even if a message is not completely used
void XcpTlFlushTransmitBuffer(void) {
    assert(gXcpTlQueue != NULL);
    gXcpTlQueue->h.flush = true;
}

//-------------------------------------------------------------------------------------------------------------------------------------------------------
// Consumer functions
// Single consumer thread !!!!!!!!!!
// The consumer is lock free against the providers, it does not contend for the mutex or spinlock used by the providers

// Get transmit queue level in bytes
// This function is thread safe, any thread can ask for the queue level
static uint32_t XcpTlGetTransmitQueueLevel(void) {
    if (gXcpTlQueue == NULL)
        return 0;
    uint64_t head = atomic_load_explicit(&gXcpTlQueue->h.head, memory_order_relaxed);
    uint64_t tail = atomic_load_explicit(&gXcpTlQueue->h.tail, memory_order_relaxed);
    return (uint32_t)(head - tail);
}

// Wait (sleep) until transmit queue is empty
// This function is thread safe, any thread can wait for transmit queue empty
// Timeout after 1s
bool XcpTlWaitForTransmitQueueEmpty(uint16_t timeout_ms) {

    if (gXcpTlQueue == NULL)
        return true;

    do {
        XcpTlFlushTransmitBuffer(); // Flush the current message
        sleepMs(20);
        if (timeout_ms < 20) { // Wait max timeout_ms until the transmit queue is empty
            DBG_PRINTF_ERROR("XcpTlWaitForTransmitQueueEmpty: timeout! (level=%u)\n", XcpTlGetTransmitQueueLevel());
            return false;
        };
        timeout_ms -= 20;
    } while (XcpTlGetTransmitQueueLevel() != 0);
    return true;
}

// Check if the queu has enough packets to consider transmitting a message
bool XcpTlTransmitQueueHasMsg(void) {

    uint32_t n = XcpTlGetTransmitQueueLevel();
    if (n == 0)
        return false;

    DBG_PRINTF5("XcpTlTransmitHasMsg: level=%u, flush=%u\n", n, gXcpTlQueue->h.flush);

    if (gXcpTlQueue->h.flush)
        return true; // Flush or high priority data in the queue
    if (n > MPSC_QUEUE_TRANSMIT_THRESHOLD)
        return true; // Enough data for a efficient message
    return false;
}

// Check if there is a message segment in the transmit queue with at least one committed packet
// Return the message length and a pointer to the message
const uint8_t *XcpTlTransmitQueuePeekMsg(uint16_t *msg_len) {

    if (gXcpTlQueue == NULL)
        return NULL;

    uint64_t head = atomic_load_explicit(&gXcpTlQueue->h.head, memory_order_relaxed);
    uint64_t tail = atomic_load_explicit(&gXcpTlQueue->h.tail, memory_order_relaxed);
    if (head == tail)
        return NULL;                                  // Queue is empty
    assert(head - tail <= gXcpTlQueue->h.queue_size); // Overrun not handled
    uint32_t level = (uint32_t)(head - tail);
    DBG_PRINTF5("XcpTlTransmitQueuePeekMsg: level=%u, ctr=%u\n", level, gXcpTlQueue->h.ctr);

    uint32_t tail_offset = tail % gXcpTlQueue->h.queue_size;
    tXcpDtoMessage *entry1 = (tXcpDtoMessage *)(gXcpTlQueue->buffer + tail_offset);

    if (gXcpTlQueue->h.tail_len == 0) {

        uint16_t ctr1 = entry1->ctr; // entry ctr may be concurrently changed by producer, when committed
        if (ctr1 == RESERVED)
            return NULL; // Not commited yet
        assert(ctr1 == COMMITTED);
        assert(entry1->dlc <= XCPTL_MAX_DTO_SIZE); // Max DTO size

        if (gXcpTlQueue->h.overruns) { // Add the number of overruns
            DBG_PRINTF4("XcpTlTransmitQueuePeekMsg: overruns=%u\n", gXcpTlQueue->h.overruns);
            gXcpTlQueue->h.ctr += gXcpTlQueue->h.overruns;
            gXcpTlQueue->h.overruns = 0;
        }

        entry1->ctr = gXcpTlQueue->h.ctr++; // Set the transport layer packet counter
        uint16_t len = entry1->dlc + XCPTL_TRANSPORT_LAYER_HEADER_SIZE;

        // Check for more packets to concatenate in a meassage segment
        uint16_t len1 = len;
        for (;;) {
            if (len == level)
                break; // Nothing more in queue
            assert(len < level);
            tail_offset += len1;
            if (tail_offset >= gXcpTlQueue->h.queue_size)
                break; // Stop, can not wrap around without copying data

            tXcpDtoMessage *entry = (tXcpDtoMessage *)(gXcpTlQueue->buffer + tail_offset);
            uint16_t ctr = entry->ctr;
            if (ctr == RESERVED)
                break;
            assert(ctr == COMMITTED);

            // Add this entry
            assert(entry->dlc <= XCPTL_MAX_DTO_SIZE); // Max DTO size
            len1 = entry->dlc + XCPTL_TRANSPORT_LAYER_HEADER_SIZE;
            if (len + len1 > XCPTL_MAX_SEGMENT_SIZE)
                break; // Max segment size reached
            len += len1;
            entry->ctr = gXcpTlQueue->h.ctr++;
        }

        gXcpTlQueue->h.tail_len = len;
    } else {
        assert(0); // @@@@ This may happen, but not observed ever
    }

    // DBG_PRINTF5("XcpTlTransmitQueuePeekMsg: msg_len = %u\n", gXcpTlQueue->h.tail_len );
    *msg_len = gXcpTlQueue->h.tail_len;
    return (uint8_t *)entry1;
}

// Advance the transmit queue tail by the message lentgh obtained from the last peek
void XcpTlTransmitQueueNextMsg(void) {
    assert(gXcpTlQueue != NULL);
    DBG_PRINTF5("XcpTlTransmitQueueNext: msg_len = %u\n", gXcpTlQueue->h.tail_len);
    if (gXcpTlQueue->h.tail_len == 0)
        return;
    atomic_fetch_add_explicit(&gXcpTlQueue->h.tail, gXcpTlQueue->h.tail_len, memory_order_relaxed);
    gXcpTlQueue->h.tail_len = 0;
    gXcpTlQueue->h.flush = false;
}
