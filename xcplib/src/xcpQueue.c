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
#include <inttypes.h> // for PRIu64
#include <stdbool.h>  // for bool
#include <stdint.h>   // for uint32_t, uint64_t, uint8_t, int64_t
#include <stdio.h>    // for NULL, snprintf
#include <stdlib.h>   // for free, malloc
#include <string.h>   // for memcpy, strcmp

#include "dbg_print.h" // for DBG_LEVEL, DBG_PRINT3, DBG_PRINTF4, DBG...
#include "platform.h"  // for platform defines (WIN_, LINUX_, MACOS_) and specific implementation of atomics, sockets, clock, thread, mutex
#include "xcpEthTl.h"  // for tXcpDtoMessage

// Queue entry states
#define RESERVED 0  // Reserved by producer
#define COMMITTED 1 // Committed by producer

#define MAX_ENTRY_SIZE (XCPTL_MAX_DTO_SIZE + XCPTL_TRANSPORT_LAYER_HEADER_SIZE)
#if MAX_ENTRY_SIZE & 4 == 0
#error "MAX_ENTRY_SIZE should be mod 4"
#endif

// Queue header
typedef struct {
    atomic_uint_fast64_t head; // Consumer reads from head
    atomic_uint_fast64_t tail; // Producers write to tail
    uint32_t queue_size;       // Size of queue in bytes (for entry offset wrapping)
    uint32_t buffer_size;      // Size of overall queue data buffer in bytes
    uint16_t ctr;              // Next DTO data transmit message packet counter
    uint16_t overruns;         // Overrun counter
    uint16_t flush;            // There is a packet in the queue which has priority
    MUTEX mutex;               // Mutex for queue producers
    bool from_memory;          // Queue memory from QueueInitFromMemory
    uint8_t reserved[7];       // Header must be 8 byte aligned
} tQueueHeader;

static_assert(((sizeof(tQueueHeader) % 8) == 0), "QueueHeader size must be %8");

// Queue
typedef struct {
    tQueueHeader h;
    char buffer[];
} tQueue;

//-------------------------------------------------------------------------------------------------------------------------------------------------------

tQueueHandle QueueInitFromMemory(void *queue_memory, uint32_t queue_memory_size, bool clear_queue) {

    tQueue *queue = NULL;
    assert(queue_memory_size <= 0xFFFFFFFF); // This implementation does not support larger queues

    // Allocate the queue memory
    if (queue_memory == NULL) {
        queue = (tQueue *)malloc(queue_memory_size);
        assert(queue != NULL);
        queue->h.from_memory = false;
        queue->h.buffer_size = queue_memory_size - sizeof(tQueueHeader);
        queue->h.queue_size = queue->h.buffer_size - MAX_ENTRY_SIZE;
        clear_queue = true;
    }
    // Queue memory is provided by the application
    else if (clear_queue) {
        queue = (tQueue *)queue_memory;
        queue->h.from_memory = true;
        queue->h.buffer_size = (uint32_t)queue_memory_size - sizeof(tQueueHeader);
        queue->h.queue_size = queue->h.buffer_size - MAX_ENTRY_SIZE;
    }

    // Queue is provided by the application and already initialized
    else {
        queue = (tQueue *)queue_memory;
        assert(queue->h.from_memory == true);
        assert(queue->h.queue_size == queue->h.buffer_size - MAX_ENTRY_SIZE);
    }

    DBG_PRINT3("Init XCP transport layer queue\n");
    DBG_PRINTF3("  XCPTL_MAX_SEGMENT_SIZE=%u, XCPTL_PACKET_ALIGNMENT=%u, queue: %u DTOs of max %u bytes, %uKiB\n", XCPTL_MAX_SEGMENT_SIZE, XCPTL_PACKET_ALIGNMENT,
                queue->h.queue_size / MAX_ENTRY_SIZE, MAX_ENTRY_SIZE, (uint32_t)((queue->h.buffer_size + sizeof(tQueueHeader)) / 1024));

    if (clear_queue) {
        queue->h.overruns = 0;
        queue->h.ctr = 0;
        queue->h.flush = false;
        mutexInit(&queue->h.mutex, false, 1000);
        atomic_store_explicit(&queue->h.head, 0, memory_order_relaxed);
        atomic_store_explicit(&queue->h.tail, 0, memory_order_relaxed);
    }

    return (tQueueHandle)queue;
}

tQueueHandle QueueInit(uint32_t queue_buffer_size) { return QueueInitFromMemory(NULL, queue_buffer_size + sizeof(tQueueHeader), true); }

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
tQueueBuffer QueueAcquire(tQueueHandle queueHandle, uint16_t packet_len) {

    tQueue *queue = (tQueue *)queueHandle;
    tXcpDtoMessage *entry = NULL;

    assert(queue != NULL);
    assert(packet_len <= XCPTL_MAX_DTO_SIZE);
    assert(packet_len > 0);

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

    DBG_PRINTF5("QueueAcquire: len=%u\n", packet_len);

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
        .size = packet_len,
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
