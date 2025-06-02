/*----------------------------------------------------------------------------
| File:
|   xcpQueue32.c
|
| Description:
|   XCP transport layer queue
|   Multi producer single consumer queue (producer side thread safe, not consumer side)
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
#include <stdint.h>   // for uintxx_t
#include <stdio.h>    // for NULL, snprintf
#include <stdlib.h>   // for free, malloc
#include <string.h>   // for memcpy, strcmp

#include "dbg_print.h" // for DBG_LEVEL, DBG_PRINT3, DBG_PRINTF4, DBG...
#include "platform.h"  // for platform defines (WIN_, LINUX_, MACOS_) and specific implementation of atomics, sockets, clock, thread, mutex
#include "xcpEthTl.h"  // for tXcpDtoMessage

#error "Not implemented yet"