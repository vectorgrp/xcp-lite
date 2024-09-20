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

#ifdef XCPTL_ENABLE_MPSC_QUEUE

// Queue entry states
#define RESERVED 1 // Reserved for producer
#define COMMITTED 2 // Committed by producer
#define FLUSH 2 // Committed and requesting flush by producer

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
} gXcpTlQueue;


void XcpTlInitTransmitQueue() {
    gXcpTlQueue.ctr = 0;
    atomic_store(&gXcpTlQueue.head, 0);
    atomic_store(&gXcpTlQueue.tail, 0);
}

void XcpTlResetTransmitQueue() {
    atomic_store(&gXcpTlQueue.head, 0);
    atomic_store(&gXcpTlQueue.tail, 0);
}

void XcpTlFreeTransmitQueue() {
    XcpTlResetTransmitQueue();
}


//-------------------------------------------------------------------------------------------------------------------------------------------------------
// Producer functions
// For multiple producers !!

// Get a buffer for a message with size
extern uint8_t* XcpTlGetTransmitBuffer(void** handle, uint16_t packet_len) {


    // Align the message length
    uint16_t msg_len = packet_len + XCPTL_TRANSPORT_LAYER_HEADER_SIZE;
#if XCPTL_PACKET_ALIGNMENT==2
    msg_len = (uint16_t)((message_len + 1) & 0xFFFE); // Add fill %2
#endif
#if XCPTL_PACKET_ALIGNMENT==4
    msg_len = (uint16_t)((message_len + 3) & 0xFFFC); // Add fill %4
#endif
#if XCPTL_PACKET_ALIGNMENT==8
    msg_len = (uint16_t)((msg_len + 7) & 0xFFF8); // Add fill %8
#endif

    // Reserve a slot in the circular buffer
    // Don't care for overflow, receiver will handle this issue
    uint64_t head = atomic_fetch_add(&gXcpTlQueue.head, msg_len);
    uint32_t offset = head % MPSC_QUEUE_SIZE;

    DBG_PRINTF5("XcpTlGetTransmitBuffer: len=%d, offset=%u\n", packet_len,offset);

    // Prepare a new entry
    // Use the ctr as commmit state
    tXcpCtoMessage *entry = (tXcpCtoMessage *)(gXcpTlQueue.buffer + offset);
    entry->dlc= msg_len-XCPTL_TRANSPORT_LAYER_HEADER_SIZE;
    entry->ctr = RESERVED;
    *handle = entry;
    return entry->data;
}

// Commit a buffer (by handle returned from XcpTlGetTransmitBuffer)
void XcpTlCommitTransmitBuffer(void* handle, BOOL flush) {

    tXcpCtoMessage *entry = (tXcpCtoMessage *)handle;
    entry->ctr = flush ? FLUSH : COMMITTED;  // Mark as commited
    notifyTransmitQueueHandler();
    
    DBG_PRINTF5("XcpTlCommitTransmitBuffer: dlc=%d, pid=%u\n", entry->dlc,entry->data[0]);
}

// Finalize the current transmit packet
void XcpTlFlushTransmitBuffer() {
    // Not needed for MPSC queue
}

// Wait (sleep) until transmit queue is empty 
void XcpTlWaitForTransmitQueueEmpty() {
    uint16_t timeout = 0;
    do {
        sleepMs(20);
        timeout++;
    } while (XcpTlGetTransmitQueueLevel()!=0 && timeout<=50); // Wait max 1s until the transmit queue is empty

}


// Get transmit queue level
int32_t XcpTlGetTransmitQueueLevel() {
    uint64_t head = atomic_load(&gXcpTlQueue.head);
    uint64_t tail = atomic_load(&gXcpTlQueue.tail);
    return head-tail;
}



//-------------------------------------------------------------------------------------------------------------------------------------------------------
// Consumer functions
// For single consumer only !!

// Check if there is a fully commited message segment in the transmit queue
// Return the message length and a pointer to the message
const uint8_t * XcpTlTransmitQueuePeek( uint16_t* msg_len) {

    uint64_t head = atomic_load(&gXcpTlQueue.head);
    uint64_t tail = atomic_load(&gXcpTlQueue.tail);

    if (head == tail) return NULL;  // Queue is empty

    // The producers can not check for overflow, because of the individual atomic operations for head and tail
    // In case of overflow (tail-head is too large) all queue data except the uncommited messages will be deleted, because the tail has been overwritten
    // It is assumed, that the overwritten part of tail can never reached the uncommited part of the queue
    assert(head-tail < MPSC_QUEUE_SIZE); // Overrun not handled yet

    uint32_t offset = tail % MPSC_QUEUE_SIZE;  // Wrap around with modulo
    tXcpCtoMessage *entry = (tXcpCtoMessage *)(gXcpTlQueue.buffer + offset);
    if (entry->ctr==RESERVED) return NULL;  // Not commited yet
    
    // Set the transport layer packet count counter and message length
    entry->ctr = gXcpTlQueue.ctr++;

    DBG_PRINTF5("XcpTlTransmitQueuePeek: tail offset = %u, dlc=%u, ctr=%u, pid=%u\n", offset, entry->dlc,entry->ctr,entry->data[0]);

    // Check for further packets to concatenated
    // @@@@

    *msg_len = entry->dlc + XCPTL_TRANSPORT_LAYER_HEADER_SIZE;
    return (uint8_t*)entry;
}


// Remove the head transmit queue entry
void XcpTlTransmitQueueNext() {
    
    uint64_t head = atomic_load(&gXcpTlQueue.head);
    uint64_t tail = atomic_load(&gXcpTlQueue.tail);
    assert(head-tail < MPSC_QUEUE_SIZE); // Overrun not handled yet

    DBG_PRINTF5("XcpTlTransmitQueueNext: head offset=%u, tail offset=%u\n", (uint32_t)(head % MPSC_QUEUE_SIZE), (uint32_t)(tail % MPSC_QUEUE_SIZE));
    if (head == tail) return;  // Queue is empty
    uint32_t offset = tail % MPSC_QUEUE_SIZE; 
    tXcpCtoMessage *entry = (tXcpCtoMessage *)(gXcpTlQueue.buffer + offset);
    tail += entry->dlc + XCPTL_TRANSPORT_LAYER_HEADER_SIZE;
    atomic_store(&gXcpTlQueue.tail, tail );
    DBG_PRINTF5("XcpTlTransmitQueueNext: new tail offset=%u\n", (uint32_t)(tail % MPSC_QUEUE_SIZE));
}

#endif