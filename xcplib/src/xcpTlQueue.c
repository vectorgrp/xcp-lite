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

#include <stdatomic.h>


// #define USE_SPINLOCK

// Queue entry states
#define RESERVED  0 // Reserved for producer
#define COMMITTED 1 // Committed by producer


// Buffer size is one entry larger than the queue size, message data is never wraped around
#define MPSC_BUFFER_SIZE ((XCPTL_QUEUE_SIZE+1)*(XCPTL_MAX_DTO_SIZE+XCPTL_TRANSPORT_LAYER_HEADER_SIZE))  
#define MPSC_QUEUE_SIZE ((XCPTL_QUEUE_SIZE)*(XCPTL_MAX_DTO_SIZE+XCPTL_TRANSPORT_LAYER_HEADER_SIZE))  

#pragma pack(push, 1)
typedef struct {
    uint16_t dlc;    // XCP TL header lenght
    uint16_t ctr;    // XCP TL Header message counter
    uint8_t data[];                 
} tXcpCtoMessage;
#pragma pack(pop)


static struct {
    char buffer[MPSC_BUFFER_SIZE];   // Preallocated buffer
    atomic_uint_fast64_t head;  // Consumer reads from head
    atomic_uint_fast64_t tail;  // Producers write to tail
    uint16_t ctr;   // next DTO data transmit message packet counter
    BOOL flush;     // There is a packet in the queue which has priority
#ifndef USE_SPINLOCK
    MUTEX mutex;    // Mutex for queue producers
#endif
} gXcpTlQueue;

atomic_flag lock = ATOMIC_FLAG_INIT;


void XcpTlInitTransmitQueue() {
    gXcpTlQueue.ctr = 0;
    gXcpTlQueue.flush = FALSE;
    mutexInit(&gXcpTlQueue.mutex, FALSE, 1000);
    atomic_store(&gXcpTlQueue.head, 0);
    atomic_store(&gXcpTlQueue.tail, 0);
}

void XcpTlResetTransmitQueue() {
    atomic_store(&gXcpTlQueue.head, 0);
    atomic_store(&gXcpTlQueue.tail, 0);
}

void XcpTlFreeTransmitQueue() {
    XcpTlResetTransmitQueue();
    mutexDestroy(&gXcpTlQueue.mutex);
}


//-------------------------------------------------------------------------------------------------------------------------------------------------------
// Producer functions
// For multiple producers !!

// Get a buffer for a message with size
extern uint8_t* XcpTlGetTransmitBuffer(void** handle, uint16_t packet_len) {

    tXcpCtoMessage *entry = NULL;

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
#ifdef USE_SPINLOCK
    // while (!atomic_compare_exchange_weak(&gXcpTlQueue.lock, FALSE, TRUE));
    while (atomic_flag_test_and_set(&lock));
#else
    mutexLock(&gXcpTlQueue.mutex);
#endif

    uint64_t head = atomic_load(&gXcpTlQueue.head);
    uint64_t tail = atomic_load(&gXcpTlQueue.tail);
    if (MPSC_QUEUE_SIZE - (head-tail) >= msg_len) {

        atomic_store(&gXcpTlQueue.head, head+msg_len);

        // Prepare a new entry
        // Use the ctr as commmit state
        uint32_t offset = head % MPSC_QUEUE_SIZE;
        entry = (tXcpCtoMessage *)(gXcpTlQueue.buffer + offset);
        entry->ctr = RESERVED;
    }

#ifdef USE_SPINLOCK
    //atomic_store(&gXcpTlQueue.lock, FALSE);
    atomic_flag_clear(&lock);
#else
    mutexUnlock(&gXcpTlQueue.mutex);
#endif
    if (entry==NULL) return NULL;

    entry->dlc = msg_len-XCPTL_TRANSPORT_LAYER_HEADER_SIZE;
    *handle = entry;
    return entry->data;
}

// Commit a buffer (by handle returned from XcpTlGetTransmitBuffer)
void XcpTlCommitTransmitBuffer(void* handle, BOOL flush) {

    tXcpCtoMessage *entry = (tXcpCtoMessage *)handle;
    if (flush) gXcpTlQueue.flush = TRUE;
    entry->ctr = COMMITTED;
    notifyTransmitQueueHandler();
    
    DBG_PRINTF5("XcpTlCommitTransmitBuffer: dlc=%d, pid=%u, flush=%u\n", entry->dlc,entry->data[0], flush);
}

