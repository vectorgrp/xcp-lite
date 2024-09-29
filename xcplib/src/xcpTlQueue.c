/*----------------------------------------------------------------------------
| File:
|   xcpTlQueue.c
|
| Description:
|   XCP transport layer queue
|   Multi producer single consumer queue (supported on 64 bit systems with atomic 64 bit operations)
|
| Copyright (c) Vector Informatik GmbH. All rights reserved.
| Licensed under the MIT license. See LICENSE file in the project root for details.
|
 ----------------------------------------------------------------------------*/

#include "main.h"
#include "platform.h"
#include "dbg_print.h"
#include "xcpLite.h"   

// Experimental
// Use spinlock/mutex instead of mutex for producer lock
// This naiv approach is usually not faster compared to a mutex and can produce higher latencies and hard to predict impact on other threads
// It might be a better solution for non preemptive tasks
//#define USE_SPINLOCK
//#define USE_YIELD
//#define TEST_LOCK_TIMING

/* 
Test results from test_multi_thread with 32 tasks and 200us sleep time:   
maxLock and avgLock time in ns

SPINLOCK+YIELD
    lockCount=501170, maxLock=296000, avgLock=768
    lockCount=501019, maxLock=195000, avgLock=744
    lockCount=500966, maxLock=210000, avgLock=724

SPINLOCK without cache friendly lock check
    lockCount=492952, maxLock=10115000, avgLock=1541

SPINLOCK
    lockCount=497254, maxLock=9935000, avgLock=512
    lockCount=494866, maxLock=11935000, avgLock=1322
    lockCount=490923, maxLock=10019000, avgLock=2073
    lockCount=489831, maxLock=10024000, avgLock=1980

MUTEX
    lockCount=499798, maxLock=114000, avgLock=840
    lockCount=500202, maxLock=135000, avgLock=806
    lockCount=499972, maxLock=130000, avgLock=790
    lockCount=500703, maxLock=124000, avgLock=755
    lockCount=500773, maxLock=126000, avgLock=669
*/

#ifdef TEST_LOCK_TIMING
static uint64_t lockTimeMax = 0;
static uint64_t lockTimeSum = 0;
static uint64_t lockCount = 0;
#endif

#ifndef _WIN

#include <stdatomic.h>

#else

#ifdef _WIN32_
#error "Windows32 not implemented yet"
#else

#undef USE_SPINLOCK
#define atomic_uint_fast64_t uint64_t
#define atomic_store(a,b) (*a)=(b)
#define atomic_load(a) (*a)
#define atomic_load_explicit(a,b) (*a)
#define atomic_fetch_add(a,b) { mutexLock(&gXcpTlQueue.mutex); (*a)+=(b); mutexUnlock(&gXcpTlQueue.mutex);}

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
#ifndef USE_SPINLOCK
    MUTEX mutex;    // Mutex for queue producers
#endif
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
#ifndef USE_SPINLOCK
    mutexInit(&gXcpTlQueue.mutex, FALSE, 1000);
#endif
    atomic_store(&gXcpTlQueue.head, 0);
    atomic_store(&gXcpTlQueue.tail, 0);
    gXcpTlQueue.tail_len = 0;
#ifdef USE_SPINLOCK
    assert(atomic_is_lock_free(&lock)!=0);
    assert(atomic_is_lock_free(&gXcpTlQueue.head)!=0);
#endif
}

void XcpTlResetTransmitQueue() {
    gXcpTlQueue.tail_len = 0;
    gXcpTlQueue.overruns = 0;
    atomic_store(&gXcpTlQueue.head, 0);
    atomic_store(&gXcpTlQueue.tail, 0);
}

