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
#include "platform.h"  // for platform defines (WIN_, LINUX_, MACOS_) and specific implementation of sockets, clock, thread, mutex, spinlock

#include "xcpEthTl.h"  // for XcpTlGetCtr
#include "xcpTl_cfg.h" // for XCPTL_TRANSPORT_LAYER_HEADER_SIZE, XCPTL_MAX_DTO_SIZE, XCPTL_MAX_SEGMENT_SIZE

#define CACHE_LINE_SIZE 128u // Cache line size, used to align the queue header

// Check platform
#if (!defined(_LINUX64) && !defined(_MACOS)) || !defined(PLATFORM_64BIT)
#error "This implementation requires a 64 Bit Posix platform (_LINUX64 or _MACOS)"
#endif
static_assert(sizeof(void *) == 8, "This implementation requires a 64 Bit Posix platform (_LINUX64 or _MACOS)"); // This implementation requires 64 Bit Posix platforms

// Check preconditions
#define MAX_ENTRY_SIZE (XCPTL_MAX_DTO_SIZE + XCPTL_TRANSPORT_LAYER_HEADER_SIZE)
#if (MAX_ENTRY_SIZE % XCPTL_PACKET_ALIGNMENT) != 0
#error "MAX_ENTRY_SIZE should be mod 4"
#endif

//-------------------------------------------------------------------------------------------------------------------------------------------------------

// Different queue implementations with different tradeoffs
// The default implementation is a mutex based producer lock, no consumer lock and memory fences between producer and consumer.
// #define QUEUE_LOCK_FREE // Not implemented yet
// #define QUEUE_SPIN_LOCK // Use spin lock instead of mutex for queue producers

// Accumulate XCP packets to multiple XCP messages in a segment obtained with QueuePeek
#define QUEUE_ACCUMULATE_PACKETS // Accumulate XCP packets to multiple XCP messages obtained with QueuePeek

// Wait for at least QUEUE_PEEK_THRESHOLD bytes in the queue before returning a segmentto optimize efficienca
#define QUEUE_PEEK_THRESHOLD XCPTL_MAX_SEGMENT_SIZE

//-------------------------------------------------------------------------------------------------------------------------------------------------------
// Test queue acquire lock timing
// Use
//   cargo test  --features=a2l_reader  -- --test-threads=1 --nocapture  --test test_performance
// for high contention
// Use OPTION_CLOCK_EPOCH_ARB / CLOCK_MONOTONIC_RAW for lower timing noise
//
// Note that this test has significant performance impact !!!!!!!!!!!
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
Comparison of mutex, spinlock and seq_lock performance with test_performance on MacBook Pro M3 Pro

// Mutex:
Lock timing statistics: lockCount=11139387, maxLockTime=4951000ns,  avgLockTime=683ns
0us: 10926125
10us: 120539
20us: 47949
30us: 20240
40us: 10559
50us: 6003
60us: 3432
70us: 2011
80us: 1118
90us: 665
100us: 357
110us: 170
120us: 122
130us: 45
140us: 27
150us: 13
160us: 2
170us: 3
180us: 1
>: 6

// QUEUE_SPIN_LOCK:
Lock timing statistics: lockCount=13857691, maxLockTime=171.151875ms !!!!!!!!,  avgLockTime=6133ns
0us: 13820486
10us: 27274
20us: 4489
30us: 1164
40us: 474
50us: 291
60us: 139
70us: 77
80us: 61
90us: 43
100us: 38
110us: 48
120us: 46
130us: 40
140us: 46
150us: 10
160us: 17
170us: 6
180us: 30
>: 2912

*/

//-------------------------------------------------------------------------------------------------------------------------------------------------------

// Queue entry states
#define RESERVED 1  // Reserved by producer
#define COMMITTED 2 // Committed by producer

// Transport layer message header
#pragma pack(push, 1)
typedef struct {
    uint16_t dlc; // XCP TL header lenght
    uint16_t ctr; // XCP TL Header message counter
    uint8_t data[];
} tXcpDtoMessage;
#pragma pack(pop)

static_assert(sizeof(tXcpDtoMessage) == XCPTL_TRANSPORT_LAYER_HEADER_SIZE, "tXcpDtoMessage size must be equal to XCPTL_TRANSPORT_LAYER_HEADER_SIZE");

