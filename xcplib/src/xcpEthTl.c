/*----------------------------------------------------------------------------
| File:
|   xcpEthTl.c
|
| Description:
|   XCP on UDP transport layer
|   Linux and Windows version
|   Supports Winsock and Linux Sockets
|
| Copyright (c) Vector Informatik GmbH. All rights reserved.
| Licensed under the MIT license. See LICENSE file in the project root for details.
|
 ----------------------------------------------------------------------------*/

#include "main.h"
#include "platform.h"
#include "dbg_print.h"
#include "xcpLite.h"   
#include "xcpTlQueue.h"   


#if defined(XCPTL_ENABLE_UDP) || defined(XCPTL_ENABLE_TCP)
static struct {

    SOCKET Sock;
#ifdef XCPTL_ENABLE_TCP
    SOCKET ListenSock;
#endif
#ifdef PLATFORM_ENABLE_GET_LOCAL_ADDR
    uint8_t ServerMac[6];
    uint8_t ServerAddr[4];
#endif
    uint16_t ServerPort;
    BOOL ServerUseTCP;
    BOOL blockingRx;
    uint8_t MasterAddr[4];
    uint16_t MasterPort;
    BOOL MasterAddrValid;

    // Multicast
#ifdef XCPTL_ENABLE_MULTICAST
    THREAD MulticastThreadHandle;
    SOCKET MulticastSock;
#endif

} gXcpTl;
#endif


#if defined(XCPTL_ENABLE_TCP) && defined(XCPTL_ENABLE_UDP)
#define isTCP() (gXcpTl.ListenSock != INVALID_SOCKET)
#else
#ifdef XCPTL_ENABLE_TCP
#define isTCP() TRUE
#else
#define isTCP() FALSE
#endif
#endif


#ifdef XCPTL_ENABLE_MULTICAST
static int handleXcpMulticastCommand(int n, tXcpCtoMessage* p, uint8_t* dstAddr, uint16_t dstPort);
#endif

// #define XCPTL_OK                   0
// #define XCPTL_ERROR_WOULD_BLOCK    1
// #define XCPTL_ERROR_SEND_FAILED    2
// #define XCPTL_ERROR_INVALID_MASTER 3



//------------------------------------------------------------------------------
// Ethernet transport layer socket functions

#if defined(XCPTL_ENABLE_UDP) || defined(XCPTL_ENABLE_TCP)

// Transmit a UDP datagramm or TCP segment (contains multiple XCP DTO messages or a single CRM message (len+ctr+packet+fill))
// Must be thread safe, because it is called from CMD and from DAQ thread
// Returns -1 on would block, 1 if ok, 0 on error
int XcpEthTlSend(const uint8_t *data, uint16_t size, const uint8_t* addr, uint16_t port) {

    int r;

    assert(size<=XCPTL_MAX_SEGMENT_SIZE); // Check for buffer overflow
    
    DBG_PRINTF5("XcpEthTlSend: msg_len = %u\n", size );

#ifdef XCPTL_ENABLE_TCP
    if (isTCP()) {
      r = socketSend(gXcpTl.Sock, data, size);
    }
    else
#endif

#ifdef XCPTL_ENABLE_UDP
    {
      if (addr != NULL) { // Respond to given addr and port (used for multicast)
        r = socketSendTo(gXcpTl.Sock, data, size, addr, port, NULL);
      }
      else { // Respond to active master
        if (!gXcpTl.MasterAddrValid) {
          DBG_PRINT_ERROR("ERROR: invalid master address!\n");
          //gXcpTl.lastError = XCPTL_ERROR_INVALID_MASTER;
          return 0;
        }
        r = socketSendTo(gXcpTl.Sock, data, size, gXcpTl.MasterAddr, gXcpTl.MasterPort, NULL);
      }
    }
#endif // UDP

    if (r != size) {
        if (socketGetLastError()==SOCKET_ERROR_WBLOCK) {
            //gXcpTl.lastError = XCPTL_ERROR_WOULD_BLOCK;
            return -1; // Would block
        }
        else {
            DBG_PRINTF_ERROR("ERROR: XcpEthTlSend: send failed (result=%d, errno=%d)!\n", r, socketGetLastError());
            //gXcpTl.lastError = XCPTL_ERROR_SEND_FAILED;
            return 0; // Error
        }
    }

    return 1; // Ok
}



