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

#include "main.h"
#include "platform.h"
#include "dbg_print.h"
#include "xcpLite.h"   

// Experimental
// Use spinlock/mutex instead of mutex for producer lock
// This naiv approach is usually faster compared to a mutex, but can produce higher worst case latencies and hard to predict impact on other threads
// It might be a better solution for non preemptive tasks
//#define USE_SPINLOCK
//#define USE_YIELD
//#define TEST_LOCK_TIMING

/*
Test results from test_multi_thread with 32 tasks and 200us sleep time 

Mutex:
lockCount=742121, maxLockTime=113000ns,  avgLockTime=1332ns
0us: 707175
10us: 18274
20us: 10192
30us: 4295
40us: 1471
50us: 451
60us: 152
70us: 63
80us: 25
90us: 16
100us: 4
110us: 3

Spinlock
lockCount=741715, maxLockTime=10058000ns,  avgLockTime=343ns
0us: 739766
10us: 1633
20us: 249
30us: 37
40us: 12
50us: 3
60us: 3
>: 12

Spinlock+Yield
lockCount=746574, maxLockTime=241000ns,  avgLockTime=517ns
0us: 734499
10us: 6553
20us: 3037
30us: 1561
40us: 398
50us: 153
60us: 61
70us: 105
80us: 128
90us: 41
100us: 29
110us: 7
140us: 1
>: 1

%32
lockCount=742068, maxLockTime=99000ns,  avgLockTime=549ns
0us: 728545
10us: 7615
20us: 3728
30us: 1596
40us: 394
50us: 123
60us: 45
70us: 13
80us: 6
90us: 3

%64
lockCount=741891, maxLockTime=215000ns,  avgLockTime=488ns
0us: 730379
10us: 6460
20us: 3108
30us: 1444
40us: 359
50us: 100
60us: 30
70us: 8
80us: 1
180us: 1
>: 1

%64 release
lockCount=741987, maxLockTime=171000ns,  avgLockTime=488ns
0us: 730705
10us: 6154
20us: 2969
30us: 1596
40us: 396
50us: 112
60us: 34
70us: 13
80us: 6
140us: 1
170us: 1


*/


#ifdef TEST_LOCK_TIMING
static uint64_t lockTimeMax = 0;
static uint64_t lockTimeSum = 0;
static uint64_t lockCount = 0;
#define HISTOGRAM_SIZE 20 // 200us in 10us steps
#define HISTOGRAM_STEP 10
static uint64_t lockTimeHistogram[HISTOGRAM_SIZE] = {0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0};
#endif

#ifndef _WIN

#include <stdatomic.h>

#else

#ifdef _WIN32_
#error "Windows32 not implemented yet"
#else

// On Windows 64 we rely on the x86-64 strong memory model and assume atomic 64 bit load/store
// and a mutex for thread safety when incrementing the tail
#undef USE_SPINLOCK
#define atomic_uint_fast64_t uint64_t
#define atomic_store_explicit(a,b,c) (*(a))=(b)
#define atomic_load_explicit(a,b) (*(a))
#define atomic_fetch_add_explicit(a,b,c) { mutexLock(&gXcpTlQueue.mutex); (*(a))+=(b); mutexUnlock(&gXcpTlQueue.mutex);}

#endif

#endif


// Queue entry states
#define RESERVED  0 // Reserved by producer
#define COMMITTED 1 // Committed by producer


// Buffer size is one entry larger than the queue size, message data is never wraped around for zero copy
#define MPSC_BUFFER_SIZE ((XCPTL_QUEUE_SIZE+1)*(XCPTL_MAX_DTO_SIZE+XCPTL_TRANSPORT_LAYER_HEADER_SIZE))  
#define MPSC_QUEUE_SIZE ((XCPTL_QUEUE_SIZE)*(XCPTL_MAX_DTO_SIZE+XCPTL_TRANSPORT_LAYER_HEADER_SIZE))  


static struct {
    char buffer[MPSC_BUFFER_SIZE];   // Preallocated buffer
    atomic_uint_fast64_t head;  // Consumer reads from head
    atomic_uint_fast64_t tail;  // Producers write to tail
    uint16_t tail_len;  // Length of the next message in the queue (determined by peek)
    uint16_t ctr;   // Next DTO data transmit message packet counter
    uint16_t overruns; // Overrun counter
    BOOL flush;     // There is a packet in the queue which has priority
    MUTEX mutex;    // Mutex for queue producers
} gXcpTlQueue;