// Queue header
// Aligned to cache line size
typedef struct {
    // Shared state
    atomic_uint_fast64_t head;         // Consumer reads from head
    atomic_uint_fast64_t tail;         // Producers write to tail
    atomic_uint_fast32_t packets_lost; // Packet lost counter, incremented by producers when a queue entry could not be acquired
    atomic_bool flush;

#ifdef QUEUE_SPIN_LOCK
    SPINLOCK lock;
#else
    MUTEX mutex; // Mutex for queue producers
#endif
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
    uint8_t buffer[];
} tQueue;

//-------------------------------------------------------------------------------------------------------------------------------------------------------

// Initialize a queue from given memory, a given existing queue or allocate a new queue
static tQueueHandle QueueInitFromMemory(void *queue_memory, uint32_t queue_memory_size, bool clear_queue) {

    tQueue *queue = NULL;

    // Allocate the queue memory
    if (queue_memory == NULL) {
        uint32_t aligned_size = (queue_memory_size + CACHE_LINE_SIZE - 1) & ~(CACHE_LINE_SIZE - 1); // Align to cache line size
        queue = (tQueue *)aligned_alloc(CACHE_LINE_SIZE, aligned_size);
        assert(queue != NULL);
        assert(queue && ((uint64_t)queue % CACHE_LINE_SIZE) == 0); // Check alignment
        memset(queue, 0, queue_memory_size);                       // Clear memory
        queue->h.from_memory = false;
        queue->h.buffer_size = queue_memory_size - sizeof(tQueueHeader);
        queue->h.queue_size = queue->h.buffer_size - MAX_ENTRY_SIZE - (8 * 1024);
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
#if defined(QUEUE_SPIN_LOCK)
        queue->h.lock = 0; // Initialize spin lock
#else
        mutexInit(&queue->h.mutex, false, 1000);
#endif

        QueueClear((tQueueHandle)queue); // Clear the queue
    }

    // Check for lock free atomic head and tail
    // @@@@ TODO Be sure that fetch_add is lock free
    assert(atomic_is_lock_free(&((tQueue *)queue_memory)->h.head));

    DBG_PRINT4("QueueInitFromMemory\n");
    return (tQueueHandle)queue;
}

// Clear the queue
void QueueClear(tQueueHandle queueHandle) {
    tQueue *queue = (tQueue *)queueHandle;
    assert(queue != NULL);

    atomic_store_explicit(&queue->h.head, 0, memory_order_release);
    atomic_store_explicit(&queue->h.tail, 0, memory_order_release);
    atomic_store_explicit(&queue->h.packets_lost, 0, memory_order_release);
    atomic_store_explicit(&queue->h.flush, false, memory_order_relaxed);

    DBG_PRINT4("QueueClear\n");
}

// Create and initialize a new queue with a given size
tQueueHandle QueueInit(uint32_t queue_buffer_size) { return QueueInitFromMemory(NULL, queue_buffer_size + sizeof(tQueueHeader), true); }

// Deinitialize and free the queue
void QueueDeinit(tQueueHandle queueHandle) {
    tQueue *queue = (tQueue *)queueHandle;
    assert(queue != NULL);

#ifdef TEST_LOCK_TIMING
    printf("Lock timing statistics: lockCount=%" PRIu64 ", maxLockTime=%" PRIu64 "ns,  avgLockTime=%" PRIu64 "ns\n", lockCount, lockTimeMax, lockTimeSum / lockCount);
    for (int i = 0; i < HISTOGRAM_SIZE - 1; i++) {
        if (lockTimeHistogram[i])
            printf("%dus: %" PRIu64 "\n", i * 10, lockTimeHistogram[i]);
    }
    if (lockTimeHistogram[HISTOGRAM_SIZE - 1])
        printf(">: %" PRIu64 "\n", lockTimeHistogram[HISTOGRAM_SIZE - 1]);
#endif

    QueueClear(queueHandle);
#if !defined(QUEUE_SPIN_LOCK)
    mutexDestroy(&queue->h.mutex);
#endif

    if (queue->h.from_memory) {
        queue->h.from_memory = false;
    } else {
        free(queue);
    }

    DBG_PRINT4("QueueDeInit\n");
}

