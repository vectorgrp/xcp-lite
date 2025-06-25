// hello_xcp xcplib example

#include <assert.h>  // for assert
#include <stdbool.h> // for bool
#include <stdint.h>  // for uintxx_t
#include <stdio.h>   // for printf
#include <string.h>  // for sprintf

#include "a2l.h"      // for xcplib A2l generation
#include "platform.h" // for sleepNs
#include "xcplib.h"   // for xcplib application programming interface

//-----------------------------------------------------------------------------------------------------

// XCP parameters
#define OPTION_ENABLE_A2L_GENERATOR          // Enable A2L file generation
#define OPTION_A2L_PROJECT_NAME "hello_xcp"  // A2L project name
#define OPTION_A2L_FILE_NAME "hello_xcp.a2l" // A2L filename
#define OPTION_USE_TCP false                 // TCP or UDP
#define OPTION_SERVER_PORT 5555              // Port
#define OPTION_SERVER_ADDR {0, 0, 0, 0}      // Bind addr, 0.0.0.0 = ANY
// #define OPTION_SERVER_ADDR {127, 0, 0, 1} // Bind addr, 0.0.0.0 = ANY
#define OPTION_QUEUE_SIZE 1024 * 16 // Size of the measurement queue in bytes, must be a multiple of 8
#define OPTION_LOG_LEVEL 3

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

// Demo main
int main(void) {

    printf("\nXCP on Ethernet hello_xcp C xcplib demo\n");

    // Set log level (1-error, 2-warning, 3-info, 4-show XCP commands)
    XcpSetLogLevel(OPTION_LOG_LEVEL);

    // Initialize the XCP singleton, must be called before starting the server
    XcpInit();

    // Initialize the XCP Server
    uint8_t addr[4] = OPTION_SERVER_ADDR;
    if (!XcpEthServerInit(addr, OPTION_SERVER_PORT, OPTION_USE_TCP, OPTION_QUEUE_SIZE)) {
        return 1;
    }

    // Enable A2L generation and prepare the A2L file, finalize the A2L file on XCP connect
#ifdef OPTION_ENABLE_A2L_GENERATOR
    if (!A2lInit(OPTION_A2L_FILE_NAME, OPTION_A2L_PROJECT_NAME, addr, OPTION_SERVER_PORT, OPTION_USE_TCP, true)) {
        return 1;
    }
#else
    // Set the A2L filename for upload, assuming the A2L file exists
    ApplXcpSetA2lName(OPTION_A2L_FILE_NAME);
#endif

    // Create a calibration segment for the calibration parameter struct
    // This segment has a working page (RAM) and a reference page (FLASH), it creates a MEMORY_SEGMENT in the A2L file
    // It provides safe (thread safe against XCP modifications), lock-free and consistent access to the calibration parameters
    // It supports XCP/ECU independant page switching, checksum calculation and reinitialization (copy reference page to working page)
    // Note that it can be used in only one ECU thread (in Rust terminology, it is Send, but not Sync)
    uint16_t calseg = XcpCreateCalSeg("params", (const uint8_t *)&params, sizeof(params));

    // Register calibration parameters in the calibration segment
    A2lSetSegAddrMode(calseg, (uint8_t *)&params);
    A2lCreateParameterWithLimits(params, counter_max, "Maximum counter value", "", 0, 2000);
    A2lCreateParameterWithLimits(params, delay_us, "Mainloop delay time in us", "us", 0, 999999);

    // Create a measurement event
    DaqCreateEvent(mainloop);

    // Register a global measurement variable (counter)
    static uint16_t counter = 0;
    A2lSetAbsoluteAddrMode(mainloop); // Set absolute addressing mode with event mainloop
    A2lCreateMeasurement(counter, "Measurement variable in global memory");

    // Register a local measurement variable (counter_local)
    uint16_t counter_local = 0;
    A2lSetStackAddrMode(mainloop); // Set stack relative addressing mode with event mainloop
    A2lCreateMeasurement(counter_local, "Measurement variable on stack");

    A2lFinalize(); // Optional: Finalize the A2L file generation early, otherwise it would be written when the client tool connects

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

        counter_local = counter + 10;

        // Trigger measurement events
        DaqEvent(mainloop);

    } // for (;;)

    // Force disconnect the XCP client
    XcpDisconnect();

    // Stop the XCP server
    XcpEthServerShutdown();

    return 0;
}
