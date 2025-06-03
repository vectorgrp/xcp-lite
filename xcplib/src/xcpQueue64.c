/*----------------------------------------------------------------------------
| File:
|   xcpQueue64.c
|
| Description:
|   XCP transport layer queue
|   Multi producer single consumer queue (producer side thread safe)
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
#include "platform.h"  // for platform defines (WIN_, LINUX_, MACOS_) and specific implementation of sockets, clock, thread, mutex
#include "xcpEthTl.h"  // for tXcpDtoMessage

// Check platform
#if (!defined(_LINUX64) && !defined(_MACOS)) || !defined(PLATFORM_64BIT)
#error "This implementation requires a 64 Bit Posix platform (_LINUX64 or _MACOS)"
#endif
static_assert(sizeof(void *) == 8, "This implementation requires a 64 Bit Posix platform (_LINUX64 or _MACOS)"); // This implementation requires 64 Bit Posix platforms

// Check preconditions
#define MAX_ENTRY_SIZE (XCPTL_MAX_DTO_SIZE + XCPTL_TRANSPORT_LAYER_HEADER_SIZE)
#if (MAX_ENTRY_SIZE % 4) != 0
#error "MAX_ENTRY_SIZE should be mod 4"
#endif

//-------------------------------------------------------------------------------------------------------------------------------------------------------

// Enable the lock free queue implementation
// The ony tradeoff is more packet loss in case of overflow, because the complete queue is cleared
#define XCP_QUEUE_LOCK_FREE

//-------------------------------------------------------------------------------------------------------------------------------------------------------
// Test queue acquire lock timing
// Use
//   cargo test  --features=a2l_reader  -- --test-threads=1 --nocapture  --test test_performance
// for high contention

// Note that this test has significant performnce impact !!!!!!!!!!!
// The results do not reflect this, because the test client has higher probability of packet loss on higher burst rate
// Check the debug log output for the packet loss reasons

// #define TEST_LOCK_TIMING
#ifdef TEST_LOCK_TIMING
static MUTEX lockMutex = MUTEX_INTIALIZER;
static uint64_t lockTimeMax = 0;
static uint64_t lockTimeSum = 0;
static uint64_t lockCount = 0;
#define HISTOGRAM_SIZE 20 // 200us in 10us steps
#define HISTOGRAM_STEP 10
static uint64_t lockTimeHistogram[HISTOGRAM_SIZE] = {0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0};
#endif

/*
Comparison of mutex and seq_lock performance with test_performance on MacBook Pro M3 Pro


Mutex:
_________________________________________________________________________
QueueDeinit: lockCount=3959789, maxLockTime=184000ns,  avgLockTime=500ns
0us: 3900900
10us: 32419
20us: 14502
30us: 6392
40us: 2727
50us: 1205
60us: 706
70us: 419
80us: 225
90us: 152
100us: 67
110us: 31
120us: 19
130us: 4
140us: 8
150us: 3
160us: 5
170us: 3
180us: 2

SeqLock:
_________________________________________________________________________
QueueDeinit: lockCount=6749534, maxLockTime=88000ns,  avgLockTime=88ns
0us: 6740465
10us: 7587
20us: 1099
30us: 272
40us: 75
50us: 28
60us: 4
70us: 1
80us: 3
*/

//-------------------------------------------------------------------------------------------------------------------------------------------------------

// Queue entry states
#define RESERVED 0  // Reserved by producer
#define COMMITTED 1 // Committed by producer

// Queue header
typedef struct {
    // Shared state
    atomic_uint_fast64_t head;         // Consumer reads from head
    atomic_uint_fast64_t tail;         // Producers write to tail
    atomic_uint_fast64_t packets_lost; // Packet lost counter, incremented by producers when a queue entry could not be acquired

#ifdef XCP_QUEUE_LOCK_FREE
    atomic_uint_fast64_t overrun_stall; // Overrun counter, incremented by producers when they detect queue overflow
    // seq_lock is used to aquire an entry safely
    // it is incremented by 0x0000000100000000 on lock and 0x0000000000000001 on unlock
    atomic_uint_fast64_t seq_lock;
#else
    MUTEX mutex; // Mutex for queue producers
#endif

    // Single threaded consumer state
    uint16_t ctr; // Next DTO data transmit message packet counter

    // Constant
    uint32_t queue_size;  // Size of queue in bytes (for entry offset wrapping)
    uint32_t buffer_size; // Size of overall queue data buffer in bytes
    bool from_memory;     // Queue memory from QueueInitFromMemory
    uint8_t reserved[7];  // Header must be 8 byte aligned
} tQueueHeader;

