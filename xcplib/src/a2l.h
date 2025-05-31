#pragma once
/* A2L.h */
/* Copyright(c) Vector Informatik GmbH.All rights reserved.
   Licensed under the MIT license.See LICENSE file in the project root for details. */

#include <platform.h> // for atomic_bool
#include <stdbool.h>  // for bool
#include <stdint.h>   // for uint8_t, uint32_t, uint64_t

#define A2L_TYPE_UINT8 1
#define A2L_TYPE_UINT16 2
#define A2L_TYPE_UINT32 4
#define A2L_TYPE_UINT64 8
#define A2L_TYPE_INT8 -1
#define A2L_TYPE_INT16 -2
#define A2L_TYPE_INT32 -4
#define A2L_TYPE_INT64 -8
#define A2L_TYPE_FLOAT -9
#define A2L_TYPE_DOUBLE -10

// Create parameters
#define A2lCreateParameter(name, type, comment, unit) A2lCreateParameter_(#name, type, A2lGetAddrExt(), A2lGetAddr((uint8_t *)&name), comment, unit)
#define A2lCreateParameterWithLimits(name, type, comment, unit, min, max)                                                                                                          \
    A2lCreateParameterWithLimits_(#name, type, A2lGetAddrExt(), A2lGetAddr((uint8_t *)&name), comment, unit, min, max)
#define A2lCreateCurve(name, type, xdim, comment, unit) A2lCreateCurve_(#name, type, A2lGetAddrExt(), A2lGetAddr((uint8_t *)&name[0]), xdim, comment, unit)
#define A2lCreateMap(name, type, xdim, ydim, comment, unit) A2lCreateMap_(#name, type, A2lGetAddrExt(), A2lGetAddr((uint8_t *)&name[0][0]), xdim, ydim, comment, unit)

// Create measurements
// Measurements are registered once, it is allowed to use the following macros in local scope which is run multiple times
#define A2lCreateMeasurement(name, type, comment)                                                                                                                                  \
    {                                                                                                                                                                              \
        static atomic_bool __once = false;                                                                                                                                         \
        if (A2lOnce(&__once))                                                                                                                                                      \
            A2lCreateMeasurement_(NULL, #name, type, A2lGetAddrExt(), A2lGetAddr((uint8_t *)&(name)), 1.0, 0.0, NULL, comment);                                                    \
    }

#define A2lCreatePhysMeasurement(name, type, comment, factor, offset, unit)                                                                                                        \
    {                                                                                                                                                                              \
        static atomic_bool __once = false;                                                                                                                                         \
        if (A2lOnce(&__once))                                                                                                                                                      \
            A2lCreateMeasurement_(NULL, #name, type, A2lGetAddrExt(), A2lGetAddr((uint8_t *)&name), factor, offset, unit, comment);                                                \
    }

#define A2lCreateMeasurementArray(name, type)                                                                                                                                      \
    {                                                                                                                                                                              \
        static atomic_bool __once = false;                                                                                                                                         \
        if (A2lOnce(&__once))                                                                                                                                                      \
            A2lCreateMeasurementArray_(NULL, #name, type, sizeof(name) / sizeof(name[0]), A2lGetAddrExt(), A2lGetAddr(&name[0]));                                                  \
    }

#define A2lCreateTypedefInstance(instanceName, typeName, comment)                                                                                                                  \
    {                                                                                                                                                                              \
        static atomic_bool __once = false;                                                                                                                                         \
        if (A2lOnce(&__once))                                                                                                                                                      \
            A2lCreateTypedefInstance_(#instanceName, #typeName, A2lGetAddrExt(), A2lGetAddr((uint8_t *)&instanceName), comment);                                                   \
    }

// Create typedefs
#define A2lTypedefBegin(name, comment) A2lTypedefBegin_(#name, (uint32_t)sizeof(name), comment)
#define A2lTypedefComponent(fieldName, type, instanceName) A2lTypedefMeasurementComponent_(#fieldName, type, ((uint8_t *)&(instanceName.fieldName) - (uint8_t *)&instanceName))
#define A2lTypedefEnd() A2lTypedefEnd_()

// Init A2L generation
bool A2lOpen(const char *filename, const char *projectName);

// Create memory segments
void A2lCreate_MOD_PAR(char *epk);

// Create XCP IF_DATA
void A2lCreate_ETH_IF_DATA(bool useTCP, const uint8_t *addr, uint16_t port);

// Set for all following A2lCreateXxxx
void A2lSetAbsAddrMode();
void A2lSetSegAddrMode(uint16_t calseg_index, const uint8_t *calseg);
void A2lSetRelAddrMode(const uint16_t *event);
void A2lSetFixedEvent(uint16_t event);
void A2lRstFixedEvent();
void A2lSetDefaultEvent(uint16_t event);
void A2lRstDefaultEvent();

// Create measurements
void A2lCreateMeasurement_(const char *instanceName, const char *name, int32_t type, uint8_t ext, uint32_t addr, double factor, double offset, const char *unit,
                           const char *comment);
void A2lCreateMeasurementArray_(const char *instanceName, const char *name, int32_t type, int dim, uint8_t ext, uint32_t addr);

// Create typedefs
void A2lTypedefBegin_(const char *name, uint32_t size, const char *comment);
void A2lTypedefMeasurementComponent_(const char *name, int32_t type, uint32_t offset);
void A2lTypedefParameterComponent_(const char *name, int32_t type, uint32_t offset);
void A2lTypedefEnd_();
void A2lCreateTypedefInstance_(const char *instanceName, const char *typeName, uint8_t ext, uint32_t addr, const char *comment);

// Create parameters
void A2lCreateParameter_(const char *name, int32_t type, uint8_t ext, uint32_t addr, const char *comment, const char *unit);
void A2lCreateParameterWithLimits_(const char *name, int32_t type, uint8_t ext, uint32_t addr, const char *comment, const char *unit, double min, double max);
void A2lCreateMap_(const char *name, int32_t type, uint8_t ext, uint32_t addr, uint32_t xdim, uint32_t ydim, const char *comment, const char *unit);
void A2lCreateCurve_(const char *name, int32_t type, uint8_t ext, uint32_t addr, uint32_t xdim, const char *comment, const char *unit);

// Create groups
void A2lParameterGroup(const char *name, int count, ...);
void A2lParameterGroupFromList(const char *name, const char *pNames[], int count);
void A2lMeasurementGroup(const char *name, int count, ...);
void A2lMeasurementGroupFromList(const char *name, char *names[], uint32_t count);

// Finish A2L generation
void A2lClose();

// Helpers for A2L generation macros
uint32_t A2lGetAddr(const uint8_t *addr);
uint8_t A2lGetAddrExt();
bool A2lOnce(atomic_bool *once);
