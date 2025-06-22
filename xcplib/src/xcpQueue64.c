/*----------------------------------------------------------------------------
| File:
|   xcpQueue64.c
|
| Description:
|   XCP transport layer queue
|   Multi producer single consumer queue (producer side is thread safe and lockless)
|   Hardcoded for (ODT BYTE, fill BYTE, DAQ WORD,) 4 Byte XCP ODT header types
|   Queue entries include XCP message header, queue can accumulate multiple XCP packets to a segment
|   Lock free with minimal wait implementation using a seq_lock and a spin loop on the producer side
|   Optional mutex based mode for higher consumer throughput as a tradeoff for higher producer latency
|   Testet on ARM weak memory modell
|
| Copyright (c) Vector Informatik GmbH. All rights reserved.
| See LICENSE file in the project root for details.
|
 ----------------------------------------------------------------------------*/

#include "platform.h" // for platform defines (WIN_, LINUX_, MACOS_) and specific implementation of sockets, clock, thread, mutex, spinlock

// Use xcpQueue32.c for 32 Bit platforms or on Windows
#if defined(PLATFORM_64BIT) && !defined(_WIN)

#include "xcpQueue.h"

#include <assert.h>    // for assert
#include <inttypes.h>  // for PRIu64
#include <stdatomic.h> // for atomic_
#include <stdbool.h>   // for bool
#include <stdint.h>    // for uint32_t, uint64_t, uint8_t, int64_t
#include <stdio.h>     // for NULL, snprintf
#include <stdlib.h>    // for free, malloc
#include <string.h>    // for memcpy, strcmp
// #include <stdalign.h> // for alignas

#include "dbg_print.h" // for DBG_LEVEL, DBG_PRINT3, DBG_PRINTF4, DBG...

#include "xcpEthTl.h"  // for XcpTlGetCtr
#include "xcptl_cfg.h" // for XCPTL_TRANSPORT_LAYER_HEADER_SIZE, XCPTL_MAX_DTO_SIZE, XCPTL_MAX_SEGMENT_SIZE

// Turn of misaligned atomic access warnings
// Alignment is assured by the queue header and the queue entry size alignment
#ifdef __GNUC__
#endif
#ifdef __clang__
#pragma GCC diagnostic ignored "-Watomic-alignment"
#endif
#ifdef _MSC_VER
#endif

// Hint to the CPU that we are spinning
#if defined(__x86_64__) || defined(__i386__)
#define spin_loop_hint() __asm__ volatile("pause" ::: "memory")
#elif defined(__aarch64__) || defined(__arm__)
#define spin_loop_hint() __asm__ volatile("yield" ::: "memory");
#else
#define spin_loop_hint() // Fallback: do nothing
#endif

// Assume a maximum cache line size of 128 bytes
#define CACHE_LINE_SIZE 128u // Cache line size, used to align the queue header

// Check for 64 Bit platform
#if (!defined(_LINUX64) && !defined(_MACOS)) || !defined(PLATFORM_64BIT)
#error "This implementation requires a 64 Bit Posix platform (_LINUX64 or _MACOS)"
#endif
static_assert(sizeof(void *) == 8, "This implementation requires a 64 Bit platform"); // This implementation requires 64 Bit Posix platforms

//-------------------------------------------------------------------------------------------------------------------------------------------------------

// Different queue implementations with different tradeoffs
// The default implementation is a mutex based producer lock, no consumer lock and memory fences between producer and consumer.

// Use a mutex for queue producers, this is the default
// #define QUEUE_MUTEX

// Use a seq_lock to protect against inconsistency during the entry acquire, the queue is lockfree with minimal spin wait when contention for increasing the head
#define QUEUE_SEQ_LOCK

// Use a spin lock to acquire an entry, not recommended, see test results
// #define QUEUE_SPIN_LOCK

#if !defined(QUEUE_SEQ_LOCK) && !defined(QUEUE_SPIN_LOCK)
#define QUEUE_MUTEX // Use mutex for queue producers
#endif

// Accumulate XCP packets to multiple XCP messages in a segment obtained with QueuePeek
#define QUEUE_ACCUMULATE_PACKETS // Accumulate XCP packets to multiple XCP messages obtained with QueuePeek

// Wait for at least QUEUE_PEEK_THRESHOLD bytes in the queue before returning a segment, to optimize efficiency
// #define QUEUE_PEEK_THRESHOLD XCPTL_MAX_SEGMENT_SIZE