// Empy the queue, even if a message is not completely used
void XcpTlFlushTransmitBuffer() {
   gXcpTlQueue.flush = TRUE; 
}



//-------------------------------------------------------------------------------------------------------------------------------------------------------
// Consumer functions
// Thread safe for single consumer only !!


// Get transmit queue level in bytes
static int32_t XcpTlGetTransmitQueueLevel() {
    uint64_t head = atomic_load(&gXcpTlQueue.head);
    uint64_t tail = atomic_load(&gXcpTlQueue.tail);
    return head-tail;
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

    DBG_PRINTF5("XcpTlTransmitHasMsg: n=%u, flush=%u\n", n, gXcpTlQueue.flush);

    if (gXcpTlQueue.flush) {
        return TRUE; // High priority data in the queue
    }
    if (n > ((XCPTL_MAX_SEGMENT_SIZE*100)/80)) return TRUE; // Enough data if queue level is 80% of a message
    return FALSE;
}

// Check if there is a fully commited message segment in the transmit queue
// Return the message length and a pointer to the message
const uint8_t * XcpTlTransmitQueuePeekMsg( uint16_t* msg_len) {

    uint64_t head = atomic_load(&gXcpTlQueue.head);
    uint64_t tail = atomic_load(&gXcpTlQueue.tail);

    if (head == tail) return NULL;  // Queue is empty

    // The producers can not check for overflow, because of the individual atomic operations for head and tail
    // In case of overflow (tail-head is too large) all queue data except the uncommited messages will be deleted, because the tail has been overwritten
    // It is assumed, that the overwritten part of tail can never reached the uncommited part of the queue
    uint32_t size = head-tail;
    assert(size <= MPSC_QUEUE_SIZE); // Overrun not handled yet
    DBG_PRINTF5("XcpTlTransmitQueuePeekMsg: queue size = %u\n", size );

    uint32_t tail_offset = tail % MPSC_QUEUE_SIZE;  
    tXcpCtoMessage *entry0 = (tXcpCtoMessage *)(gXcpTlQueue.buffer + tail_offset);
    uint16_t ctr = entry0->ctr; // entry ctr may be concurrently changed by producer, when committed
    if (ctr==RESERVED) {
        DBG_PRINT5("XcpTlTransmitQueuePeekMsg: RESERVED\n");
        return NULL;  // Not commited yet
    }
    assert(ctr==COMMITTED); 
    entry0->ctr = gXcpTlQueue.ctr++; // Set the transport layer packet count counter 
    uint16_t len0 = entry0->dlc + XCPTL_TRANSPORT_LAYER_HEADER_SIZE;

    // Check for more packets to concatenate
    uint16_t len = len0;
    for (;;) {
        if (len0==size) break; // Queue is empty
        assert(len0<size);
        tail_offset += len;
        if (tail_offset>=MPSC_QUEUE_SIZE) break; // Can not wrap around

        tXcpCtoMessage *entry = (tXcpCtoMessage *)(gXcpTlQueue.buffer + tail_offset); 
        uint16_t ctr = entry->ctr;
        if (ctr!=COMMITTED) {
            assert(ctr==RESERVED);
            break; // Not commited yet
        }
        assert(entry->dlc<=XCPTL_MAX_DTO_SIZE); // Max DTO size
        len = entry->dlc + XCPTL_TRANSPORT_LAYER_HEADER_SIZE; 
        if (len0+len > XCPTL_MAX_SEGMENT_SIZE ) break; // Max segment size reached
        len0 += len;
        
        entry->ctr = gXcpTlQueue.ctr++;
    }
    
    DBG_PRINTF5("XcpTlTransmitQueuePeekMsg: len = %u\n", len0 );
    *msg_len = len0;
    return (uint8_t*)entry0;
}


// Advance the transmit queue tail
void XcpTlTransmitQueueNextMsg( uint16_t msg_len ) {
    
    uint64_t tail = atomic_load(&gXcpTlQueue.tail);
    tail += msg_len;
    atomic_store(&gXcpTlQueue.tail, tail );
    DBG_PRINTF5("XcpTlTransmitQueueNext: new tail offset=%u\n", (uint32_t)(tail % MPSC_QUEUE_SIZE));
    gXcpTlQueue.flush = FALSE;
}