static_assert(((sizeof(tQueueHeader) % 8) == 0), "QueueHeader size must be %8");

// Queue
typedef struct {
    tQueueHeader h;
    char buffer[];
} tQueue;

//-------------------------------------------------------------------------------------------------------------------------------------------------------

// Initialize a queue from given memory, a given existing queue or allocate a new queue
tQueueHandle QueueInitFromMemory(void *queue_memory, uint32_t queue_memory_size, bool clear_queue) {

    tQueue *queue = NULL;

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
        queue->h.buffer_size = queue_memory_size - sizeof(tQueueHeader);
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
#ifndef XCP_QUEUE_LOCK_FREE
        mutexInit(&queue->h.mutex, false, 1000);
#endif
        queue->h.ctr = 0;
        QueueClear((tQueueHandle)queue); // Clear the queue
    }

    // Check for lock free atomic head and tail
    // @@@@ TODO Be sure that fetch_add is lock free
    assert(atomic_is_lock_free(&((tQueue *)queue_memory)->h.head));

    return (tQueueHandle)queue;
}

tQueueHandle QueueInit(uint32_t queue_buffer_size) { return QueueInitFromMemory(NULL, queue_buffer_size + sizeof(tQueueHeader), true); }

// Clear the queue
void QueueClear(tQueueHandle queueHandle) {
    tQueue *queue = (tQueue *)queueHandle;
    assert(queue != NULL);

    atomic_store_explicit(&queue->h.head, 0, memory_order_relaxed);
    atomic_store_explicit(&queue->h.tail, 0, memory_order_relaxed);
    atomic_store_explicit(&queue->h.packets_lost, 0, memory_order_seq_cst);
#ifdef XCP_QUEUE_LOCK_FREE
    atomic_store_explicit(&queue->h.seq_lock, 0, memory_order_relaxed);
    // Must be last
    atomic_store_explicit(&queue->h.overrun_stall, 0, memory_order_seq_cst);
#endif
}