//-------------------------------------------------------------------------------------------------------------------------------------------------------
// Test queue acquire lock timing and spin lock performance test
// Use
//   cargo test  --features=a2l_reader  -- --test-threads=1 --nocapture  --test test_performance
// for high contention
// Use OPTION_CLOCK_EPOCH_ARB / CLOCK_MONOTONIC_RAW for lower timing noise
//
// Note that this tests have significant performance impact, do not turn on for production use !!!!!!!!!!!

// #define TEST_LOCK_TIMING
#ifdef TEST_LOCK_TIMING
static MUTEX lockMutex = MUTEX_INTIALIZER;
static uint64_t lockTimeMax = 0;
static uint64_t lockTimeSum = 0;
static uint64_t lockCount = 0;
#define LOCK_TIME_HISTOGRAM_SIZE 20 // 200us in 10us steps
#define HISTOGRAM_STEP 10
static uint64_t lockTimeHistogram[LOCK_TIME_HISTOGRAM_SIZE] = {0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0};
#endif

// #define TEST_SPIN_LOCK
#ifdef TEST_SPIN_LOCK
#define SPIN_LOCK_HISTOGRAM_SIZE 100 // Up to 100 loops
static atomic_uint_least32_t spinLockHistogramm[SPIN_LOCK_HISTOGRAM_SIZE] = {
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,

};
#endif

// #define TEST_SEQ_LOCK
#ifdef TEST_SEQ_LOCK
#define SEQ_LOCK_HISTOGRAM_SIZE 200  // Up to 200 loops
static uint32_t seqLockMaxLevel = 0; // Maximum queue level reached
static atomic_uint_least32_t seqLockHistogramm[SEQ_LOCK_HISTOGRAM_SIZE] = {
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,

};
#endif

/*
--------------------------------------------------------------------------------------------------------
Comparison of mutex, spin_lock and seq_lock performance


------------------------------------------------------
Results for test_multi_thread on MacBook Pro M3 Pro

const TEST_TASK_COUNT: usize = 64; // Number of test tasks to create
const TEST_SIGNAL_COUNT: usize = 32; // Number of signals is TEST_SIGNAL_COUNT + 5 for each task
const TEST_DURATION_MS: u64 = 10 * 1000; // Stop after TEST_DURATION_MS milliseconds
const TEST_CYCLE_TIME_US: u32 = 250; // Cycle time in microseconds
const TEST_QUEUE_SIZE: u32 = 1024 * 256; // Size of the XCP server transmit queue in Bytes

// QUEUE_MUTEX:
Lock timing statistics: lockCount=3770338, maxLockTime=146542ns,  avgLockTime=432ns
0ns: 3724782
10ns: 30965
20ns: 9535
30ns: 3027
40ns: 1127
50ns: 462
60ns: 257
70ns: 96
80ns: 29
90ns: 30
100ns: 10
110ns: 13
120ns: 3
130ns: 1
140ns: 1

// QUEUE_SPIN_LOCK:
Lock timing statistics: lockCount=3689814, maxLockTime=10044083ns,  avgLockTime=838ns
0us: 3683171
10us: 5551
20us: 516
30us: 118
40us: 46
50us: 21
60us: 18
70us: 35
80us: 9
90us: 1
120us: 1
130us: 11
>: 316


// QUEUE_SEQ_LOCK:
Lock timing statistics: lockCount=3772703, maxLockTime=49667ns,  avgLockTime=124ns
0ns: 3766733
10ns: 5409
20ns: 477
30ns: 72
40ns: 12

Producer spin wait statistics:
1: 21381
2: 4617
3: 826
4: 96

Consumer spin wait statistics:
2: 166115
3: 2489
4: 2458
5: 2457
6: 2455
7: 2454
8: 2449
9: 2444
10: 2438
11: 2434
12: 2430
13: 2353
14: 2002
15: 1691
16: 1643
17: 1617
18: 1570
19: 1531
20: 1499
21: 1479
22: 1458
23: 1429
24: 1350
25: 1245
26: 1157
27: 1093
28: 1030
29: 987
30: 922
...
96: 306
97: 301
98: 300
99: 296
>100: 443960


------------------------------------------------------
Results for test_multi_thread on Raspberry Pi 5

const TEST_TASK_COUNT: usize = 50; // Number of test tasks to create
const TEST_SIGNAL_COUNT: usize = 32; // Number of signals is TEST_SIGNAL_COUNT + 5 for each task
const TEST_DURATION_MS: u64 = 10 * 1000; // Stop after TEST_DURATION_MS milliseconds
const TEST_CYCLE_TIME_US: u32 = 200; // Cycle time in microseconds
const TEST_QUEUE_SIZE: u32 = 1024 * 256; // Size of the XCP server transmit queue in Bytes


Lock timing statistics: lockCount=1891973, maxLockTime=109167ns,  avgLockTime=146ns
0ns: 1891855
10ns: 52
20ns: 8
30ns: 27
40ns: 23
50ns: 4
70ns: 1
80ns: 1
90ns: 1
100ns: 1


*/