#ifdef USE_SPINLOCK
static atomic_flag lock = ATOMIC_FLAG_INIT;
#endif

void XcpTlInitTransmitQueue() {

    DBG_PRINT3("\nInit XCP transport layer queue\n");
    DBG_PRINTF3("  SEGMENT_SIZE=%u, QUEUE_SIZE=%u, ALIGNMENT=%u, %uKiB queue memory used\n", XCPTL_MAX_SEGMENT_SIZE, XCPTL_QUEUE_SIZE, XCPTL_PACKET_ALIGNMENT, (unsigned int)sizeof(gXcpTlQueue) / 1024);
    gXcpTlQueue.overruns = 0;
    gXcpTlQueue.ctr = 0;
    gXcpTlQueue.flush = FALSE;
    mutexInit(&gXcpTlQueue.mutex, FALSE, 1000);
    atomic_store_explicit(&gXcpTlQueue.head, 0, memory_order_relaxed);
    atomic_store_explicit(&gXcpTlQueue.tail, 0, memory_order_relaxed);
    gXcpTlQueue.tail_len = 0;
#ifdef USE_SPINLOCK
    assert(atomic_is_lock_free(&lock)!=0);
    assert(atomic_is_lock_free(&gXcpTlQueue.head)!=0);
#endif
}

void XcpTlResetTransmitQueue() {
    gXcpTlQueue.tail_len = 0;
    gXcpTlQueue.overruns = 0;
    atomic_store_explicit(&gXcpTlQueue.head, 0, memory_order_relaxed);
    atomic_store_explicit(&gXcpTlQueue.tail, 0, memory_order_relaxed);
}

void XcpTlFreeTransmitQueue() {
    XcpTlResetTransmitQueue();
    mutexDestroy(&gXcpTlQueue.mutex);
    
#ifdef TEST_LOCK_TIMING
    printf("XcpTlFreeTransmitQueue: overruns=%u, lockCount=%llu, maxLockTime=%lluns,  avgLockTime=%lluns\n", gXcpTlQueue.overruns, lockCount, lockTimeMax, lockTimeSum/lockCount);
    for (int i=0; i<HISTOGRAM_SIZE-1; i++) {
        if (lockTimeHistogram[i]) printf("%dus: %llu\n", i*10, lockTimeHistogram[i]);
    }
    if (lockTimeHistogram[HISTOGRAM_SIZE-1]) printf(">: %llu\n", lockTimeHistogram[HISTOGRAM_SIZE-1]);
#endif
}


//-------------------------------------------------------------------------------------------------------------------------------------------------------
// Producer functions
// For multiple producers !!