//------------------------------------------------------------------------------


// Transmit XCP multicast response
#ifdef XCPTL_ENABLE_MULTICAST
void XcpEthTlSendMulticastCrm(const uint8_t* packet, uint16_t packet_size, const uint8_t* addr, uint16_t port) {

  int r;

  // Build XCP CTO message (ctr+dlc+packet)
  tXcpCtoMessage p;
  p.dlc = (uint16_t)packet_size;
  p.ctr = 0;
  memcpy(p.packet, packet, packet_size);
  r = XcpEthTlSend((uint8_t*)&p, (uint16_t)(packet_size + XCPTL_TRANSPORT_LAYER_HEADER_SIZE),addr,port);
  if (r == (-1)) { // Would block
      // @@@@ ToDo: Handle this case
  }
}
#endif


//------------------------------------------------------------------------------

static int handleXcpCommand(tXcpCtoMessage *p, uint8_t *srcAddr, uint16_t srcPort) {

    int connected;

    // gXcpTl.LastCrmCtr = p->ctr;
    connected = XcpIsConnected();

#ifdef DBG_LEVEL
    if (DBG_LEVEL >= 5) {
        DBG_PRINTF5("RX: CTR %04X LEN %04X DATA = ", p->ctr,p->dlc);
        for (int i = 0; i < p->dlc; i++) printf("%0X ", p->packet[i]);
        printf("\n");
    }
#endif

    /* Connected */
    if (connected) {

#ifdef XCPTL_ENABLE_UDP
        if (!isTCP() && gXcpTl.MasterAddrValid) {

            // Check unicast ip address, not allowed to change
            if (memcmp(&gXcpTl.MasterAddr, srcAddr, sizeof(gXcpTl.MasterAddr)) != 0) { // Message from different master received
                DBG_PRINTF_WARNING("WARNING: message from unknown new master %u.%u.%u.%u, disconnecting!\n", srcAddr[0], srcAddr[1], srcAddr[2], srcAddr[3]);
                XcpDisconnect();
                gXcpTl.MasterAddrValid = FALSE;
                return 1; // Disconnect
            }

            // Check unicast master udp port, not allowed to change
            if (gXcpTl.MasterPort != srcPort) {
                DBG_PRINTF_WARNING("WARNING: master port changed from %u to %u, disconnecting!\n", gXcpTl.MasterPort, srcPort);
                XcpDisconnect();
                gXcpTl.MasterAddrValid = FALSE;
                return 1; // Disconnect
            }
        }
#endif // UDP
        if (p->dlc>XCPTL_MAX_CTO_SIZE) return 0;
        XcpCommand((const uint32_t*)&p->packet[0], (uint8_t)p->dlc); // Handle command
    }

    /* Not connected yet */
    else {
        /* Check for CONNECT command ? */
        if (p->dlc == 2 && p->packet[0] == CC_CONNECT) {
#ifdef XCPTL_ENABLE_UDP
            if (!isTCP()) {
                memcpy(gXcpTl.MasterAddr, srcAddr, sizeof(gXcpTl.MasterAddr)); // Save master address, so XcpCommand can send the CONNECT response
                gXcpTl.MasterPort = srcPort;
                gXcpTl.MasterAddrValid = TRUE;
            }
#endif // UDP
            XcpTlResetTransmitQueue();
            XcpCommand((const uint32_t*)&p->packet[0],(uint8_t)p->dlc); // Handle CONNECT command
        }
        else {
            DBG_PRINT_WARNING("WARNING: handleXcpCommand: no valid CONNECT command\n");
        }

    }

#ifdef XCPTL_ENABLE_UDP
    if (!isTCP() && !connected) { // not connected before
        if (XcpIsConnected()) {
            DBG_PRINTF3("XCP master connected on UDP addr=%u.%u.%u.%u, port=%u\n", gXcpTl.MasterAddr[0], gXcpTl.MasterAddr[1], gXcpTl.MasterAddr[2], gXcpTl.MasterAddr[3], gXcpTl.MasterPort);
        }
        else { // Is not in connected state
            gXcpTl.MasterAddrValid = FALSE; // Any client can connect
        }
    } // not connected before
#endif // UDP

    return 1; // Ok
}