//-------------------------------------------------------------------------------------------------------------------------------------------------------

// Check preconditions
#define MAX_ENTRY_SIZE (XCPTL_MAX_DTO_SIZE + XCPTL_TRANSPORT_LAYER_HEADER_SIZE)
#if (MAX_ENTRY_SIZE % XCPTL_PACKET_ALIGNMENT) != 0
#error "MAX_ENTRY_SIZE should be aligned to XCPTL_PACKET_ALIGNMENT"
#endif

// Queue entry states
#define CTR_RESERVED 0xEEEEu  // Reserved by producer
#define CTR_COMMITTED 0xCCCCu // Committed by producer

#define SIG_RESERVED 0xEEEEEEEEu  // Reserved by producer
#define SIG_COMMITTED 0xCCCCCCCCu // Committed by producer

static_assert(sizeof(atomic_uint_least32_t) == 4, "atomic_uint_least32_t must be 4 bytes");

// Transport layer message header
#pragma pack(push, 1)
typedef struct {
    atomic_uint_least32_t ctr_dlc;
    uint8_t data[];
} tXcpDtoMessage;
#pragma pack(pop)

static_assert(sizeof(tXcpDtoMessage) == XCPTL_TRANSPORT_LAYER_HEADER_SIZE, "tXcpDtoMessage size must be equal to XCPTL_TRANSPORT_LAYER_HEADER_SIZE");

// Queue header
// Aligned to cache line size
typedef struct {
    // Shared state
    atomic_uint_fast64_t head;          // Consumer reads from head
    atomic_uint_fast64_t tail;          // Producers write to tail
    atomic_uint_least32_t packets_lost; // Packet lost counter, incremented by producers when a queue entry could not be acquired
    atomic_bool flush;

#if defined(QUEUE_SPIN_LOCK)
    // A spin lock is used to acquire an entry safely
    SPINLOCK spin_lock; // Spin lock for queue producers, producers contend on each other but not on the consumer

#elif defined(QUEUE_SEQ_LOCK)
    // seq_lock is used to aquire an entry safely
    // A spin loop is used to increment the head
    // It is incremented by 0x0000000100000000 on lock and 0x0000000000000001 on unlock
    atomic_uint_fast64_t seq_lock;
    uint64_t seq_head; // Last head detected as consistent by the seq lock
#else
    MUTEX mutex; // Mutex for queue producers, producers contend on each other but not on the consumer
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
        queue->h.buffer_size = queue_memory_size - (uint32_t)sizeof(tQueueHeader);
        queue->h.queue_size = queue->h.buffer_size - MAX_ENTRY_SIZE;
        clear_queue = true;
    }
    // Queue memory is provided by the caller
    else if (clear_queue) {
        queue = (tQueue *)queue_memory;
        queue->h.from_memory = true;
        queue->h.buffer_size = queue_memory_size - (uint32_t)sizeof(tQueueHeader);
        queue->h.queue_size = queue->h.buffer_size - MAX_ENTRY_SIZE;
    }

    // Queue is provided by the caller and already initialized
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
        spinLockInit(&queue->h.spin_lock); // Initialize the spin lock
#elif defined(QUEUE_SEQ_LOCK)
        queue->h.seq_head = 0;
        atomic_store_explicit(&queue->h.seq_lock, 0, memory_order_relaxed); // Initialize the seq_lock
#else
        mutexInit(&queue->h.mutex, false, 1000);
#endif

        QueueClear((tQueueHandle)queue); // Clear the queue
    }

    // Checks
    assert(atomic_is_lock_free(&((tQueue *)queue_memory)->h.head));
    assert((queue->h.queue_size & (XCPTL_PACKET_ALIGNMENT - 1)) == 0);

    DBG_PRINT4("QueueInitFromMemory\n");
    return (tQueueHandle)queue;
}