// Get a buffer for a message with size
uint8_t* XcpTlGetTransmitBuffer(void** handle, uint16_t packet_len) {

    tXcpDtoMessage *entry = NULL;

    // Align the message length
    uint16_t msg_len = packet_len + XCPTL_TRANSPORT_LAYER_HEADER_SIZE;
#if XCPTL_PACKET_ALIGNMENT==2
    msg_len = (uint16_t)((msg_len + 1) & 0xFFFE); // Add fill %2
#endif
#if XCPTL_PACKET_ALIGNMENT==4
    msg_len = (uint16_t)((msg_len + 3) & 0xFFFC); // Add fill %4
#endif
#if XCPTL_PACKET_ALIGNMENT==8
    msg_len = (uint16_t)((msg_len + 7) & 0xFFF8); // Add fill %8
#endif

    DBG_PRINTF5("XcpTlGetTransmitBuffer: len=%d\n", packet_len);

#ifdef TEST_LOCK_TIMING
    uint64_t c = clockGet();
#endif

    // Producer lock
#ifdef USE_SPINLOCK
    #ifdef USE_YIELD
    uint32_t n = 1;
    #endif
    for (;;) {
        BOOL locked = atomic_load_explicit(&lock._Value, memory_order_relaxed);
        if (!locked  && !atomic_flag_test_and_set_explicit(&lock, memory_order_acquire)) break;
    #ifdef USE_YIELD
        if (++n%64==0) yield_thread();
    #endif
    }
#else
    mutexLock(&gXcpTlQueue.mutex);
#endif

    uint64_t head = atomic_load_explicit(&gXcpTlQueue.head,memory_order_relaxed);
    uint64_t tail = atomic_load_explicit(&gXcpTlQueue.tail,memory_order_relaxed);
    if (MPSC_QUEUE_SIZE - (uint32_t)(head-tail) >= msg_len) {

        // Prepare a new entry
        // Use the ctr as commmit state
        uint32_t offset = head % MPSC_QUEUE_SIZE;
        entry = (tXcpDtoMessage *)(gXcpTlQueue.buffer + offset);
        entry->ctr = RESERVED;  

        atomic_store_explicit(&gXcpTlQueue.head, head+msg_len,memory_order_relaxed);
    }

#ifdef USE_SPINLOCK
    atomic_flag_clear_explicit(&lock, memory_order_release);
#else
    mutexUnlock(&gXcpTlQueue.mutex);
#endif

#ifdef TEST_LOCK_TIMING
    uint64_t d = clockGet() - c;
    mutexLock(&gXcpTlQueue.mutex);
    if (d>lockTimeMax) lockTimeMax = d;
    int i = (d/1000)/10;
    if (i<HISTOGRAM_SIZE) lockTimeHistogram[i]++; else lockTimeHistogram[HISTOGRAM_SIZE-1]++;
    lockTimeSum += d;
    lockCount++;
    mutexUnlock(&gXcpTlQueue.mutex);
#endif

    if (entry==NULL) {
        gXcpTlQueue.overruns++;
        return NULL;
    }

    entry->dlc = msg_len-XCPTL_TRANSPORT_LAYER_HEADER_SIZE;
    *handle = entry;
    assert((((uint64_t)entry)&0x3)==0); // Check alignment
    return entry->data;
}

// Commit a buffer (by handle returned from XcpTlGetTransmitBuffer)
void XcpTlCommitTransmitBuffer(void* handle, BOOL flush) {

    tXcpDtoMessage *entry = (tXcpDtoMessage *)handle;
    if (flush) gXcpTlQueue.flush = TRUE;
    entry->ctr = COMMITTED;

#if defined(_WIN) // Windows has event driven transmit queue handler, Linux uses transmit queue polling 
    XcpTlNotifyTransmitQueueHandler(); 
#endif
    
    DBG_PRINTF5("XcpTlCommitTransmitBuffer: dlc=%d, pid=%u, flush=%u, overruns=%u\n", entry->dlc,entry->data[0], flush, gXcpTlQueue.overruns);
}

// Empy the queue, even if a message is not completely used
void XcpTlFlushTransmitBuffer() {
   gXcpTlQueue.flush = TRUE; 
}



//-------------------------------------------------------------------------------------------------------------------------------------------------------
// Consumer functions
// Single consumer thread !!!!!!!!!!
// The consumer is lock free against the providers, it does not contend for the mutex or spinlock used by the providers


// Get transmit queue level in bytes
// This function is thread safe, any thread can ask for the queue level
static uint32_t XcpTlGetTransmitQueueLevel() {
    uint64_t head = atomic_load_explicit(&gXcpTlQueue.head,memory_order_relaxed);
    uint64_t tail = atomic_load_explicit(&gXcpTlQueue.tail,memory_order_relaxed);
    return (uint32_t)(head-tail);
}

// Wait (sleep) until transmit queue is empty 
// This function is thread safe, any thread can wait for transmit queue empty
void XcpTlWaitForTransmitQueueEmpty() {
    uint16_t timeout = 0;
    do {
        sleepMs(20);
        timeout++;
    } while (XcpTlGetTransmitQueueLevel()!=0 && timeout<=50); // Wait max 1s until the transmit queue is empty

}