//-------------------------------------------------------------------------------------------------------------------------------------------------------
// Producer functions
// For multiple producers !!

// Get a buffer for a message with size
tQueueBuffer QueueAcquire(tQueueHandle queueHandle, uint16_t packet_len) {

    tQueue *queue = (tQueue *)queueHandle;
    assert(queue != NULL);
    assert(packet_len > 0 && packet_len <= XCPTL_MAX_DTO_SIZE);

    tXcpDtoMessage *entry = NULL;

    // Align the message length
    uint16_t msg_len = packet_len + XCPTL_TRANSPORT_LAYER_HEADER_SIZE;
    // #if XCPTL_PACKET_ALIGNMENT == 2
    //     msg_len = (uint16_t)((msg_len + 1) & 0xFFFE); // Add fill %2
    // #endif
    // #if XCPTL_PACKET_ALIGNMENT == 4
    //     msg_len = (uint16_t)((msg_len + 3) & 0xFFFC); // Add fill %4
    // #endif
    // #if XCPTL_PACKET_ALIGNMENT == 8
    //     msg_len = (uint16_t)((msg_len + 7) & 0xFFF8); // Add fill %8
    // #endif
    assert(msg_len <= MAX_ENTRY_SIZE);
    // DBG_PRINTF4("QueueAcquire: len=%u\n", packet_len);

#ifdef TEST_LOCK_TIMING
    uint64_t c = clockGet();
#endif

    // Producer mutex lock
    // To assure queue overrun is detected
    // To assure initialization of the RESERVED state
#if defined(QUEUE_SPIN_LOCK)
    spinLock(&queue->h.lock);
#else
    mutexLock(&queue->h.mutex);
#endif

    uint64_t tail = atomic_load_explicit(&queue->h.tail, memory_order_acquire);
    uint64_t head = atomic_load_explicit(&queue->h.head, memory_order_acquire);
    assert(head >= tail);
    uint32_t level = (uint32_t)(head - tail);
    assert(level <= queue->h.queue_size);
    if (queue->h.queue_size - level >= msg_len) {

        // Prepare a new entry, use the ctr as commmit state
        uint32_t offset = head % queue->h.queue_size;
        entry = (tXcpDtoMessage *)(queue->buffer + offset);
        entry->dlc = msg_len - XCPTL_TRANSPORT_LAYER_HEADER_SIZE;
        entry->ctr = RESERVED;

        atomic_store_explicit(&queue->h.head, head + msg_len, memory_order_release);
    } else {
        uint32_t lost = atomic_fetch_add_explicit(&queue->h.packets_lost, 1, memory_order_acq_rel);
        if (lost == 1)
            DBG_PRINT_WARNING("QueueAcquire: Overrun\n");
    }

#if defined(QUEUE_SPIN_LOCK)
    spinUnlock(&queue->h.lock);
#else
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

    DBG_PRINTF5("QueueAcquire: size=%u, (dlc=%u, ctr=%u)\n", msg_len, entry->dlc, entry->ctr);

    tQueueBuffer ret = {
        .buffer = entry->data,
        .size = msg_len, // Return the size of the complete entry, data buffer size can be larger than requested packet_len !
    };
    return ret;
}

// Commit a buffer (returned from XcpTlGetTransmitBuffer)
void QueuePush(tQueueHandle queueHandle, tQueueBuffer *const queueBuffer, bool flush) {

    tQueue *queue = (tQueue *)queueHandle;
    assert(queue != NULL);

    if (flush) {
        atomic_store_explicit(&queue->h.flush, true, memory_order_relaxed); // Set flush flag, used by the consumer
    }
    assert(queueBuffer->buffer != NULL);
    tXcpDtoMessage *entry = (tXcpDtoMessage *)(queueBuffer->buffer - XCPTL_TRANSPORT_LAYER_HEADER_SIZE);
    DBG_PRINTF5("QueuePush: size=%u (dlc=%u, ctr=%u)\n", queueBuffer->size, entry->dlc, entry->ctr);
    atomic_thread_fence(memory_order_release); // Ensure visibility of the data before before changing the commit state
    entry->ctr = COMMITTED;                    // Commit the entry
}