// Clear the queue
void QueueClear(tQueueHandle queueHandle) {
    tQueue *queue = (tQueue *)queueHandle;
    assert(queue != NULL);

    atomic_store_explicit(&queue->h.head, 0, memory_order_relaxed);
    atomic_store_explicit(&queue->h.tail, 0, memory_order_relaxed);
    atomic_store_explicit(&queue->h.packets_lost, 0, memory_order_relaxed);
    atomic_store_explicit(&queue->h.flush, false, memory_order_relaxed);
#if defined(QUEUE_SEQ_LOCK)
    queue->h.seq_head = 0;
    atomic_store_explicit(&queue->h.seq_lock, 0, memory_order_relaxed);
#endif
    DBG_PRINT4("QueueClear\n");
}

// Create and initialize a new queue with a given size
tQueueHandle QueueInit(uint32_t queue_buffer_size) { return QueueInitFromMemory(NULL, queue_buffer_size + sizeof(tQueueHeader), true); }

// Deinitialize and free the queue
void QueueDeinit(tQueueHandle queueHandle) {
    tQueue *queue = (tQueue *)queueHandle;
    assert(queue != NULL);

    // Print statistics
#ifdef TEST_LOCK_TIMING
    printf("\nLock timing statistics: lockCount=%" PRIu64 ", maxLockTime=%" PRIu64 "ns,  avgLockTime=%" PRIu64 "ns\n", lockCount, lockTimeMax, lockTimeSum / lockCount);
    for (int i = 0; i < LOCK_TIME_HISTOGRAM_SIZE - 1; i++) {
        if (lockTimeHistogram[i])
            printf("%dns: %" PRIu64 "\n", i * 10, lockTimeHistogram[i]);
    }
    if (lockTimeHistogram[LOCK_TIME_HISTOGRAM_SIZE - 1])
        printf(">%uns: %" PRIu64 "\n", LOCK_TIME_HISTOGRAM_SIZE * 10, lockTimeHistogram[LOCK_TIME_HISTOGRAM_SIZE - 1]);
    printf("\n");
#endif
#ifdef TEST_SPIN_LOCK
    printf("Producer spin wait statistics: \n");
    for (int i = 0; i < SPIN_LOCK_HISTOGRAM_SIZE - 1; i++) {
        if (spinLockHistogramm[i] > 0)
            printf("%d: %u\n", i + 1, spinLockHistogramm[i]);
    }
    if (spinLockHistogramm[SPIN_LOCK_HISTOGRAM_SIZE - 1] > 0)
        printf(">%u: %u\n", SPIN_LOCK_HISTOGRAM_SIZE, spinLockHistogramm[SPIN_LOCK_HISTOGRAM_SIZE - 1]);
    printf("\n");
#endif
#ifdef TEST_SEQ_LOCK
    printf("Consumer seq lock spin wait statistics: \n");
    printf("Max queue level reached: %u of %u, %u%%\n", seqLockMaxLevel, queue->h.queue_size, (seqLockMaxLevel * 100) / queue->h.queue_size);
    for (int i = 0; i < SEQ_LOCK_HISTOGRAM_SIZE - 1; i++) {
        if (seqLockHistogramm[i] > 0)
            printf("%d: %u\n", i + 1, seqLockHistogramm[i]);
    }
    if (seqLockHistogramm[SEQ_LOCK_HISTOGRAM_SIZE - 1] > 0)
        printf(">%u: %u\n", SEQ_LOCK_HISTOGRAM_SIZE, seqLockHistogramm[SEQ_LOCK_HISTOGRAM_SIZE - 1]);
    printf("\n");
#endif

    QueueClear(queueHandle);
#if defined(QUEUE_MUTEX)
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
//-------------------------------------------------------------------------------------------------------------------------------------------------------
// Producer functions
//-------------------------------------------------------------------------------------------------------------------------------------------------------
// For multiple producers !!

// Get a buffer for a message with packet_len bytes
tQueueBuffer QueueAcquire(tQueueHandle queueHandle, uint16_t packet_len) {

    tQueue *queue = (tQueue *)queueHandle;
    assert(queue != NULL);
    assert(packet_len > 0 && packet_len <= XCPTL_MAX_DTO_SIZE);

    tXcpDtoMessage *entry = NULL;

    // Align the message length
    uint16_t msg_len = packet_len + XCPTL_TRANSPORT_LAYER_HEADER_SIZE;
#if XCPTL_PACKET_ALIGNMENT == 2
    msg_len = (uint16_t)((msg_len + 1) & 0xFFFE); // Add fill %2
#error "XCPTL_PACKET_ALIGNMENT == 2 is not supported, use 4"
#endif
#if XCPTL_PACKET_ALIGNMENT == 4
    msg_len = (uint16_t)((msg_len + 3) & 0xFFFC); // Add fill %4
#endif
#if XCPTL_PACKET_ALIGNMENT == 8
    msg_len = (uint16_t)((msg_len + 7) & 0xFFF8); // Add fill %8
#error "XCPTL_PACKET_ALIGNMENT == 8 is not supported, use 4"
#endif

    assert(msg_len <= MAX_ENTRY_SIZE);

#ifdef TEST_LOCK_TIMING
    uint64_t c = clockGet();
#endif

    // Prepare a new entry in reserved state
    // Reserved state has a valid dlc and ctr, ctr is unknown yet and will be marked as CTR_RESERVED for checking

    //----------------------------------------------
    // Use a spin lock to protect the entry acquire
#if defined(QUEUE_SPIN_LOCK)

    spinLock(&queue->h.spin_lock); // Acquire the spin lock
    uint64_t tail = atomic_load_explicit(&queue->h.tail, memory_order_relaxed);
    uint64_t head = atomic_load_explicit(&queue->h.head, memory_order_acquire);
    if (queue->h.queue_size - msg_len >= head - tail) {
        entry = (tXcpDtoMessage *)(queue->buffer + (head % queue->h.queue_size));
        atomic_store_explicit(&entry->ctr_dlc, (CTR_RESERVED << 16) | (uint32_t)(msg_len - XCPTL_TRANSPORT_LAYER_HEADER_SIZE), memory_order_release);
        atomic_store_explicit(&queue->h.head, head + msg_len, memory_order_release);
    }
    spinUnlock(&queue->h.spin_lock); // Release the spin lock

    //----------------------------------------------
    // Use a seq_lock to protect the entry acquire, CAS loop to increment the head
#elif defined(QUEUE_SEQ_LOCK)

    uint64_t tail = atomic_load_explicit(&queue->h.tail, memory_order_relaxed);
    uint64_t head = atomic_load_explicit(&queue->h.head, memory_order_acquire);

    // Consumer is using the seq_lock to acquire a consistent head
    // By making sure no producer is currently in the following sequence, which might have incremented the head, but not set the entry state to not commited yet
    atomic_fetch_add_explicit(&queue->h.seq_lock, 0x0000000100000000, memory_order_acq_rel);
#ifdef TEST_SPIN_LOCK
    uint32_t spin_count = 0;
#endif
    for (;;) {

        // Check for overrun
        if (queue->h.queue_size - msg_len < head - tail) {
            break; // Overrun
        }

        // Try increment the head
        // Compare exchange weak, false negative ok
        if (atomic_compare_exchange_weak_explicit(&queue->h.head, &head, head + msg_len, memory_order_acq_rel, memory_order_acquire)) {
            entry = (tXcpDtoMessage *)(queue->buffer + (head % queue->h.queue_size));
            atomic_store_explicit(&entry->ctr_dlc, (CTR_RESERVED << 16) | (uint32_t)(msg_len - XCPTL_TRANSPORT_LAYER_HEADER_SIZE), memory_order_release);
            break;
        }

        // Get spin count statistics
        // spin_loop_hint(); // No hint, spin count is usually low and the locked sequence should be as fast as possible
        // assert(spin_count < 100); // No reason to be afraid about the spin count, enable spin count statistics to check
#ifdef TEST_SPIN_LOCK
        if (spin_count++ >= SPIN_LOCK_HISTOGRAM_SIZE)
            spin_count = SPIN_LOCK_HISTOGRAM_SIZE - 1;
        atomic_fetch_add_explicit(&spinLockHistogramm[spin_count], 1, memory_order_relaxed);
#endif

    } // for (;;)

    atomic_fetch_add_explicit(&queue->h.seq_lock, 0x0000000000000001, memory_order_acq_rel);

    //----------------------------------------------
    // Use a mutex to protect the entry acquire
#else

    mutexLock(&queue->h.mutex);

    uint64_t tail = atomic_load_explicit(&queue->h.tail, memory_order_relaxed);
    uint64_t head = atomic_load_explicit(&queue->h.head, memory_order_acquire);
    assert(head >= tail);
    if (queue->h.queue_size - msg_len >= head - tail) {
        entry = (tXcpDtoMessage *)(queue->buffer + (head % queue->h.queue_size));
        atomic_store_explicit(&entry->ctr_dlc, (CTR_RESERVED << 16) | (uint32_t)(msg_len - XCPTL_TRANSPORT_LAYER_HEADER_SIZE), memory_order_release);
        atomic_store_explicit(&queue->h.head, head + msg_len, memory_order_release);
    }

    mutexUnlock(&queue->h.mutex);

#endif

#ifdef TEST_LOCK_TIMING
    uint64_t d = clockGet() - c;
    mutexLock(&lockMutex);
    if (d > lockTimeMax)
        lockTimeMax = d;
    int i = (d / 1000) / 10;
    if (i < LOCK_TIME_HISTOGRAM_SIZE)
        lockTimeHistogram[i]++;
    else
        lockTimeHistogram[LOCK_TIME_HISTOGRAM_SIZE - 1]++;
    lockTimeSum += d;
    lockCount++;
    mutexUnlock(&lockMutex);
#endif

    if (entry == NULL) {
        uint32_t lost = atomic_fetch_add_explicit(&queue->h.packets_lost, 1, memory_order_acq_rel);
        if (lost == 0)
            DBG_PRINTF_WARNING("Transmit queue overrun, msg_len=%u, head=%" PRIu64 ", tail=%" PRIu64 ", level=%u, queue_size=%u\n", msg_len, head, tail, (uint32_t)(head - tail),
                               queue->h.queue_size);
        tQueueBuffer ret = {
            .buffer = NULL,
            .size = 0,
        };
        return ret;
    }

    tQueueBuffer ret = {
        .buffer = entry->data,
        .size = msg_len, // Return the size of the complete entry, data buffer size can be larger than requested packet_len !
    };
    return ret;
}

// Commit a buffer (returned from QueueAcquire)
void QueuePush(tQueueHandle queueHandle, tQueueBuffer *const queueBuffer, bool flush) {

    tQueue *queue = (tQueue *)queueHandle;
    assert(queue != NULL);

    // Set flush request
    if (flush) {
        atomic_store_explicit(&queue->h.flush, true, memory_order_relaxed); // Set flush flag, used by the consumer to priorize packets
    }

    assert(queueBuffer != NULL);
    assert(queueBuffer->buffer != NULL);
    tXcpDtoMessage *entry = (tXcpDtoMessage *)(queueBuffer->buffer - XCPTL_TRANSPORT_LAYER_HEADER_SIZE);

    // Go to commit state
    // Complete data is then visible to the consumer
    atomic_store_explicit(&entry->ctr_dlc, (CTR_COMMITTED << 16) | (uint32_t)(queueBuffer->size - XCPTL_TRANSPORT_LAYER_HEADER_SIZE), memory_order_release);
}

//-------------------------------------------------------------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------------------------------------------------------------
// Consumer functions
//-------------------------------------------------------------------------------------------------------------------------------------------------------
// Single consumer thread !!!!!!!!!!
// The consumer does not contend against the providers

// Get current transmit queue level in bytes
// This function is thread safe
// Not used by the queue implementation itself
// Returns 0 when the queue is empty
uint32_t QueueLevel(tQueueHandle queueHandle) {
    tQueue *queue = (tQueue *)queueHandle;
    if (queue == NULL)
        return 0;
    uint64_t head = atomic_load_explicit(&queue->h.head, memory_order_relaxed);
    uint64_t tail = atomic_load_explicit(&queue->h.tail, memory_order_relaxed);
    assert(head >= tail);
    assert(head - tail <= queue->h.queue_size);
    return (uint32_t)(head - tail);
}

// Check if there is a message segment (one or more accumulated packets) in the transmit queue
// Return the message length and a pointer to the message
// Returns the number of packets lost since the last call
// May not be called twice, each buffer must be released immediately with QueueRelease
// Is not thread safe, must be called from one consumer thread only
tQueueBuffer QueuePeek(tQueueHandle queueHandle, bool flush, uint32_t *packets_lost) {
    tQueue *queue = (tQueue *)queueHandle;
    assert(queue != NULL);

    // Return the number of packets lost in the queue
    if (packets_lost != NULL) {
        uint32_t lost = atomic_exchange_explicit(&queue->h.packets_lost, 0, memory_order_acq_rel);
        *packets_lost = lost;
        if (lost) {
            DBG_PRINTF_WARNING("QueuePeek: packets lost since last call: %u\n", lost);
        }
    }

    uint64_t head, tail;
    uint32_t level;

    uint32_t first_offset;
    tXcpDtoMessage *first_entry;

    uint16_t total_len = 0;

    tail = atomic_load_explicit(&queue->h.tail, memory_order_relaxed);

    // Read a consistent head
    // Consistent means, the validity of the commit state for this entry is garantueed

#if defined(QUEUE_SEQ_LOCK)

    // Check if there is data in the queue already verified to be consistent
    level = (uint32_t)(queue->h.seq_head - tail);
    if (level >= XCPTL_MAX_SEGMENT_SIZE) {

        // Use the last head detected as consistent by the seq lock
        head = queue->h.seq_head;
    } else {

        // Spin until the seq_lock is consistent when reading the head
        // This spinning is the tradeoff for lockless on the producer side, it may impact the consumer performance but greatly improves the producer latency
        uint64_t seq_lock1, seq_lock2;
        uint32_t spin_count = 0;
        do {

            seq_lock1 = atomic_load_explicit(&queue->h.seq_lock, memory_order_acquire);
            head = atomic_load_explicit(&queue->h.head, memory_order_acquire);
            seq_lock2 = atomic_load_explicit(&queue->h.seq_lock, memory_order_acquire);

            spin_loop_hint(); // Hint to the CPU that this is a spin loop

            // Limit spinning
            if (spin_count++ >= 50) {
                sleepNs(100000); // Sleep for 100us to reduce CPU load
                spin_count = 0;  // Reset
            }

            // Get spin count statistics
#ifdef TEST_SEQ_LOCK
            if (spin_count >= SEQ_LOCK_HISTOGRAM_SIZE)
                spin_count = SEQ_LOCK_HISTOGRAM_SIZE - 1;
            atomic_fetch_add_explicit(&seqLockHistogramm[spin_count], 1, memory_order_relaxed);
#endif

        } while ((seq_lock1 != seq_lock2) || ((seq_lock1 >> 32) != (seq_lock2 & 0xFFFFFFFF)));

        queue->h.seq_head = head; // Set the last head detected as consistent by the seq lock
    }

#elif defined(QUEUE_SPIN_LOCK)

    spinLock(&queue->h.spin_lock);
    head = atomic_load_explicit(&queue->h.head, memory_order_acquire);
    spinUnlock(&queue->h.spin_lock);

#else

    mutexLock(&queue->h.mutex);
    head = atomic_load_explicit(&queue->h.head, memory_order_relaxed);
    mutexUnlock(&queue->h.mutex);

#endif

    // Check if there is data in the queue
    assert(head >= tail);
    level = (uint32_t)(head - tail);
    assert(level <= queue->h.queue_size);
    if (level == 0) { // Queue is empty
        tQueueBuffer ret = {
            .buffer = NULL,
            .size = 0,
        };
        return ret;
    }

#ifdef TEST_SEQ_LOCK
    if (level > seqLockMaxLevel) {
        seqLockMaxLevel = level;
    }
#endif

    // Require a minimum amount of data, to optimize segment usage (Ethernet frames)
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

    // Get a pointer to the entry in the queue
    first_offset = (uint32_t)(tail % queue->h.queue_size);
    first_entry = (tXcpDtoMessage *)(queue->buffer + first_offset);

    // Check the entry commit state

    uint32_t ctr_dlc = atomic_load_explicit(&first_entry->ctr_dlc, memory_order_acquire);
    uint16_t dlc = ctr_dlc & 0xFFFF;          // Transport layer packet data length
    uint16_t ctr = (uint16_t)(ctr_dlc >> 16); // Transport layer counter
    if (ctr != CTR_COMMITTED) {

        // This should never happen
        // An entry is consistent, if it is neither in reserved or committed state
        if (ctr != CTR_RESERVED) {
            DBG_PRINTF_ERROR("QueuePeek initial: inconsistent reserved - h=%" PRIu64 ", t=%" PRIu64 ", level=%u, entry: (dlc=0x%04X, ctr=0x%04X)\n", head, tail, level, dlc, ctr);
            assert(false); // Fatal error, inconsistent state
        }

        // Nothing to read, the first entry is still in reserved state
        tQueueBuffer ret = {
            .buffer = NULL,
            .size = 0,
        };
        return ret;
    }

    // This should never fail
    // An committed entry must have a valid length and an XCP ODT in it
    if (!((ctr == CTR_COMMITTED) && (dlc > 0) && (dlc <= XCPTL_MAX_DTO_SIZE) && (first_entry->data[1] == 0xAA || first_entry->data[0] >= 0xFC))) {
        DBG_PRINTF_ERROR("QueuePeek initial: inconsistent commit - h=%" PRIu64 ", t=%" PRIu64 ", level=%u, entry: (dlc=0x%04X, ctr=0x%04X, res=0x%02X)\n", head, tail, level, dlc,
                         ctr, first_entry->data[1]);
        assert(false); // Fatal error, corrupt committed state
        tQueueBuffer ret = {
            .buffer = NULL,
            .size = 0,
        };
        return ret;
    }

    // Set and increment the transport layer packet counter
    // The packet counter is obtained from the XCP transport layer
    ctr_dlc = ((uint32_t)XcpTlGetCtr() << 16) | dlc;
    atomic_store_explicit(&first_entry->ctr_dlc, ctr_dlc, memory_order_release);

    // First entry is ok now
    total_len = dlc + XCPTL_TRANSPORT_LAYER_HEADER_SIZE; // Include the transport layer header size

// Check for more packets to concatenate in a message segment with maximum of XCPTL_MAX_SEGMENT_SIZE, by repeating this procedure
// @@@@ TODO maybe optimize the duplicate code below
#ifdef QUEUE_ACCUMULATE_PACKETS
    uint32_t offset = first_offset + total_len;
    uint32_t max_offset = first_offset + level - 1;
    if (max_offset >= queue->h.queue_size) {
        max_offset = queue->h.queue_size - 1; // Don't read over wrap around
        DBG_PRINTF5("%u-%u: QueuePeek: max_offset wrapped around, head=%" PRIu64 ", tail=%" PRIu64 ", level=%u, queue_size=%u\n", first_offset, max_offset, head, tail, level,
                    queue->h.queue_size);
    }

    for (;;) {
        // Check if there is another entry in the queue to accumulate
        // It is safe to read until max_offset calculated from the consistent head
        // Just stop on wrap around
        if (offset > max_offset) {
            break; // Nothing more safe to read in queue
        }

        tXcpDtoMessage *entry = (tXcpDtoMessage *)(queue->buffer + offset);

        // Check the entry commit state
        uint32_t ctr_dlc = atomic_load_explicit(&entry->ctr_dlc, memory_order_acquire);
        uint16_t dlc = ctr_dlc & 0xFFFF;          // Transport layer packet data length
        uint16_t ctr = (uint16_t)(ctr_dlc >> 16); // Transport layer counter
        if (ctr != CTR_COMMITTED) {

            if (ctr != CTR_RESERVED) {
                DBG_PRINTF_ERROR("QueuePeek: inconsistent reserved - h=%" PRIu64 ", t=%" PRIu64 ", level=%u, entry: (dlc=0x%04X, ctr=0x%04X)\n", head, tail, level, dlc, ctr);
                assert(false);
            }

            // Nothing more to concat, the entry is still in reserved state
            break;
        }

        // Check consistency, this should never fail
        if (!((ctr == CTR_COMMITTED) && (dlc > 0) && (dlc <= XCPTL_MAX_DTO_SIZE) && (entry->data[1] == 0xAA || entry->data[0] >= 0xFC))) {
            DBG_PRINTF_ERROR("QueuePeek: inconsistent commit - h=%" PRIu64 ", t=%" PRIu64 ", level=%u, entry: (dlc=0x%04X, ctr=0x%04X, res=0x%02X)\n", head, tail, level, dlc, ctr,
                             entry->data[1]);
            assert(false); // Fatal error, corrupt committed state
            break;
        }

        uint16_t len = dlc + XCPTL_TRANSPORT_LAYER_HEADER_SIZE;

        // Check if this entry fits into the segment
        if (total_len + len > XCPTL_MAX_SEGMENT_SIZE) {
            break; // Max segment size reached
        }

        // Add this entry
        total_len += len;
        offset += len;

        ctr_dlc = ((uint32_t)XcpTlGetCtr() << 16) | dlc;
        atomic_store_explicit(&entry->ctr_dlc, ctr_dlc, memory_order_release);

    } // for(;;)
#endif // QUEUE_ACCUMULATE_PACKETS

    assert(total_len > 0 && total_len <= XCPTL_MAX_SEGMENT_SIZE);
    tQueueBuffer ret = {
        .buffer = (uint8_t *)first_entry,
        .size = total_len,
    };
    return ret;
}

// Advance the transmit queue tail by the message length obtained from the last QueuePeek call
// Segments obtained from QueuePeek must be released immediately with this function
void QueueRelease(tQueueHandle queueHandle, tQueueBuffer *const queueBuffer) {
    tQueue *queue = (tQueue *)queueHandle;
    assert(queue != NULL);
    assert(queueBuffer->size > 0 && queueBuffer->size <= XCPTL_MAX_SEGMENT_SIZE);
    atomic_fetch_add_explicit(&queue->h.tail, queueBuffer->size, memory_order_relaxed);
}

#endif