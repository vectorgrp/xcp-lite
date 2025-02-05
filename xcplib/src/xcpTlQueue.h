#pragma once
#define __XCP_TL_QUEUE_h__

/* Copyright(c) Vector Informatik GmbH.All rights reserved.
   Licensed under the MIT license.See LICENSE file in the project root for details. */

#include <stdbool.h>
#include <stdint.h>

extern void XcpTlInitTransmitQueue(void *queue, uint32_t queueSize);
extern void XcpTlResetTransmitQueue(void);
extern void XcpTlFreeTransmitQueue(void);