//-------------------------------------------------------------------------------------------------------------------------------------------------------
// Consumer functions
// Single consumer thread !!!!!!!!!!
// The consumer is lock free against the providers, it does not contend for the lock used by the providers

// Get transmit queue level in bytes
// This function is thread safe, any thread can ask for the queue level
// Not used by the queue implementation itself
uint32_t QueueLevel(tQueueHandle queueHandle) {
    tQueue *queue = (tQueue *)queueHandle;
    if (queue == NULL)
        return 0;
    uint64_t head = atomic_load_explicit(&queue->h.head, memory_order_acquire);
    uint64_t tail = atomic_load_explicit(&queue->h.tail, memory_order_acquire);
    assert(head >= tail);
    assert(head - tail <= queue->h.queue_size);
    return (uint32_t)(head - tail);
}

// Check if there is a message segment in the transmit queue with at least one committed packet
// Return the message length and a pointer to the message
// Returns the number of packets lost since the last call to QueuePeek
// May not be calles twice, each buffer must be released with QueueRelease
// Is not thread safe, must be called from the consumer thread only
tQueueBuffer QueuePeek(tQueueHandle queueHandle, bool flush, uint32_t *packets_lost) {
    tQueue *queue = (tQueue *)queueHandle;
    assert(queue != NULL);

    // Return the number of packets lost in the queue
    if (packets_lost != NULL) {
        uint32_t lost = atomic_exchange_explicit(&queue->h.packets_lost, 0, memory_order_acq_rel);
        *packets_lost = lost;
    }

    uint64_t head, tail;
    uint32_t first_offset;
    uint32_t max_level;
    uint16_t ctr, dlc, len;
    tXcpDtoMessage *first_entry;
    uint16_t total_len = 0;

    // Get a pointer to the next entry in the queue
    // The transport layer XCP message header counter is used as commit state (RESERVED,COMMITTED)
    // Get a pointer to the next entry in the queue
    mutexLock(&queue->h.mutex);
    head = atomic_load_explicit(&queue->h.head, memory_order_acquire);
    tail = atomic_load_explicit(&queue->h.tail, memory_order_acquire);
    mutexUnlock(&queue->h.mutex);
    assert(head >= tail);
    assert(head - tail <= queue->h.queue_size);

    // Check if there is data in the queue
    max_level = (uint32_t)(head - tail);
    assert(max_level <= queue->h.queue_size);
    if (max_level == 0) { // Queue is empty
        // DBG_PRINTF5("%u: QueuePeek: queue is empty, head=%" PRIu64 ", tail=%" PRIu64 ", level=%u\n", (uint32_t)(tail % queue->h.queue_size), head, tail, max_level);
        tQueueBuffer ret = {
            .buffer = NULL,
            .size = 0,
        };
        return ret;
    }
    // Require a minimum amount, to optimize segment count
#if defined(QUEUE_ACCUMULATE_PACKETS) && defined(QUEUE_PEEK_THRESHOLD)
    if ((max_level <= QUEUE_PEEK_THRESHOLD && (flush || atomic_load_explicit(&queue->h.flush, memory_order_relaxed)))) { // Queue is empty or not above the minimum size
        atomic_store_explicit(&queue->h.flush, false, memory_order_relaxed);
        tQueueBuffer ret = {
            .buffer = NULL,
            .size = 0,
        };
        return ret;
    }
#else
    (void)flush;
#endif

    first_offset = tail % queue->h.queue_size;
    first_entry = (tXcpDtoMessage *)(queue->buffer + first_offset);
    DBG_PRINTF5("%u: QueuePeek:  head=%" PRIu64 ", tail=%" PRIu64 ", max_level=%u\n", first_offset, head, tail, max_level);
    // Read the entry commit state
    // The transport layer XCP message header counter is used as commit state (RESERVED,COMMITTED)
    // The producer lock was used to protect the entry commit state and to avoid queue overruns
    // On ARM architectures without TSO (Total Store Order) memory model, the commit state must be read before the data and a memory fences is needed to garantuee visibilty of the
    // data
    atomic_thread_fence(memory_order_acquire); // Ensure visibility of the data before reading the commit state
    ctr = first_entry->ctr;

    // Check the commit state
    if (ctr == RESERVED) {
        DBG_PRINTF5("%u QueuePeek: entry not committed yet, head=%" PRIu64 ", tail=%" PRIu64 ", level=%u\n", first_offset, head, tail, max_level);
        // Not commited yet, can't read further ahead
        tQueueBuffer ret = {
            .buffer = NULL,
            .size = 0,
        };
        return ret; // Not commited yet
    }
    assert(ctr == COMMITTED);

    dlc = first_entry->dlc;
    assert(dlc > 0 && dlc <= XCPTL_MAX_DTO_SIZE);
    len = dlc + XCPTL_TRANSPORT_LAYER_HEADER_SIZE; // Length of the first entry
    total_len = len;

    // Set and increment the transport layer packet counter
    first_entry->ctr = XcpTlGetCtr();

// Check for more packets to concatenate in a meassage segment with maximumun of XCPTL_MAX_SEGMENT_SIZE
#ifdef QUEUE_ACCUMULATE_PACKETS
    uint32_t offset = first_offset + total_len;
    uint32_t max_offset = first_offset + max_level - 1;
    if (max_offset >= queue->h.queue_size) {
        max_offset = queue->h.queue_size - 1; // Don't read over wrap around
        DBG_PRINTF5("%u-%u: QueuePeek: max_offset wrapped around, head=%" PRIu64 ", tail=%" PRIu64 ", max_level=%u, queue_size=%u\n", first_offset, max_offset, head, tail,
                    max_level, queue->h.queue_size);
    }

    for (;;) {

        // Check if there is another entry in the queue to accumulate
        if (offset > max_offset) {
            DBG_PRINTF5("%u: QueuePeek: stop accumulation offset=%u > max_offset=%u\n", offset, offset, max_offset);
            break; // Nothing more safe to read in queue
        }

        tXcpDtoMessage *entry = (tXcpDtoMessage *)(queue->buffer + offset);
        atomic_thread_fence(memory_order_acquire); // Ensure visibility of the data before reading the commit state

        // Check the entry commit state
        uint16_t ctr = entry->ctr;
        DBG_PRINTF5("%u: QueuePeek try accumulate: (ctr=%u, dlc=%u)\n", offset, ctr, entry->dlc);
        if (ctr == RESERVED) {
            DBG_PRINTF5("%u: QueuePeek: entry not committed yet\n", offset);
            break; // Not commited yet, can't read further ahead
        }
        assert(ctr == COMMITTED);

        // Check if the entry fits into the segment
        dlc = entry->dlc;
        assert(dlc > 0 && dlc <= XCPTL_MAX_DTO_SIZE);
        len = dlc + XCPTL_TRANSPORT_LAYER_HEADER_SIZE;
        if (total_len + len > XCPTL_MAX_SEGMENT_SIZE) {
            DBG_PRINTF5("%u: QueuePeek: segment size reached\n", offset);
            break; // Max segment size reached
        }

        // Add this entry
        total_len += len;
        offset += len;
        entry->ctr = XcpTlGetCtr(); // Set and increment the transport layer packet counter
        DBG_PRINTF5("%u: QueuePeek accumulated: (total_len=%u)\n", offset, total_len);

    } // for(;;)
#endif // QUEUE_ACCUMULATE_PACKETS

    DBG_PRINTF5("%u: QueuePeek: returning segment with len=%u, head=%" PRIu64 ", tail=%" PRIu64 ", max_level=%u\n", first_offset, total_len, head, tail, max_level);

    tQueueBuffer ret = {
        .buffer = (uint8_t *)first_entry,
        .size = total_len,
    };
    return ret;
}

// Advance the transmit queue tail by the message length obtained from the last QueuePeek call
void QueueRelease(tQueueHandle queueHandle, tQueueBuffer *const queueBuffer) {
    tQueue *queue = (tQueue *)queueHandle;
    assert(queue != NULL);
    assert(queueBuffer->size > 0 && queueBuffer->size <= XCPTL_MAX_SEGMENT_SIZE);
    atomic_fetch_add_explicit(&queue->h.tail, queueBuffer->size, memory_order_acq_rel);
    DBG_PRINTF5("%llu: QueueRelease: tail advanced by %u bytes\n", (uint64_t)(queueBuffer->buffer - queue->buffer), queueBuffer->size);
}