// Check if the queu has enough packets to consider transmitting a message
BOOL XcpTlTransmitQueueHasMsg() {

    uint32_t n = XcpTlGetTransmitQueueLevel();
    if (n==0) return FALSE;

    DBG_PRINTF5("XcpTlTransmitHasMsg: level=%u, flush=%u\n", n, gXcpTlQueue.flush);

    if (gXcpTlQueue.flush) {
        return TRUE; // High priority data in the queue
    }
    if (n > ((XCPTL_MAX_SEGMENT_SIZE*100)/80)) return TRUE; // Enough data if queue level is 80% of a message
    return FALSE;
}

// Check if there is message segment in the transmit queue with at least one committed packet
// Return the message length and a pointer to the message
const uint8_t * XcpTlTransmitQueuePeekMsg( uint16_t* msg_len ) {

    uint64_t head = atomic_load_explicit(&gXcpTlQueue.head,memory_order_relaxed);
    uint64_t tail = atomic_load_explicit(&gXcpTlQueue.tail,memory_order_relaxed);
    if (head == tail) return NULL;  // Queue is empty
    assert(head-tail<=MPSC_QUEUE_SIZE); // Overrun not handled
    uint32_t level = (uint32_t)(head-tail);
    DBG_PRINTF5("XcpTlTransmitQueuePeekMsg: level=%u, ctr=%u\n", level, gXcpTlQueue.ctr );

    uint32_t tail_offset = tail % MPSC_QUEUE_SIZE;  
    tXcpDtoMessage *entry1 = (tXcpDtoMessage *)(gXcpTlQueue.buffer + tail_offset);

    if (gXcpTlQueue.tail_len==0) {

        uint16_t ctr1 = entry1->ctr; // entry ctr may be concurrently changed by producer, when committed
        if (ctr1==RESERVED) return NULL;  // Not commited yet
        assert(ctr1==COMMITTED); 
        assert(entry1->dlc<=XCPTL_MAX_DTO_SIZE); // Max DTO size
        
        if (gXcpTlQueue.overruns) { // Add the number of overruns
            DBG_PRINTF4("XcpTlTransmitQueuePeekMsg: overruns=%u\n", gXcpTlQueue.overruns);
            gXcpTlQueue.ctr += gXcpTlQueue.overruns;
            gXcpTlQueue.overruns = 0;
        } 

        entry1->ctr = gXcpTlQueue.ctr++; // Set the transport layer packet counter 
        uint16_t len = entry1->dlc + XCPTL_TRANSPORT_LAYER_HEADER_SIZE;

        // Check for more packets to concatenate in a meassage segment
        uint16_t len1 = len;
        for (;;) {
            if (len==level) break; // Nothing more in queue
            assert(len<level);
            tail_offset += len1;
            if (tail_offset>=MPSC_QUEUE_SIZE) break; // Stop, can not wrap around without copying data

            tXcpDtoMessage *entry = (tXcpDtoMessage *)(gXcpTlQueue.buffer + tail_offset); 
            uint16_t ctr = entry->ctr;
            if (ctr==RESERVED) break;
            assert(ctr==COMMITTED);

            // Add this entry
            assert(entry->dlc<=XCPTL_MAX_DTO_SIZE); // Max DTO size
            len1 = entry->dlc + XCPTL_TRANSPORT_LAYER_HEADER_SIZE; 
            if (len+len1 > XCPTL_MAX_SEGMENT_SIZE ) break; // Max segment size reached
            len += len1;
            entry->ctr = gXcpTlQueue.ctr++;
        }

        gXcpTlQueue.tail_len = len;
    }
    else {
        assert(0);  // @@@@ This may happen, but not observed ever
    }

    //DBG_PRINTF5("XcpTlTransmitQueuePeekMsg: msg_len = %u\n", gXcpTlQueue.tail_len );
    *msg_len = gXcpTlQueue.tail_len;
    return (uint8_t*)entry1;
}


// Advance the transmit queue tail by the message lentgh obtained from the last peek
void XcpTlTransmitQueueNextMsg() {
    
    DBG_PRINTF5("XcpTlTransmitQueueNext: msg_len = %u\n", gXcpTlQueue.tail_len );
    if (gXcpTlQueue.tail_len==0) return;
    atomic_fetch_add_explicit(&gXcpTlQueue.tail,gXcpTlQueue.tail_len,memory_order_relaxed);
    gXcpTlQueue.tail_len = 0;
    gXcpTlQueue.flush = FALSE;
}

