#pragma once
/* A2L.h */
/* Copyright(c) Vector Informatik GmbH.All rights reserved.
   Licensed under the MIT license.See LICENSE file in the project root for details. */

#include <assert.h>  // for assert
#include <stdbool.h> // for bool
#include <stdint.h>  // for uintxx_t

#include "../xcplib.h" // for tXcpEventId, tXcpCalSegIndex
#include "platform.h"  // for atomic_bool

// ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------

// Basic A2L types
typedef int8_t tA2lTypeId; // A2L type ID, positive for unsigned types, negative for signed types
#define A2L_TYPE_UINT8 (tA2lTypeId)1
#define A2L_TYPE_UINT16 (tA2lTypeId)2
#define A2L_TYPE_UINT32 (tA2lTypeId)4
#define A2L_TYPE_UINT64 (tA2lTypeId)8
#define A2L_TYPE_INT8 (tA2lTypeId) - 1
#define A2L_TYPE_INT16 (tA2lTypeId) - 2
#define A2L_TYPE_INT32 (tA2lTypeId) - 4
#define A2L_TYPE_INT64 (tA2lTypeId) - 8
#define A2L_TYPE_FLOAT (tA2lTypeId) - 9
#define A2L_TYPE_DOUBLE (tA2lTypeId) - 10
#define A2L_TYPE_UNDEFINED (tA2lTypeId)0

static_assert(sizeof(char) == 1, "sizeof(char) must be 1 bytes for A2L types to work correctly");
static_assert(sizeof(short) == 2, "sizeof(short) must be 2 bytes for A2L types to work correctly");
static_assert(sizeof(long long) == 8, "sizeof(long long) must be 8 bytes for A2L types to work correctly");

// Macro to generate type
// A2L type
#define A2lGetTypeId(type)                                                                                                                                                         \
    _Generic((type),                                                                                                                                                               \
        signed char: A2L_TYPE_INT8,                                                                                                                                                \
        unsigned char: A2L_TYPE_UINT8,                                                                                                                                             \
        bool: A2L_TYPE_UINT8,                                                                                                                                                      \
        signed short: A2L_TYPE_INT16,                                                                                                                                              \
        unsigned short: A2L_TYPE_UINT16,                                                                                                                                           \
        signed int: (tA2lTypeId)(-sizeof(int)),                                                                                                                                    \
        unsigned int: (tA2lTypeId)sizeof(int),                                                                                                                                     \
        signed long: (tA2lTypeId)(-sizeof(long)),                                                                                                                                  \
        unsigned long: (tA2lTypeId)sizeof(long),                                                                                                                                   \
        signed long long: A2L_TYPE_INT64,                                                                                                                                          \
        unsigned long long: A2L_TYPE_UINT64,                                                                                                                                       \
        float: A2L_TYPE_FLOAT,                                                                                                                                                     \
        double: A2L_TYPE_DOUBLE,                                                                                                                                                   \
        default: A2L_TYPE_UNDEFINED)

// Macros to generate type names as static char* string
const char *A2lGetA2lTypeName(tA2lTypeId type);
const char *A2lGetA2lTypeName_M(tA2lTypeId type);
const char *A2lGetA2lTypeName_C(tA2lTypeId type);
const char *A2lGetRecordLayoutName_(tA2lTypeId type);

#define A2lGetTypeName(type) A2lGetA2lTypeName(A2lGetTypeId(type))
#define A2lGetTypeName_M(type) A2lGetA2lTypeName_M(A2lGetTypeId(type))
#define A2lGetTypeName_C(type) A2lGetA2lTypeName_C(A2lGetTypeId(type))
#define A2lGetRecordLayoutName(type) A2lGetRecordLayoutName_(A2lGetTypeId(type))

// ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------
extern MUTEX gA2lMutex;

// ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------
// Set mode (address generation and event) for all following A2lCreateXxxx macros and functions
// Not thread safe !!!!!

