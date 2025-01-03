// wrapper.h
// Source file for bindgen to generate Rust bindings for the XCPlite library



typedef unsigned char uint8_t;
typedef unsigned short uint16_t;
typedef unsigned long uint32_t;
typedef signed long int32_t;
typedef unsigned long long uint64_t;
typedef signed long long int64_t;

#define BOOL uint8_t
#define FALSE 0
#define TRUE 1

#include "main_cfg.h"
#include "src/xcpLite.h"   
#include "src/xcpDaq.h"   
#include "xcpAppl.h"   
#include "src/xcpEthTl.h"
#include "src/xcpEthServer.h"