void XcpTlFreeTransmitQueue() {
    XcpTlResetTransmitQueue();
#ifndef USE_SPINLOCK
    mutexDestroy(&gXcpTlQueue.mutex);
#endif
    
#ifdef TEST_LOCK_TIMING
    DBG_PRINTF3("XcpTlFreeTransmitQueue: overruns=%u, lockCount=%llu, maxLock=%llu, avgLock=%llu\n", gXcpTlQueue.overruns, lockCount, lockTimeMax, lockTimeSum/lockCount);
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

    // Producer lock
#ifdef TEST_LOCK_TIMING
    uint64_t c = clockGet();
#endif
#ifdef USE_SPINLOCK
    for (uint32_t n = 1;1;n++) {
        BOOL locked = atomic_load_explicit(&lock._Value, memory_order_relaxed);
        if (!locked  && !atomic_flag_test_and_set_explicit(&lock, memory_order_acquire)) break;
        //if ( !atomic_flag_test_and_set_explicit(&lock, memory_order_acquire)) break;
    #ifdef USE_YIELD
        if (n%16==0) yield_thread();
    #endif
    }
#else
    mutexLock(&gXcpTlQueue.mutex);
#endif
#ifdef TEST_LOCK_TIMING
    uint64_t d = clockGet() - c;
    if (d>lockTimeMax) lockTimeMax = d;
    lockTimeSum += d;
    lockCount++;
#endif

    uint64_t head = atomic_load(&gXcpTlQueue.head);
    uint64_t tail = atomic_load_explicit(&gXcpTlQueue.tail,memory_order_relaxed);
    if (MPSC_QUEUE_SIZE - (uint32_t)(head-tail) >= msg_len) {

        // Prepare a new entry
        // Use the ctr as commmit state
        uint32_t offset = head % MPSC_QUEUE_SIZE;
        entry = (tXcpDtoMessage *)(gXcpTlQueue.buffer + offset);
        entry->ctr = RESERVED;  

        atomic_store(&gXcpTlQueue.head, head+msg_len);
    }

#ifdef USE_SPINLOCK
    atomic_flag_clear_explicit(&lock, memory_order_release);
#else
    mutexUnlock(&gXcpTlQueue.mutex);
#endif
    if (entry==NULL) {
        gXcpTlQueue.overruns++;
        return NULL;
    }

    entry->dlc = msg_len-XCPTL_TRANSPORT_LAYER_HEADER_SIZE;
    *handle = entry;
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
// Thread safe for single consumer only !!


// Get transmit queue level in bytes
static uint32_t XcpTlGetTransmitQueueLevel() {
    uint64_t head = atomic_load(&gXcpTlQueue.head);
    uint64_t tail = atomic_load(&gXcpTlQueue.tail);
    return (uint32_t)(head-tail);
}

// Wait (sleep) until transmit queue is empty 
void XcpTlWaitForTransmitQueueEmpty() {
    uint16_t timeout = 0;
    do {
        sleepMs(20);
        timeout++;
    } while (XcpTlGetTransmitQueueLevel()!=0 && timeout<=50); // Wait max 1s until the transmit queue is empty

}


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

// Check if there is a fully commited message segment in the transmit queue
// Return the message length and a pointer to the message
const uint8_t * XcpTlTransmitQueuePeekMsg( uint16_t* msg_len ) {

    uint64_t head = atomic_load(&gXcpTlQueue.head);
    uint64_t tail = atomic_load(&gXcpTlQueue.tail);
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
            DBG_PRINTF3("XcpTlTransmitQueuePeekMsg: overruns=%u\n", gXcpTlQueue.overruns);
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
        assert(0);  // @@@@@@@ This may happen, but not observed ever
    }

    //DBG_PRINTF5("XcpTlTransmitQueuePeekMsg: msg_len = %u\n", gXcpTlQueue.tail_len );
    *msg_len = gXcpTlQueue.tail_len;
    return (uint8_t*)entry1;
}


// Advance the transmit queue tail by the message lentgh obtained from the last peek
void XcpTlTransmitQueueNextMsg() {
    
    DBG_PRINTF5("XcpTlTransmitQueueNext: msg_len = %u\n", gXcpTlQueue.tail_len );
    if (gXcpTlQueue.tail_len==0) return;
    atomic_fetch_add(&gXcpTlQueue.tail,gXcpTlQueue.tail_len);
    gXcpTlQueue.tail_len = 0;
    gXcpTlQueue.flush = FALSE;
}