// Handle incoming XCP commands
// Blocking for timeout_ms, currently XCPTL_TIMEOUT_INFINITE only (blocking)
// returns FALSE on error
BOOL XcpEthTlHandleCommands(uint32_t timeout_ms) {

    tXcpCtoMessage msgBuf;
    int16_t n;

    // Timeout not used
    // Behaviour depends on socket mode (blocking or non blocking)
    (void)timeout_ms;
    assert((!gXcpTl.blockingRx && timeout_ms==0) || (gXcpTl.blockingRx && timeout_ms==XCPTL_TIMEOUT_INFINITE));

#ifdef XCPTL_ENABLE_TCP
    if (isTCP()) {

        // Listen to incoming TCP connection if not connected
        if (gXcpTl.Sock == INVALID_SOCKET) {
            DBG_PRINT5("Waiting for TCP connection ...\n");
            gXcpTl.Sock = socketAccept(gXcpTl.ListenSock, gXcpTl.MasterAddr); // Wait here for incoming connection
            if (gXcpTl.Sock == INVALID_SOCKET) {
                DBG_PRINT_ERROR("ERROR: accept failed!\n");
                return TRUE; // Ignore error from accept, when in non blocking mode
            }
            else {
                DBG_PRINTF3("XCP master %u.%u.%u.%u accepted!\n", gXcpTl.MasterAddr[0], gXcpTl.MasterAddr[1], gXcpTl.MasterAddr[2], gXcpTl.MasterAddr[3]);
                DBG_PRINT3("Listening for XCP commands\n");
            }
        }

        // Receive TCP transport layer message
        n = socketRecv(gXcpTl.Sock, (uint8_t*)&msgBuf.dlc, (uint16_t)XCPTL_TRANSPORT_LAYER_HEADER_SIZE, TRUE); // header, recv blocking
        if (n == XCPTL_TRANSPORT_LAYER_HEADER_SIZE) {
            n = socketRecv(gXcpTl.Sock, (uint8_t*)&msgBuf.packet, msgBuf.dlc, TRUE); // packet, recv blocking
            if (n > 0) {
                if (n == msgBuf.dlc) {
                    return handleXcpCommand(&msgBuf, NULL, 0);
                }
                else {
                    socketShutdown(gXcpTl.Sock); // Let the receive thread terminate without error message
                    return FALSE;  // Should not happen
                }
            }
        }
        if (n==0) {  // Socket closed
            DBG_PRINT3("XCP Master closed TCP connection! XCP disconnected.\n");
            XcpDisconnect();
            sleepMs(100);
            socketShutdown(gXcpTl.Sock); // Let the receive thread terminate without error message
            socketClose(&gXcpTl.Sock);
            return TRUE; // Ok, TCP socket closed
        }
    }
#endif // TCP

#ifdef XCPTL_ENABLE_UDP
    if (!isTCP()) {
        uint16_t srcPort;
        uint8_t srcAddr[4];
        n = socketRecvFrom(gXcpTl.Sock, (uint8_t*)&msgBuf, (uint16_t)sizeof(msgBuf), srcAddr, &srcPort, NULL); 
        if (n == 0) return TRUE; // Socket closed, should not happen
        if (n < 0) {  // error
            if (socketGetLastError() == SOCKET_ERROR_WBLOCK) return 1; // Ok, timeout, no command pending
            DBG_PRINTF_ERROR("ERROR %u: recvfrom failed (result=%d)!\n", socketGetLastError(), n);
            return FALSE; // Error
        }
        else { // Ok
            if (msgBuf.dlc != n - XCPTL_TRANSPORT_LAYER_HEADER_SIZE) {
              DBG_PRINT_ERROR("ERROR: corrupt message received!\n");
              return FALSE; // Error
            }
            return handleXcpCommand(&msgBuf, srcAddr, srcPort);
        }
    }
#endif // UDP

    return FALSE;
}


//-------------------------------------------------------------------------------------------------------
// XCP Multicast

#ifdef XCPTL_ENABLE_MULTICAST

