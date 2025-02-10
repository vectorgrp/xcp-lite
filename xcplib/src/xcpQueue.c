/*----------------------------------------------------------------------------
| File:
|   xcpQueue.c
|
| Description:
|   XCP transport layer queue
|   Multi producer single consumer queue (producer side thread safe, not consumer side)
|   XCP transport layer specific:
|   Queue entries include XCP message header of WORD CTR and LEN type, CTR incremented on push, overflow indication via CTR
|
| Copyright (c) Vector Informatik GmbH. All rights reserved.
| See LICENSE file in the project root for details.
|
 ----------------------------------------------------------------------------*/

#include "xcpQueue.h"

#include <assert.h>   // for assert
#include <stdbool.h>  // for bool
#include <stdint.h>   // for uint32_t, uint64_t, uint8_t, int64_t
#include <stdio.h>    // for NULL, snprintf
#include <inttypes.h> // for PRIu64
#include <stdlib.h>   // for free, malloc
#include <string.h>   // for memcpy, strcmp

#include "dbg_print.h" // for DBG_LEVEL, DBG_PRINT3, DBG_PRINTF4, DBG...
#include "platform.h"  // for platform defines (WIN_, LINUX_, MACOS_) and specific implementation of sockets, clock, thread, mutex
#include "xcpEthTl.h"  // for tXcpDtoMessage

#ifndef _WIN
#include <stdatomic.h>
#else
#ifdef _WIN32_ // @@@@
#error "Windows32 not implemented yet"
#endif

// On Windows 64 we rely on the x86-64 strong memory model and assume atomic 64 bit load/store
// and a mutex for thread safety when incrementing the tail
#define atomic_uint_fast64_t uint64_t
#define atomic_store_explicit(a, b, c) (*(a)) = (b)
#define atomic_load_explicit(a, b) (*(a))
#define atomic_fetch_add_explicit(a, b, c)                                                                                                                                         \
    {                                                                                                                                                                              \
        mutexLock(&queue->h.mutex);                                                                                                                                                \
        (*(a)) += (b);                                                                                                                                                             \
        mutexUnlock(&queue->h.mutex);                                                                                                                                              \
    }
#endif

// #define TEST_LOCK_TIMING
#ifdef TEST_LOCK_TIMING
static uint64_t lockTimeMax = 0;
static uint64_t lockTimeSum = 0;
static uint64_t lockCount = 0;
#define HISTOGRAM_SIZE 20 // 200us in 10us steps
#define HISTOGRAM_STEP 10
static uint64_t lockTimeHistogram[HISTOGRAM_SIZE] = {0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0};
#endif

// Queue entry states
#define RESERVED 0  // Reserved by producer
#define COMMITTED 1 // Committed by producer

#define ENTRY_SIZE (XCPTL_MAX_DTO_SIZE + XCPTL_TRANSPORT_LAYER_HEADER_SIZE)
// #define BUFFER_SIZE ((XCPTL_QUEUE_SIZE + 1) * ENTRY_SIZE) // Buffer size must be one entry larger than queue size

// Queue header
typedef struct {
    atomic_uint_fast64_t head; // Consumer reads from head
    atomic_uint_fast64_t tail; // Producers write to tail
    uint32_t queue_size;       // Size of queue in bytes
    uint32_t buffer_size;      // Size of buffer in bytes
    uint16_t ctr;              // Next DTO data transmit message packet counter
    uint16_t overruns;         // Overrun counter
    uint16_t flush;            // There is a packet in the queue which has priority
    MUTEX mutex;               // Mutex for queue producers
    bool from_memory;          // Queue memory from QueueInitFromMemory
} tQueueHeader;

// Queue
typedef struct {
    tQueueHeader h;
    char buffer[];
} tQueue;

tQueueHandle QueueInitFromMemory(void *queue_buffer, int64_t queue_buffer_size, bool clear_queue, int64_t *out_buffer_size) {

    tQueue *queue = NULL;
    assert(queue_buffer_size <= 0xFFFFFFFF);

    // Allocate the queue memory
    if (queue_buffer == NULL) {
        queue = (tQueue *)malloc(queue_buffer_size);
        assert(queue != NULL);
        queue->h.from_memory = false;
        queue->h.buffer_size = (uint32_t)queue_buffer_size;
        queue->h.queue_size = queue->h.buffer_size - ENTRY_SIZE;
        clear_queue = true;
    }
    // Queue memory is provided by the application
    else if (clear_queue) {
        queue = (tQueue *)queue_buffer;
        queue->h.from_memory = true;
        queue->h.buffer_size = (uint32_t)queue_buffer_size - sizeof(tQueueHeader);
        queue->h.queue_size = queue->h.buffer_size - ENTRY_SIZE;
    }

    // Queue is provided by the application and already initialized
    else {
        queue = (tQueue *)queue_buffer;
        assert(queue->h.from_memory == true);
        assert(queue->h.queue_size == queue->h.buffer_size - ENTRY_SIZE);
    }

    DBG_PRINT3("Init XCP transport layer queue\n");
    DBG_PRINTF3("  XCPTL_MAX_SEGMENT_SIZE=%u, XCPTL_PACKET_ALIGNMENT=%u, queue: %u DTOs of max %u bytes, %uKiB\n", XCPTL_MAX_SEGMENT_SIZE, XCPTL_PACKET_ALIGNMENT,
                queue->h.queue_size / ENTRY_SIZE, ENTRY_SIZE, (uint32_t)((queue->h.buffer_size + sizeof(tQueueHeader)) / 1024));

    if (clear_queue) {
        queue->h.overruns = 0;
        queue->h.ctr = 0;
        queue->h.flush = false;
        mutexInit(&queue->h.mutex, false, 1000);
        atomic_store_explicit(&queue->h.head, 0, memory_order_relaxed);
        atomic_store_explicit(&queue->h.tail, 0, memory_order_relaxed);
    }

    if (out_buffer_size != NULL)
        *out_buffer_size = 0;

    return (tQueueHandle)queue;
}

