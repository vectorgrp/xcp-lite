
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
// A2L file generation options

#include "a2l.h" // for sleepMs

#define OPTION_A2L_NAME "C_Demo"          // A2L name
#define OPTION_A2L_FILE_NAME "C_Demo.a2l" // A2L filename

//-----------------------------------------------------------------------------------------------------

// Demo calibration parameters
struct params_t {
    uint16_t counter_max; // Maximum value for the counters
    uint32_t delay_us;    // Delay in microseconds for the main loop
} params = {.counter_max = 1000, .delay_us = 10000};

//-----------------------------------------------------------------------------------------------------

// Global demo measurement variable
static uint16_t counter_global = 0;

//-----------------------------------------------------------------------------------------------------

// Demo main
void c_demo(void) {

    printf("\nXCP on Ethernet C xcplib demo\n");

    // Set log level (1-error, 2-warning, 3-info, 4-debug)
    XcpSetLogLevel(3);

    // Initialize the XCP singleton, must be called before starting the server
    XcpInit();

    // Initialize the XCP Server
    if (!XcpEthServerInit(gOptionBindAddr, gOptionPort, gOptionUseTCP, NULL, gOptionQueueSize)) {
        return;
    }

    // Prepare the A2L file
    if (!A2lOpen(OPTION_A2L_FILE_NAME, OPTION_A2L_NAME)) {
        printf("Failed to open A2L file %s\n", OPTION_A2L_FILE_NAME);
        return;
    }

    // Create a calibration segment for parameters
    uint16_t calseg = XcpCreateCalSeg("params", &params, sizeof(params));

    // Register calibration parameters in calseg with segment relative addresses
    A2lSetSegAddrMode(calseg, (uint8_t *)&params);
    A2lCreateParameterWithLimits(params.counter_max, A2L_TYPE_UINT16, "maximum counter value", "", 0, 2000);
    A2lCreateParameterWithLimits(params.delay_us, A2L_TYPE_UINT32, "mainloop delay time in ue", "us", 0, 1000000);

    // Create a measurement event for global variables
    uint16_t event_global = XcpCreateEvent("mainloop_global", 0, 0);

    // Register measurement variables located on stack
    // A2lSetAbsAddrMode(); // Enable absolute addressing
    // A2lCreatePhysMeasurement(counter_global, A2L_TYPE_UINT16, "Measurement variable", 1.0, 0.0, "counts");

    // A demo variable on stack
    uint16_t counter = 0;

    // Create a measurement event for local variables
    uint16_t event = XcpCreateEvent("mainloop_local", 0, 0);

    // Register measurement variables located on stack
    A2lSetRelAddrMode(&event); // Enable event relative addressing
    A2lCreatePhysMeasurement(counter, A2L_TYPE_UINT16, "Measurement variable", 1.0, 0.0, "counts");

    // Close the A2L file
    A2lCreate_MOD_PAR("EPK_xxxx");
    A2lCreate_ETH_IF_DATA(gOptionUseTCP, gOptionBindAddr, gOptionPort);
    A2lClose();

    for (;;) {
        sleepMs(100);

        // Lock the calibration parameter segment for consistent and thread safe access
        struct params_t *params = (struct params_t *)XcpLockCalSeg(calseg);

        // Local variable
        counter++;
        if (counter > params->counter_max) {
            counter = 0;
        }

        // Unlock the calibration segment
        XcpUnlockCalSeg(calseg);

        // Global variable
        counter_global = counter;

        // Trigger measurement events
        XcpEventExt(event, (void *)&event); // For local variables
        XcpEvent(event_global);             // For global variables

        // Check server status
        if (!XcpEthServerStatus()) {
            printf("\nXCP Server failed\n");
            break;
        }
    }

    // Stop the XCP server
    XcpEthServerShutdown();
}