static int handleXcpMulticastCommand(int n, tXcpCtoMessage* p, uint8_t* dstAddr, uint16_t dstPort) {

    (void)dstAddr;
    (void)dstPort;

    // @@@@ ToDo: Check addr and cluster id and port
    //printf("MULTICAST: %u.%u.%u.%u:%u len=%u\n", dstAddr[0], dstAddr[1], dstAddr[2], dstAddr[3], dstPort, n);

    // Valid socket data received, at least transport layer header and 1 byte
    if (n >= XCPTL_TRANSPORT_LAYER_HEADER_SIZE + 1 && p->dlc <= n- XCPTL_TRANSPORT_LAYER_HEADER_SIZE) {
        if (p->dlc >= XCPTL_MAX_CTO_SIZE) return 0; // Error
        XcpCommand((const uint32_t*)&p->packet[0],(uint8_t)p->dlc); // Handle command
    }
    else {
      printf("MULTICAST ignored\n");

    }
    return 1; // Ok
}

void XcpEthTlSetClusterId(uint16_t clusterId) {
  (void)clusterId;
  // Not implemented
}


#if defined(_WIN) // Windows
DWORD WINAPI XcpTlMulticastThread(LPVOID par)
#elif defined(_LINUX) // Linux
extern void* XcpTlMulticastThread(void* par)
#endif
{
    uint8_t buffer[256];
    int16_t n;
    uint16_t srcPort;
    uint8_t srcAddr[4];

    (void)par;

    for (;;) {
        n = socketRecvFrom(gXcpTl.MulticastSock, buffer, (uint16_t)sizeof(buffer), srcAddr, &srcPort, NULL);
        if (n <= 0) break; // Terminate on error or socket close 
#ifdef XCLTL_RESTRICT_MULTICAST
        // Accept multicast from active master only
        if (gXcpTl.MasterAddrValid && memcmp(gXcpTl.MasterAddr, srcAddr, 4) == 0) {
            handleXcpMulticastCommand(n, (tXcpCtoMessage*)buffer, srcAddr, srcPort);
        }
        else {
            DBG_PRINTF_WARNING("WARNING: Ignored Multicast from %u.%u.%u.%u:%u\n", srcAddr[0], srcAddr[1], srcAddr[2], srcAddr[3], srcPort);
        }
#else
        handleXcpMulticastCommand(n, (tXcpCtoMessage*)buffer, srcAddr, srcPort);
#endif
    }
    DBG_PRINT3("XCP multicast thread terminated\n");
    socketClose(&gXcpTl.MulticastSock);
    return 0;
}

#endif // XCPTL_ENABLE_MULTICAST


//-------------------------------------------------------------------------------------------------------

