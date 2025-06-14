// hello_xcp xcplib example

#include <assert.h>  // for assert
#include <stdbool.h> // for bool
#include <stdint.h>  // for uintxx_t
#include <stdio.h>   // for printf
#include <string.h>  // for sprintf

#include "a2l.h"          // for A2l generation
#include "platform.h"     // for sleepMs, clockGet
#include "xcpEthServer.h" // for XcpEthServerInit, XcpEthServerShutdown, XcpEthServerStatus
#include "xcpLite.h"      // for XcpInit, XcpEventExt, XcpCreateEvent, XcpCreateCalSeg, ...

//-----------------------------------------------------------------------------------------------------

// XCP parameters
#define OPTION_A2L_PROJECT_NAME "hello_xcp"  // A2L project name
#define OPTION_A2L_FILE_NAME "hello_xcp.a2l" // A2L filename
#define OPTION_USE_TCP false                 // TCP or UDP
#define OPTION_SERVER_PORT 5555              // Port
// #define OPTION_SERVER_ADDR {0, 0, 0, 0} // Bind addr, 0.0.0.0 = ANY
#define OPTION_SERVER_ADDR {192, 168, 8, 110} // Bind addr, 0.0.0.0 = ANY
#define OPTION_QUEUE_SIZE 1024 * 16           // Size of the measurement queue in bytes, must be a multiple of 8

//-----------------------------------------------------------------------------------------------------

// Demo calibration parameters
typedef struct params {
    uint16_t counter_max; // Maximum value for the counters
    uint32_t delay_us;    // Delay in microseconds for the main loop
    int8_t test_byte1;
    int8_t test_byte2;
} params_t;

// Default values
const params_t params = {.counter_max = 1000, .delay_us = 1000, .test_byte1 = -1, .test_byte2 = 1};

//-----------------------------------------------------------------------------------------------------

// Global demo measurement variable
static uint16_t counter = 0;

//-----------------------------------------------------------------------------------------------------

// Demo main
int main(void) {

    printf("\nXCP on Ethernet hello_xcp C xcplib demo\n");

    // Set log level (1-error, 2-warning, 3-info, 4-show XCP commands)
    XcpSetLogLevel(3);

    // Initialize the XCP singleton, must be called before starting the server
    XcpInit();

    // Initialize the XCP Server
    uint8_t addr[4] = OPTION_SERVER_ADDR;
    if (!XcpEthServerInit(addr, OPTION_SERVER_PORT, OPTION_USE_TCP, OPTION_QUEUE_SIZE)) {
        return 1;
    }

    // Prepare the A2L file
    if (!A2lInit(OPTION_A2L_FILE_NAME, OPTION_A2L_PROJECT_NAME, addr, OPTION_SERVER_PORT, OPTION_USE_TCP, true)) {
        return 1;
    }

    // Create a calibration segment for the calibration parameter struct
    // This segment has a working page (RAM) and a reference page (FLASH), it creates a MEMORY_SEGMENT in the A2L file
    // It provides safe (thread safe against XCP modifications), lock-free and consistent access to the calibration parameters
    // It supports XCP/ECU independant page switching, checksum calculation and reinitialization (copy reference page to working page)
    // Note that it can be used in only one ECU thread (in Rust terminology, it is Send, but not Sync)
    uint16_t calseg = XcpCreateCalSeg("params", (const uint8_t *)&params, sizeof(params));

    // Register individual calibration parameters in the calibration segment
    A2lSetSegAddrMode(calseg, (uint8_t *)&params);
    A2lCreateParameterWithLimits(params.counter_max, A2L_TYPE_UINT16, "maximum counter value", "", 0, 2000);
    A2lCreateParameterWithLimits(params.delay_us, A2L_TYPE_UINT32, "mainloop delay time in ue", "us", 0, 1000000);

    // Create a measurement event
    uint16_t event = XcpCreateEvent("mainloop", 0, 0);

    // Register a global measurement variable
    A2lSetAbsAddrMode(); // Set absolute addressing
    A2lCreatePhysMeasurement(counter, A2L_TYPE_UINT16, "Measurement variable", 1.0, 0.0, "counts");

    for (;;) {
        // Lock the calibration parameter segment for consistent and safe access
        // Calibration segment locking is completely lock-free and wait-free (no mutexes, system calls or CAS operations )
        // It returns a pointer to the active page (working or reference) of the calibration segment
        params_t *params = (params_t *)XcpLockCalSeg(calseg);

        // Sleep for the specified delay parameter in microseconds
        sleepNs(params->delay_us * 1000);

        // Local variable for measurement
        counter++;
        if (counter > params->counter_max) {
            counter = 0;
        }

        // Unlock the calibration segment
        XcpUnlockCalSeg(calseg);

        // Trigger measurement events
        XcpEvent(event);

    } // for(;;)

    // Force disconnect the XCP client
    XcpDisconnect();

    // Stop the XCP server
    XcpEthServerShutdown();

    return 0;
}