// Deinitialize the queue
void QueueDeinit(tQueueHandle queueHandle) {
    tQueue *queue = (tQueue *)queueHandle;
    assert(queue != NULL);

#ifdef TEST_LOCK_TIMING
    printf("QueueDeinit: lockCount=%" PRIu64 ", maxLockTime=%" PRIu64 "ns,  avgLockTime=%" PRIu64 "ns\n", lockCount, lockTimeMax, lockTimeSum / lockCount);
    for (int i = 0; i < HISTOGRAM_SIZE - 1; i++) {
        if (lockTimeHistogram[i])
            printf("%dus: %" PRIu64 "\n", i * 10, lockTimeHistogram[i]);
    }
    if (lockTimeHistogram[HISTOGRAM_SIZE - 1])
        printf(">: %" PRIu64 "\n", lockTimeHistogram[HISTOGRAM_SIZE - 1]);
#endif

    QueueClear(queueHandle);
#ifndef XCP_QUEUE_LOCK_FREE
    mutexDestroy(&queue->h.mutex);
#endif
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

#ifdef TEST_LOCK_TIMING
    uint64_t c = clockGet();
#endif

#ifdef XCP_QUEUE_LOCK_FREE

    // Producer SeqLock
    atomic_fetch_add_explicit(&queue->h.seq_lock, 0x0000000100000000, memory_order_relaxed);

    // Check overrun stall
    if (atomic_load_explicit(&queue->h.overrun_stall, memory_order_relaxed) == 0) {

        // Check if there is enough space in the queue
        uint64_t head = atomic_fetch_add_explicit(&queue->h.head, msg_len, memory_order_relaxed);
        uint64_t tail = atomic_load_explicit(&queue->h.tail, memory_order_relaxed);
        if (head - tail < queue->h.queue_size) {

            // Prepare a new entry, use the ctr as commmit state
            uint32_t offset = head % queue->h.queue_size;
            entry = (tXcpDtoMessage *)(queue->buffer + offset);
            entry->ctr = RESERVED;
        } else {
            // Stall on overflow
            atomic_fetch_add_explicit(&queue->h.overrun_stall, 1, memory_order_relaxed);
        }
    }

    atomic_fetch_add_explicit(&queue->h.seq_lock, 0x0000000000000001, memory_order_relaxed);

#else
    // Producer mutex lock
    mutexLock(&queue->h.mutex);

    uint64_t head = atomic_load_explicit(&queue->h.head, memory_order_relaxed);
    uint64_t tail = atomic_load_explicit(&queue->h.tail, memory_order_relaxed);
    if (queue->h.queue_size - (uint32_t)(head - tail) >= msg_len) {

        // Prepare a new entry, use the ctr as commmit state
        uint32_t offset = head % queue->h.queue_size;
        entry = (tXcpDtoMessage *)(queue->buffer + offset);
        entry->ctr = RESERVED;

        atomic_store_explicit(&queue->h.head, head + msg_len, memory_order_relaxed);
    } else {
        atomic_fetch_add_explicit(&queue->h.packets_lost, 1, memory_order_relaxed);
    }

    mutexUnlock(&queue->h.mutex);
#endif

#ifdef TEST_LOCK_TIMING
    uint64_t d = clockGet() - c;
    mutexLock(&lockMutex);
    if (d > lockTimeMax)
        lockTimeMax = d;
    int i = (d / 1000) / 10;
    if (i < HISTOGRAM_SIZE)
        lockTimeHistogram[i]++;
    else
        lockTimeHistogram[HISTOGRAM_SIZE - 1]++;
    lockTimeSum += d;
    lockCount++;
    mutexUnlock(&lockMutex);
#endif

    if (entry == NULL) {
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

// Commit a buffer (returned from XcpTlGetTransmitBuffer)
void QueuePush(tQueueHandle queueHandle, tQueueBuffer *const queueBuffer, bool flush) {

    (void)flush;       // Not implemented
    (void)queueHandle; // Not used, but required by the function signature

    tXcpDtoMessage *entry = (tXcpDtoMessage *)(queueBuffer->buffer - XCPTL_TRANSPORT_LAYER_HEADER_SIZE);
    assert((uint8_t *)entry->data - (uint8_t *)entry == XCPTL_TRANSPORT_LAYER_HEADER_SIZE);

    // @@@@ TODO Should be an atomic operation in principle, set queue alignment to 8 bytes, check atomic alignment requirements
    entry->ctr = COMMITTED;

    DBG_PRINTF5("QueuePush: dlc=%d\n", entry->dlc);
}

// Empty the queue, even if a message is not completely used
void QueueFlush(tQueueHandle queueHandle) {
    tQueue *queue = (tQueue *)queueHandle;
    assert(queue != NULL);
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

#ifdef XCP_QUEUE_LOCK_FREE

    // If there is a queue overrun
    if (atomic_load_explicit(&queue->h.overrun_stall, memory_order_relaxed) > 0) {

        // Use the SeqLock to clear the queue safely
        // All packets in the queue are lost
        uint64_t seq_lock1, seq_lock2;
        do {
            seq_lock1 = atomic_load_explicit(&queue->h.seq_lock, memory_order_acquire);
            QueueClear(queueHandle); // Clear the queue
            seq_lock2 = atomic_load_explicit(&queue->h.seq_lock, memory_order_acquire);
        } while (seq_lock1 != seq_lock2);
        // As we don't know how many packets were lost, we just set the  packet lost counter to 100
        atomic_store_explicit(&queue->h.packets_lost, 100, memory_order_seq_cst);

        DBG_PRINT_WARNING("QueuePeek: overrun stall, queue cleared\n");
        tQueueBuffer ret = {
            .buffer = NULL,
            .size = 0,
        };
        return ret;
    }

#endif

    // Get a pointer to the next entry in the queue
    uint64_t head = atomic_load_explicit(&queue->h.head, memory_order_relaxed);
    uint64_t tail = atomic_load_explicit(&queue->h.tail, memory_order_relaxed);
    if (head == tail) { // Queue is empty
        tQueueBuffer ret = {
            .buffer = NULL,
            .size = 0,
        };
        return ret;
    }
    assert(head - tail <= queue->h.queue_size); // Overrun not handled
    uint32_t level = (uint32_t)(head - tail);
    uint32_t tail_offset = tail % queue->h.queue_size;
    tXcpDtoMessage *entry1 = (tXcpDtoMessage *)(queue->buffer + tail_offset);
    DBG_PRINTF5("QueuePeek: level=%u\n", level);

    // Read the entry commit state
    // The transport layer XCP message header counter is used as commit state (RESERVED,COMMITTED)
#ifdef XCP_QUEUE_LOCK_FREE

    // Use the seq lock to clear the complete queue safely
    // @@@@ TODO proof, that this really works safely !!!!!!!!!!!!!!!!!!!!!!!!!!!!
    uint64_t seq_lock1, seq_lock2;
    uint16_t ctr1;
    uint32_t spin_count = 0;
    do {
        seq_lock1 = atomic_load_explicit(&queue->h.seq_lock, memory_order_acquire);
        ctr1 = entry1->ctr; // Read the entry commit state
        seq_lock2 = atomic_load_explicit(&queue->h.seq_lock, memory_order_acquire);
        spin_count++;
    } while (seq_lock1 != seq_lock2);
    if (spin_count > 10) {
        DBG_PRINTF_WARNING("QueuePeek: spin_count=%u\n", spin_count);
    }
#else
    // The producer mutex was used to protect the entry commit state
    uint16_t ctr1 = entry1->ctr;
#endif

    // Check the commit state
    if (ctr1 == RESERVED) {
        // Not commited yet, can't read further ahead
        tQueueBuffer ret = {
            .buffer = NULL,
            .size = 0,
        };
        return ret; // Not commited yet
    }
    assert(ctr1 == COMMITTED);
    assert(entry1->dlc <= XCPTL_MAX_DTO_SIZE);

    // Set and increment the transport layer packet counter
    // Queue overflow indication by incrementing transport layer packet counter
    entry1->ctr = queue->h.ctr++;
    uint64_t lost = atomic_load_explicit(&queue->h.packets_lost, memory_order_relaxed);
    if (lost) { // Add the number of overruns to CTR
        atomic_fetch_sub_explicit(&queue->h.packets_lost, lost, memory_order_relaxed);
        DBG_PRINTF_WARNING("QueuePeek: overruns=%u\n", (uint32_t)lost);
        queue->h.ctr += lost;
    }

    // Check for more packets to concatenate in a meassage segment
    uint16_t len = entry1->dlc + XCPTL_TRANSPORT_LAYER_HEADER_SIZE;
    uint16_t len1 = len;
    for (;;) {
        if (len == level)
            break; // Nothing more in queue
        assert(len < level);
        tail_offset += len1;
        if (tail_offset >= queue->h.queue_size)
            break; // Stop, can not wrap around without copying data

        tXcpDtoMessage *entry = (tXcpDtoMessage *)(queue->buffer + tail_offset);

        // @@@@ TODO duplicate code
#ifdef XCP_QUEUE_LOCK_FREE
        uint64_t seq_lock1, seq_lock2;
        uint16_t ctr;
        uint32_t spin_count = 0;
        do {
            seq_lock1 = atomic_load_explicit(&queue->h.seq_lock, memory_order_acquire);
            ctr = entry->ctr; // Read the entry commit state
            seq_lock2 = atomic_load_explicit(&queue->h.seq_lock, memory_order_acquire);
            spin_count++;
        } while (seq_lock1 != seq_lock2);
        if (spin_count > 10) {
            DBG_PRINTF_WARNING("QueuePeek: spin_count=%u\n", spin_count);
        }
#else
        uint16_t ctr = entry->ctr;
#endif
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
    } // for(;;)

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
}
