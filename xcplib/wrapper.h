// Source file for bindgen to generate Rust bindings for the XCPlite library

#define __WRAPPER_H__

#include <stdbool.h>
#include <stdint.h>

#define _LINUX
#if defined(_ix64_) || defined(__x86_64__) || defined(__aarch64__)
#define _LINUX64
#ifdef __APPLE__
#define _MACOS
#endif
#endif

#include "main_cfg.h"
#include "src/xcp_cfg.h"
#include "src/xcpLite.h"
#include "src/xcptl_cfg.h"
#include "src/xcpTl.h"
#include "src/xcpEthTl.h"
#include "src/xcpEthServer.h"
#include "xcpAppl.h"