void A2lSetAbsAddrMode(void);                                                // Absolute addressing mode
void A2lSetSegAddrMode(tXcpCalSegIndex calseg_index, const uint8_t *calseg); // Calibration segment relative addressing mode
void A2lSetRelAddrMode(const tXcpEventId *event);                            // Relative addressing mode, event is used as base address, max offset is signed int 32 Bit
void A2lSetDynAddrMode(const tXcpEventId *event); // Dynamic addressing mode, event is used as base address with write access, offset limited to signed int 16 Bit
void A2lRstAddrMode(void);

void A2lSetRelativeAddrMode_(const char *event_name, const uint8_t *stack_frame_pointer);
void A2lSetAbsoluteAddrMode_(const char *event_name);

void A2lSetFixedEvent(tXcpEventId event);
void A2lRstFixedEvent(void);
void A2lSetDefaultEvent(tXcpEventId event);
void A2lRstDefaultEvent(void);

// ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------
// Stack frame relative addressing mode
// Can be used without runtime A2L file generation

#ifndef get_stack_frame_pointer
#define get_stack_frame_pointer() (uint8_t *)__builtin_frame_address(0)
#endif

// static inline uint8_t *get_stack_frame_pointer_(void) {
//  #if defined(__x86_64__) || defined(_M_X64)
//      void *fp;
//      __asm__ volatile("movq %%rbp, %0" : "=r"(fp));
//      return (uint8_t *)fp;
//  #elif defined(__i386__) || defined(_M_IX86)
//      void *fp;
//      __asm__ volatile("movl %%ebp, %0" : "=r"(fp));
//      return (uint8_t *)fp;
//  #elif defined(__aarch64__)
//      void *fp;
//      __asm__ volatile("mov %0, x29" : "=r"(fp));
//      return (uint8_t *)fp;
//  #elif defined(__arm__)
//      void *fp;
//      __asm__ volatile("mov %0, fp" : "=r"(fp));
//      return (uint8_t *)fp;
//  #else
//      return (uint8_t *)__builtin_frame_address(0);
//  #endif
//}

