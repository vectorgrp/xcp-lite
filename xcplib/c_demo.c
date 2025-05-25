
#include <assert.h>  // for assert
#include <stdbool.h> // for bool
#include <stdint.h>  // for uint8_t, uint32_t, uint64_t
#include <stdio.h>   // for fclose, fopen, fread, fseek, ftell
#include <string.h>  // for strlen, strncpy

#include "platform.h" // for sleepMs

#include "xcpEthServer.h" // for XcpEthServerInit, XcpEthServerShutdown, XcpEthServerStatus
#include "xcpLite.h"      // for XcpInit, XcpEventExt, XcpCreateEvent

//-----------------------------------------------------------------------------------------------------

// XCP parameters
#define OPTION_USE_TCP false            // TCP or UDP
#define OPTION_SERVER_PORT 5555         // Port
#define OPTION_SERVER_ADDR {0, 0, 0, 0} // Bind addr, 0.0.0.0 = ANY
#define OPTION_QUEUE_SIZE 1024 * 16     // Size of the measurement queue in bytes, must be a multiple of 8

uint32_t gOptionQueueSize = OPTION_QUEUE_SIZE;
bool gOptionUseTCP = OPTION_USE_TCP;
uint16_t gOptionPort = OPTION_SERVER_PORT;
uint8_t gOptionBindAddr[4] = OPTION_SERVER_ADDR;

//-----------------------------------------------------------------------------------------------------

// Demo calibration parameters
struct params_t {
    uint16_t counter_max; // Maximum value for the counters
} params = {.counter_max = 1000};

//-----------------------------------------------------------------------------------------------------

// Global demo measurement variable
static uint16_t counter_static = 0;

//-----------------------------------------------------------------------------------------------------

// Demo main
void c_demo(void) {

    printf("\nXCP on Ethernet C Demo\n");

    // Initialize the XCP Server
    XcpInit(); // Initialize XCP singleton, must be called before XcpEthServerInit()
    if (!XcpEthServerInit(gOptionBindAddr, gOptionPort, gOptionUseTCP, NULL, gOptionQueueSize)) {
        return;
    }

    // Create a calibration segment for parameters
    uint16_t calseg = XcpCreateCalSeg("params", &params, sizeof(params));

    // Register calibration parameters in calseg
    // XcpRegisterParameter(calseg, "params.counter_max", &params.counter_max, sizeof(params.counter_max));

    // A demo variable on stack
    uint16_t counter = 0;

    // Create a measurement event
    uint16_t event = XcpCreateEvent("mainloop", 0, 0);

    // Register a measurement for the counter
    // XcpRegisterMeasurement(event, "counter", &counter, XCP_MEASUREMENT_TYPE_UINT16);

    for (;;) {
        sleepMs(100);

        // Lock the calibration segment for consistent and thread safe access
        // struct params_t *params = (struct params_t *)XcpLockCalseg(calseg);

        counter++;
        counter_static++;
        if (counter > params.counter_max) {
            counter = 0;
            counter_static = 0;
        }

        // Unlock the calibration segment
        // XcpUnlockCalSeg(calseg);

        // Trigger a measurement event
        XcpEventExt(event, (void *)&event);

        // Check server status
        if (!XcpEthServerStatus()) {
            printf("\nXCP Server failed\n");
            break;
        }
    }

    // Stop the XCP server
    XcpEthServerShutdown();
}