BOOL XcpEthTlInit(const uint8_t* addr, uint16_t port, BOOL useTCP, BOOL blockingRx) {

    if (!XcpTlInit()) return FALSE;

    uint8_t bind_addr[4] = {0,0,0,0}; // Bind to ANY(0.0.0.0)
    if (addr != NULL)  { // Bind to given addr 
        memcpy(bind_addr, addr, 4);
    } 

    gXcpTl.ServerPort = port;
    gXcpTl.ServerUseTCP = useTCP;
    gXcpTl.blockingRx = blockingRx;
    gXcpTl.MasterAddrValid = FALSE;
    gXcpTl.Sock = INVALID_SOCKET;

    // Unicast UDP or TCP commands
#ifdef XCPTL_ENABLE_TCP
    gXcpTl.ListenSock = INVALID_SOCKET;
    if (useTCP) 
    { // TCP
        if (!socketOpen(&gXcpTl.ListenSock, TRUE /* useTCP */, !blockingRx, TRUE /*reuseAddr*/, FALSE /* timestamps*/)) return FALSE;
        if (!socketBind(gXcpTl.ListenSock, bind_addr, gXcpTl.ServerPort)) return FALSE; 
        if (!socketListen(gXcpTl.ListenSock)) return FALSE; // Put socket in listen mode
        DBG_PRINTF3("  Listening for TCP connections on %u.%u.%u.%u port %u\n", bind_addr[0], bind_addr[1], bind_addr[2], bind_addr[3], port);
    }
    else
#else
    if (useTCP) 
    { // TCP
        DBG_PRINT_ERROR("ERROR: #define XCPTL_ENABLE_TCP for TCP support\n");
        return FALSE;
    }
    else
#endif
    { // UDP
        if (!socketOpen(&gXcpTl.Sock, FALSE /* useTCP */, !blockingRx, TRUE /*reuseAddr*/, FALSE /* timestamps*/)) return FALSE;
        if (!socketBind(gXcpTl.Sock, bind_addr, port)) return FALSE; // Bind on ANY, when serverAddr=255.255.255.255
        DBG_PRINTF3("  Listening for XCP commands on UDP %u.%u.%u.%u port %u\n", bind_addr[0], bind_addr[1], bind_addr[2], bind_addr[3], port);
    }

#ifdef PLATFORM_ENABLE_GET_LOCAL_ADDR
    {
        uint8_t addr[4] = {0,0,0,0};
        uint8_t mac[6] = {0,0,0,0,0,0};
        socketGetLocalAddr(mac,addr); // Store actual MAC and IP addr for later use
        DBG_PRINTF3("  MAC=%02X.%02X.%02X.%02X.%02X.%02X IP=%u.%u.%u.%u\n", mac[0],mac[1],mac[2],mac[3],mac[4],mac[5], addr[0],addr[1],addr[2],addr[3]);
        if (bind_addr[0]==0) {
            memcpy(gXcpTl.ServerAddr,addr,4); // Store IP address for XcpEthTlGetInfo
        } else { 
            memcpy(gXcpTl.ServerAddr,bind_addr, 4); 
        }
        memcpy(gXcpTl.ServerMac,mac,6); // Store MAC address for XcpEthTlGetInfo
    }  
#endif  

    // Multicast UDP commands
#ifdef XCPTL_ENABLE_MULTICAST

      // Open a socket for GET_DAQ_CLOCK_MULTICAST and join its multicast group
      if (!socketOpen(&gXcpTl.MulticastSock, FALSE /*useTCP*/, FALSE /*nonblocking*/, TRUE /*reusable*/, FALSE /* timestamps*/)) return FALSE;
      DBG_PRINTF3("  Bind XCP multicast socket to %u.%u.%u.%u:%u\n", bind_addr[0], bind_addr[1], bind_addr[2], bind_addr[3], XCPTL_MULTICAST_PORT);
      if (!socketBind(gXcpTl.MulticastSock, bind_addr, XCPTL_MULTICAST_PORT)) return FALSE; // Bind to ANY, when serverAddr=255.255.255.255
      uint16_t cid = XcpGetClusterId();
      uint8_t maddr[4] = { 239,255,0,0 }; // XCPTL_MULTICAST_ADDR = 0xEFFFiiii; 
      maddr[2] = (uint8_t)(cid >> 8);
      maddr[3] = (uint8_t)(cid);
      if (!socketJoin(gXcpTl.MulticastSock, maddr)) return FALSE;
      DBG_PRINTF3("  Listening for XCP GET_DAQ_CLOCK multicast on %u.%u.%u.%u\n", maddr[0], maddr[1], maddr[2], maddr[3]);

      DBG_PRINT3("  Start XCP multicast thread\n");
      create_thread(&gXcpTl.MulticastThreadHandle, XcpTlMulticastThread);

#endif

    return TRUE;
}


void XcpEthTlShutdown() {

    // Close all sockets to enable all threads to terminate
#ifdef XCPTL_ENABLE_MULTICAST
    socketClose(&gXcpTl.MulticastSock);
    join_thread(gXcpTl.MulticastThreadHandle);
#endif
#ifdef XCPTL_ENABLE_TCP
    if (isTCP()) socketClose(&gXcpTl.ListenSock);
#endif
    socketClose(&gXcpTl.Sock);

    // Free other resources
    XcpTlShutdown();
}



//-------------------------------------------------------------------------------------------------------
#ifdef PLATFORM_ENABLE_GET_LOCAL_ADDR
void XcpEthTlGetInfo(BOOL* isTcp, uint8_t* mac, uint8_t* addr, uint16_t *port) {
  
    if (isTcp!=NULL) *isTcp = gXcpTl.ServerUseTCP;
    if (addr!=NULL) memcpy(addr, gXcpTl.ServerAddr, 4);
    if (mac!=NULL) memcpy(mac, gXcpTl.ServerMac, 6);
    if (port!=NULL) *port = gXcpTl.ServerPort;
}
#endif

#endif  // defined(XCPTL_ENABLE_UDP) || defined(XCPTL_ENABLE_TCP)