tQueueHandle QueueInit(int64_t queue_buffer_size) { return QueueInitFromMemory(NULL, queue_buffer_size + sizeof(tQueueHeader), true, NULL); }

void QueueClear(tQueueHandle queueHandle) {
    tQueue *queue = (tQueue *)queueHandle;
    assert(queue != NULL);
    queue->h.overruns = 0;
    atomic_store_explicit(&queue->h.head, 0, memory_order_relaxed);
    atomic_store_explicit(&queue->h.tail, 0, memory_order_relaxed);
}

void QueueDeinit(tQueueHandle queueHandle) {
    tQueue *queue = (tQueue *)queueHandle;
    assert(queue != NULL);

#ifdef TEST_LOCK_TIMING
    printf("QueueDeinit: overruns=%u, lockCount=%" PRIu64 ", maxLockTime=%" PRIu64 "ns,  avgLockTime=%" PRIu64 "ns\n", queue->h.overruns, lockCount, lockTimeMax,
           lockTimeSum / lockCount);
    for (int i = 0; i < HISTOGRAM_SIZE - 1; i++) {
        if (lockTimeHistogram[i])
            printf("%dus: %" PRIu64 "\n", i * 10, lockTimeHistogram[i]);
    }
    if (lockTimeHistogram[HISTOGRAM_SIZE - 1])
        printf(">: %" PRIu64 "\n", lockTimeHistogram[HISTOGRAM_SIZE - 1]);
#endif

    QueueClear(queueHandle);
    mutexDestroy(&queue->h.mutex);

    if (queue->h.from_memory) {
        queue->h.from_memory = false;
    } else {
        free(queue);
    }
}

//-------------------------------------------------------------------------------------------------------------------------------------------------------
// Producer functions
// For multiple producers !!

// Get a buffer for a message with size
tQueueBuffer QueueAcquire(tQueueHandle queueHandle, uint64_t packet_len) {

    tQueue *queue = (tQueue *)queueHandle;
    tXcpDtoMessage *entry = NULL;

    assert(queue != NULL);

    if (packet_len > 0xFFFF - XCPTL_TRANSPORT_LAYER_HEADER_SIZE) {
        tQueueBuffer ret = {
            .buffer = NULL,
            .size = 0,
        };
        return ret;
    }

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

    DBG_PRINTF5("QueueAcquire: len=%" PRIu64 "\n", packet_len);

#ifdef TEST_LOCK_TIMING
    uint64_t c = clockGet();
#endif

    // Producer lock
    mutexLock(&queue->h.mutex);

    uint64_t head = atomic_load_explicit(&queue->h.head, memory_order_relaxed);
    uint64_t tail = atomic_load_explicit(&queue->h.tail, memory_order_relaxed);
    if (queue->h.queue_size - (uint32_t)(head - tail) >= msg_len) {

        // Prepare a new entry
        // Use the ctr as commmit state
        uint32_t offset = head % queue->h.queue_size;
        entry = (tXcpDtoMessage *)(queue->buffer + offset);
        entry->ctr = RESERVED;

        atomic_store_explicit(&queue->h.head, head + msg_len, memory_order_relaxed);
    }

    mutexUnlock(&queue->h.mutex);

#ifdef TEST_LOCK_TIMING
    uint64_t d = clockGet() - c;
    mutexLock(&queue->h.mutex);
    if (d > lockTimeMax)
        lockTimeMax = d;
    int i = (d / 1000) / 10;
    if (i < HISTOGRAM_SIZE)
        lockTimeHistogram[i]++;
    else
        lockTimeHistogram[HISTOGRAM_SIZE - 1]++;
    lockTimeSum += d;
    lockCount++;
    mutexUnlock(&queue->h.mutex);
#endif

    if (entry == NULL) {
        queue->h.overruns++;
        tQueueBuffer ret = {
            .buffer = NULL,
            .size = 0,
        };
        return ret;
    }

    entry->dlc = msg_len - XCPTL_TRANSPORT_LAYER_HEADER_SIZE;
    assert((((uint64_t)entry) & 0x3) == 0); // Check alignment

    tQueueBuffer ret = {
        .buffer = entry->data,
        .size = (int16_t)packet_len,
    };
    return ret;
}

