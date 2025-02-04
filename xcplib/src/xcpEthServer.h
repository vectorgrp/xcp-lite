#pragma once
/* xcpServer.h */

/* Copyright(c) Vector Informatik GmbH.All rights reserved.
   Licensed under the MIT license.See LICENSE file in the project root for details. */

#if defined(XCPTL_ENABLE_UDP) || defined(XCPTL_ENABLE_TCP)

extern bool XcpEthServerInit(const uint8_t *addr, uint16_t port, bool useTCP, void *queue, uint32_t queueSize);
extern bool XcpEthServerShutdown(void);
extern bool XcpEthServerStatus(void);

#endif
