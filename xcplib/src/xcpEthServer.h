#pragma once
#define __XCP_ETH_SERVER_H__

/* Copyright(c) Vector Informatik GmbH.All rights reserved.
   Licensed under the MIT license.See LICENSE file in the project root for details. */

#include <stdbool.h>
#include <stdint.h>

#include "src/xcptl_cfg.h" // for XCPTL_ENABLE_UDP, ...

#if defined(XCPTL_ENABLE_UDP) || defined(XCPTL_ENABLE_TCP)

extern bool XcpEthServerInit(const uint8_t *addr, uint16_t port, bool useTCP, uint32_t queueSize);
extern bool XcpEthServerShutdown(void);
extern bool XcpEthServerStatus(void);

#endif