// Commit a buffer (by handle returned from XcpTlGetTransmitBuffer)
void QueuePush(tQueueHandle queueHandle, tQueueBuffer *const queueBuffer, bool flush) {

    tQueue *queue = (tQueue *)queueHandle;
    tXcpDtoMessage *entry = (tXcpDtoMessage *)(queueBuffer->buffer - XCPTL_TRANSPORT_LAYER_HEADER_SIZE);
    assert((uint8_t *)entry->data - (uint8_t *)entry == XCPTL_TRANSPORT_LAYER_HEADER_SIZE);

    if (flush)
        queue->h.flush = true;
    entry->ctr = COMMITTED;

    DBG_PRINTF5("QueuePush: dlc=%d, pid=%u, flush=%u, overruns=%u\n", entry->dlc, entry->data[0], flush, queue->h.overruns);
}

// Empty the queue, even if a message is not completely used
void QueueFlush(tQueueHandle queueHandle) {
    tQueue *queue = (tQueue *)queueHandle;
    assert(queue != NULL);
    queue->h.flush = true;
}

//-------------------------------------------------------------------------------------------------------------------------------------------------------
// Consumer functions
// Single consumer thread !!!!!!!!!!
// The consumer is lock free against the providers, it does not contend for the mutex or spinlock used by the providers

// Get transmit queue level in bytes
// This function is thread safe, any thread can ask for the queue level
uint32_t QueueLevel(tQueueHandle queueHandle) {
    tQueue *queue = (tQueue *)queueHandle;
    if (queue == NULL)
        return 0;
    uint64_t head = atomic_load_explicit(&queue->h.head, memory_order_relaxed);
    uint64_t tail = atomic_load_explicit(&queue->h.tail, memory_order_relaxed);
    return (uint32_t)(head - tail);
}

// Check if there is a message segment in the transmit queue with at least one committed packet
// Return the message length and a pointer to the message
tQueueBuffer QueuePeek(tQueueHandle queueHandle) {
    tQueue *queue = (tQueue *)queueHandle;

    assert(queue != NULL);

    uint64_t head = atomic_load_explicit(&queue->h.head, memory_order_relaxed);
    uint64_t tail = atomic_load_explicit(&queue->h.tail, memory_order_relaxed);
    if (head == tail) {
        tQueueBuffer ret = {
            .buffer = NULL,
            .size = 0,
        };
        return ret;
    }

    // Queue is empty
    assert(head - tail <= queue->h.queue_size); // Overrun not handled
    uint32_t level = (uint32_t)(head - tail);
    DBG_PRINTF5("QueuePeek: level=%u, ctr=%u\n", level, queue->h.ctr);

    uint32_t tail_offset = tail % queue->h.queue_size;
    tXcpDtoMessage *entry1 = (tXcpDtoMessage *)(queue->buffer + tail_offset);

    uint16_t ctr1 = entry1->ctr; // entry ctr may be concurrently changed by producer, when committed
    if (ctr1 == RESERVED) {
        tQueueBuffer ret = {
            .buffer = NULL,
            .size = 0,
        };
        return ret; // Not commited yet
    }
    assert(ctr1 == COMMITTED);
    assert(entry1->dlc <= XCPTL_MAX_DTO_SIZE); // Max DTO size

    // Queue overflow indication
    if (queue->h.overruns) { // Add the number of overruns to CTR
        DBG_PRINTF4("QueuePeek: overruns=%u\n", queue->h.overruns);
        queue->h.ctr += queue->h.overruns;
        queue->h.overruns = 0;
    }

    entry1->ctr = queue->h.ctr++; // Set the transport layer packet counter
    uint16_t len = entry1->dlc + XCPTL_TRANSPORT_LAYER_HEADER_SIZE;

    // Check for more packets to concatenate in a meassage segment
    uint16_t len1 = len;
    for (;;) {
        if (len == level)
            break; // Nothing more in queue
        assert(len < level);
        tail_offset += len1;
        if (tail_offset >= queue->h.queue_size)
            break; // Stop, can not wrap around without copying data

        tXcpDtoMessage *entry = (tXcpDtoMessage *)(queue->buffer + tail_offset);
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
        entry->ctr = queue->h.ctr++;
    }

    // DBG_PRINTF5("QueuePeek: msg_len = %u\n", len );

    tQueueBuffer ret = {
        .buffer = (uint8_t *)entry1,
        .size = len,
    };
    return ret;
}

// Advance the transmit queue tail by the message length obtained from the last peek
void QueueRelease(tQueueHandle queueHandle, tQueueBuffer *const queueBuffer) {
    tQueue *queue = (tQueue *)queueHandle;
    assert(queue != NULL);
    DBG_PRINTF5("QueuePeek: msg_len = %u\n", queueBuffer->size);
    if (queueBuffer->size == 0)
        return;
    atomic_fetch_add_explicit(&queue->h.tail, queueBuffer->size, memory_order_relaxed);
    queue->h.flush = false;
}