// Set addressing mode to relative for a given event 'name' and base address
// Error if the event does not exist
// Use in combination with DaqEvent(name)
#define A2lSetRelativeAddrMode(name, base_addr)                                                                                                                                    \
    {                                                                                                                                                                              \
        static atomic_bool a2l_mode_rel_##name##_ = false;                                                                                                                         \
        if (A2lOnce_(&a2l_mode_rel_##name##_))                                                                                                                                     \
            A2lSetRelativeAddrMode_(#name, (const uint8_t *)base_addr);                                                                                                            \
    }

// Set addressing mode to absolute and event 'name'
// Error if the event does not exist
// Use in combination with DaqEvent(name)
#define A2lSetAbsoluteAddrMode(name)                                                                                                                                               \
    {                                                                                                                                                                              \
        static atomic_bool a2l_mode_abs_##name##_ = false;                                                                                                                         \
        if (A2lOnce_(&a2l_mode_abs_##name##_))                                                                                                                                     \
            A2lSetAbsoluteAddrMode_(#name);                                                                                                                                        \
    }

// Set addressing mode to stack and event 'name'
// Error if the event does not exist
// Use in combination with DaqEvent(name)
#define A2lSetStackAddrMode(name)                                                                                                                                                  \
    {                                                                                                                                                                              \
        static atomic_bool a2l_mode_stack_##name##_ = false;                                                                                                                       \
        if (A2lOnce_(&a2l_mode_stack_##name##_))                                                                                                                                   \
            A2lSetRelativeAddrMode_(#name, get_stack_frame_pointer());                                                                                                             \
    }

// ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------
// Create parameters in a calibration segment or in global memory

// Once
#define A2lCreateParameter(instance_name, name, comment, unit, min, max)                                                                                                           \
    {                                                                                                                                                                              \
        static atomic_bool a2l_par_##name##_ = false;                                                                                                                              \
        if (A2lOnce_(&a2l_par_##name##_))                                                                                                                                          \
            A2lCreateParameter_(#instance_name "." #name, A2lGetTypeId(instance_name.name), A2lGetAddrExt_(), A2lGetAddr_((uint8_t *)&instance_name.name), comment, unit, min,     \
                                max);                                                                                                                                              \
    }

// Once
#define A2lCreateCurve(instance_name, name, xdim, comment, unit)                                                                                                                   \
    {                                                                                                                                                                              \
        static atomic_bool a2l_par_##name##_ = false;                                                                                                                              \
        if (A2lOnce_(&a2l_par_##name##_))                                                                                                                                          \
            A2lCreateCurve_(#instance_name "." #name, A2lGetTypeId(instance_name.name[0]), A2lGetAddrExt_(), A2lGetAddr_((uint8_t *)&instance_name.name[0]), xdim, comment, unit); \
    }

// Once
#define A2lCreateMap(instance_name, name, xdim, ydim, comment, unit)                                                                                                               \
    {                                                                                                                                                                              \
        static atomic_bool a2l_par_##name##_ = false;                                                                                                                              \
        if (A2lOnce_(&a2l_par_##name##_))                                                                                                                                          \
            A2lCreateMap_(#instance_name "." #name, A2lGetTypeId(instance_name.name[0][0]), A2lGetAddrExt_(), A2lGetAddr_((uint8_t *)&instance_name.name[0][0]), xdim, ydim,       \
                          comment, unit);                                                                                                                                          \
    }

// ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------
// Create measurements on stack or in global memory
// Measurements are registered once, it is allowed to use the following macros in local scope which is run multiple times

// Once
#define A2lCreateMeasurement(name, comment)                                                                                                                                        \
    {                                                                                                                                                                              \
        static atomic_bool a2l_##name##_ = false;                                                                                                                                  \
        if (A2lOnce_(&a2l_##name##_))                                                                                                                                              \
            A2lCreateMeasurement_(NULL, #name, A2lGetTypeId(name), A2lGetAddrExt_(), A2lGetAddr_((uint8_t *)&(name)), 1.0, 0.0, NULL, comment);                                    \
    }

// Thread safe
// Create thread local measurement instance, combine with XcpCreateEventInstance() and XcpEventDyn()
#define A2lCreateMeasurementInstance(instance_name, event, name, comment)                                                                                                          \
    {                                                                                                                                                                              \
        mutexLock(&gA2lMutex);                                                                                                                                                     \
        A2lSetDynAddrMode(&event);                                                                                                                                                 \
        A2lCreateMeasurement_(instance_name, #name, A2lGetTypeId(name), A2lGetAddrExt_(), A2lGetAddr_((uint8_t *)&(name)), 1.0, 0.0, NULL, comment);                               \
        mutexUnlock(&gA2lMutex);                                                                                                                                                   \
    }

// Once
#define A2lCreatePhysMeasurement(name, comment, factor, offset, unit)                                                                                                              \
    {                                                                                                                                                                              \
        static atomic_bool a2l_##name##_ = false;                                                                                                                                  \
        if (A2lOnce_(&a2l_##name##_))                                                                                                                                              \
            A2lCreateMeasurement_(NULL, #name, A2lGetTypeId(name), A2lGetAddrExt_(), A2lGetAddr_((uint8_t *)&name), factor, offset, unit, comment);                                \
    }

// Once
#define A2lCreateMeasurementArray(name, comment)                                                                                                                                   \
    {                                                                                                                                                                              \
        static atomic_bool a2l_##name##_ = false;                                                                                                                                  \
        if (A2lOnce_(&a2l_##name##_))                                                                                                                                              \
            A2lCreateMeasurementArray_(NULL, #name, A2lGetTypeId(name[0]), sizeof(name) / sizeof(name[0]), 1, A2lGetAddrExt_(), A2lGetAddr_(&name[0]), 1.0, 0.0, "", comment);     \
    }

// Once
#define A2lCreateMeasurementMatrix(name, comment)                                                                                                                                  \
    {                                                                                                                                                                              \
        static atomic_bool a2l_##name##_ = false;                                                                                                                                  \
        if (A2lOnce_(&a2l_##name##_))                                                                                                                                              \
            A2lCreateMeasurementArray_(NULL, #name, A2lGetTypeId(name[0][0]), sizeof(name[0]) / sizeof(name[0][0]), sizeof(name) / sizeof(name[0]), A2lGetAddrExt_(),              \
                                       A2lGetAddr_(&name[0]), 1.0, 0.0, "", comment);                                                                                              \
    }

// ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------
// Create typedefs and typedef components

// Once
#define A2lCreateTypedefInstance(name, typeName, comment)                                                                                                                          \
    {                                                                                                                                                                              \
        static atomic_bool a2l_##name##_ = false;                                                                                                                                  \
        if (A2lOnce_(&a2l_##name##_)) {                                                                                                                                            \
            A2lCreateTypedefInstance_(#name, #typeName, 0, A2lGetAddrExt_(), A2lGetAddr_((uint8_t *)&name), comment);                                                              \
        }                                                                                                                                                                          \
    }

// Once
#define A2lCreateTypedefReference(name, typeName, comment)                                                                                                                         \
    {                                                                                                                                                                              \
        static atomic_bool a2l_##name##_ = false;                                                                                                                                  \
        if (A2lOnce_(&a2l_##name##_)) {                                                                                                                                            \
            A2lCreateTypedefInstance_(#name, #typeName, 0, A2lGetAddrExt_(), A2lGetAddr_((uint8_t *)name), comment);                                                               \
        }                                                                                                                                                                          \
    }

// Once
#define A2lCreateTypedefArray(name, typeName, dim, comment)                                                                                                                        \
    {                                                                                                                                                                              \
        static atomic_bool a2l_##name##_ = false;                                                                                                                                  \
        if (A2lOnce_(&a2l_##name##_)) {                                                                                                                                            \
            A2lCreateTypedefInstance_(#name, #typeName, dim, A2lGetAddrExt_(), A2lGetAddr_((uint8_t *)&name), comment);                                                            \
        }                                                                                                                                                                          \
    }

// Once
#define A2lCreateTypedefArrayReference(name, typeName, dim, comment)                                                                                                               \
    {                                                                                                                                                                              \
        static atomic_bool a2l_##name##_ = false;                                                                                                                                  \
        if (A2lOnce_(&a2l_##name##_)) {                                                                                                                                            \
            A2lCreateTypedefInstance_(#name, #typeName, dim, A2lGetAddrExt_(), A2lGetAddr_((uint8_t *)name), comment);                                                             \
        }                                                                                                                                                                          \
    }

// Once
#define A2lTypedefBegin(type_name, comment)                                                                                                                                        \
    {                                                                                                                                                                              \
        static atomic_bool a2l_##type_name##_ = false;                                                                                                                             \
        if (A2lOnce_(&a2l_##type_name##_)) {                                                                                                                                       \
            A2lTypedefBegin_(#type_name, (uint32_t)sizeof(type_name), comment);                                                                                                    \
        }                                                                                                                                                                          \
    }

// Once
#define A2lTypedefMeasurementComponent(field_name, typedef_name)                                                                                                                   \
    {                                                                                                                                                                              \
        static atomic_bool a2l_##name##_ = false;                                                                                                                                  \
        if (A2lOnce_(&a2l_##name##_)) {                                                                                                                                            \
            typedef_name instance;                                                                                                                                                 \
            A2lTypedefComponent_(#field_name, A2lGetTypeName_M(instance.field_name), 1, ((uint8_t *)&(instance.field_name) - (uint8_t *)&instance));                               \
        }                                                                                                                                                                          \
    }

// Once
#define A2lTypedefParameterComponent(field_name, typeName, comment, unit, min, max)                                                                                                \
    {                                                                                                                                                                              \
        static atomic_bool a2l_##field_name##_ = false;                                                                                                                            \
        if (A2lOnce_(&a2l_##field_name##_)) {                                                                                                                                      \
            typeName instance;                                                                                                                                                     \
            A2lTypedefParameterComponent_(#field_name, A2lGetRecordLayoutName(instance.field_name), 1, 1, ((uint8_t *)&(instance.field_name) - (uint8_t *)&instance), comment,     \
                                          unit, min, max);                                                                                                                         \
        }                                                                                                                                                                          \
    }

// Once
#define A2lTypedefCurveComponent(field_name, typeName, x_dim, comment, unit, min, max)                                                                                             \
    {                                                                                                                                                                              \
        static atomic_bool a2l_##field_name##_ = false;                                                                                                                            \
        if (A2lOnce_(&a2l_##field_name##_)) {                                                                                                                                      \
            typeName instance;                                                                                                                                                     \
            A2lTypedefParameterComponent_(#field_name, A2lGetRecordLayoutName(instance.field_name[0]), x_dim, 1, ((uint8_t *)&(instance.field_name) - (uint8_t *)&instance),       \
                                          comment, unit, min, max);                                                                                                                \
        }                                                                                                                                                                          \
    }

// Once
#define A2lTypedefMapComponent(field_name, typeName, x_dim, y_dim, comment, unit, min, max)                                                                                        \
    {                                                                                                                                                                              \
        static atomic_bool a2l_##field_name##_ = false;                                                                                                                            \
        if (A2lOnce_(&a2l_##field_name##_)) {                                                                                                                                      \
            typeName instance;                                                                                                                                                     \
            A2lTypedefParameterComponent_(#field_name, A2lGetRecordLayoutName(instance.field_name[0][0]), x_dim, y_dim,                                                            \
                                          ((uint8_t *)&(instance.field_name) - (uint8_t *)&instance), comment, unit, min, max);                                                    \
        }                                                                                                                                                                          \
    }

// Once
#define A2lTypedefMeasurementArrayComponent(field_name, typedef_name)                                                                                                              \
    {                                                                                                                                                                              \
        static atomic_bool a2l_##field_name##_ = false;                                                                                                                            \
        if (A2lOnce_(&a2l_##field_name##_)) {                                                                                                                                      \
            typedef_name instance;                                                                                                                                                 \
            A2lTypedefComponent_(#field_name, A2lGetTypeName_M(instance.field_name[0]), sizeof(instance.field_name) / sizeof(instance.field_name[0]),                              \
                                 ((uint8_t *)&(instance.field_name[0]) - (uint8_t *)&instance));                                                                                   \
        }                                                                                                                                                                          \
    }

// Once
#define A2lTypedefParameterArrayComponent(field_name, typeName, comment, unit, min, max)                                                                                           \
    {                                                                                                                                                                              \
        static atomic_bool a2l_##field_name##_ = false;                                                                                                                            \
        if (A2lOnce_(&a2l_##field_name##_)) {                                                                                                                                      \
            typeName instance;                                                                                                                                                     \
            A2lTypedefComponent_(#field_name, A2lGetTypeName_C(instance.field_name[0]), sizeof(instance.field_name) / sizeof(instance.field_name[0]),                              \
                                 ((uint8_t *)&(instance.field_name[0]) - (uint8_t *)&instance));                                                                                   \
        }                                                                                                                                                                          \
    }

// Once
#define A2lTypedefParameterMatrixComponent(field_name, typeName, comment, unit, min, max)                                                                                          \
    {                                                                                                                                                                              \
        static atomic_bool a2l_##field_name##_ = false;                                                                                                                            \
        if (A2lOnce_(&a2l_##field_name##_)) {                                                                                                                                      \
            typeName instance;                                                                                                                                                     \
            A2lTypedefComponent_(#field_name, A2lGetTypeName_C(instance.field_name[0][0]), sizeof(instance.field_name) / sizeof(instance.field_name[0]),                           \
                                 ((uint8_t *)&(instance.field_name[0]) - (uint8_t *)&instance));                                                                                   \
        }                                                                                                                                                                          \
    }

// Once
#define A2lTypedefComponent(field_name, field_type_name, field_dim, typedef_name)                                                                                                  \
    {                                                                                                                                                                              \
        static atomic_bool a2l_##field_name##_ = false;                                                                                                                            \
        if (A2lOnce_(&a2l_##field_name##_)) {                                                                                                                                      \
            typedef_name instance;                                                                                                                                                 \
            A2lTypedefComponent_(#field_name, #field_type_name, field_dim, ((uint8_t *)&(instance.field_name) - (uint8_t *)&instance));                                            \
        }                                                                                                                                                                          \
    }

// Once
#define A2lTypedefEnd()                                                                                                                                                            \
    {                                                                                                                                                                              \
        static atomic_bool a2l_once = false;                                                                                                                                       \
        if (A2lOnce_(&a2l_once)) {                                                                                                                                                 \
            A2lTypedefEnd_();                                                                                                                                                      \
        }                                                                                                                                                                          \
    }

// ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------
// Create groups

void A2lParameterGroup(const char *name, int count, ...);
void A2lParameterGroupFromList(const char *name, const char *pNames[], int count);
void A2lMeasurementGroup(const char *name, int count, ...);
void A2lMeasurementGroupFromList(const char *name, char *names[], uint32_t count);

// ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------

// Init A2L generation
bool A2lInit(const char *a2l_filename, const char *a2l_projectname, const uint8_t *addr, uint16_t port, bool useTCP, bool finalize_on_connect);

// Finish A2L generation
bool A2lFinalize(void);

// --------------------------------------------------------------------------------------------
// Helper functions used in the for A2L generation macros

bool A2lOnce_(atomic_bool *once);

uint32_t A2lGetAddr_(const void *addr);
uint8_t A2lGetAddrExt_(void);

// Create measurements
void A2lCreateMeasurement_(const char *instance_name, const char *name, tA2lTypeId type, uint8_t ext, uint32_t addr, double factor, double offset, const char *unit,
                           const char *comment);

void A2lCreateMeasurementArray_(const char *instance_name, const char *name, tA2lTypeId type, int x_dim, int y_dim, uint8_t ext, uint32_t addr, double factor, double offset,
                                const char *unit, const char *comment);

// Create typedefs
void A2lTypedefBegin_(const char *name, uint32_t size, const char *comment);
void A2lTypedefComponent_(const char *name, const char *type_name, uint16_t x_dim, uint32_t offset);
void A2lTypedefParameterComponent_(const char *name, const char *type_name, uint16_t x_dim, uint16_t y_dim, uint32_t offset, const char *comment, const char *unit, double min,
                                   double max);
void A2lTypedefEnd_(void);
void A2lCreateTypedefInstance_(const char *instance_name, const char *type_name, uint16_t x_dim, uint8_t ext, uint32_t addr, const char *comment);

// Create parameters
void A2lCreateParameter_(const char *name, tA2lTypeId type, uint8_t ext, uint32_t addr, const char *comment, const char *unit, double min, double max);
void A2lCreateMap_(const char *name, tA2lTypeId type, uint8_t ext, uint32_t addr, uint32_t xdim, uint32_t ydim, const char *comment, const char *unit);
void A2lCreateCurve_(const char *name, tA2lTypeId type, uint8_t ext, uint32_t addr, uint32_t xdim, const char *comment, const char *unit);
