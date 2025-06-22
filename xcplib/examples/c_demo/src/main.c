// c_demo xcplib example

#include <assert.h>  // for assert
#include <stdbool.h> // for bool
#include <stdint.h>  // for uintxx_t
#include <stdio.h>   // for printf
#include <string.h>  // for sprintf

#include "a2l.h"          // for A2l generation
#include "platform.h"     // for sleepMs, clockGet
#include "xcpEthServer.h" // for XcpEthServerInit, XcpEthServerShutdown, XcpEthServerStatus
#include "xcpLite.h"      // for XcpInit, XcpEventXxx, XcpCreateEvent, XcpCreateCalSeg, DaqXxxx, ...

//-----------------------------------------------------------------------------------------------------

// XCP parameters
#define OPTION_ENABLE_A2L_GENERATOR       // Enable A2L file generation
#define OPTION_A2L_PROJECT_NAME "C_Demo"  // A2L project name
#define OPTION_A2L_FILE_NAME "C_Demo.a2l" // A2L file name
#define OPTION_USE_TCP false              // TCP or UDP
#define OPTION_SERVER_PORT 5555           // Port
#define OPTION_SERVER_ADDR {0, 0, 0, 0}   // Bind addr, 0.0.0.0 = ANY
// #define OPTION_SERVER_ADDR {127, 0, 0, 1} // Bind addr, 0.0.0.0 = ANY
#define OPTION_QUEUE_SIZE 1024 * 32 // Size of the measurement queue in bytes, must be a multiple of 8
#define OPTION_LOG_LEVEL 3

//-----------------------------------------------------------------------------------------------------

// Demo calibration parameters
typedef struct params {
    uint16_t counter_max; // Maximum value for the counters
    uint32_t delay_us;    // Delay in microseconds for the main loop
    int8_t test_byte1;
    int8_t test_byte2;
    int8_t curve[8];
    int8_t map[8][8];
} params_t;

const params_t params = {.counter_max = 1000, .delay_us = 1000, .test_byte1 = -1, .test_byte2 = 1, .curve = {0, 1, 2, 3, 4, 5, 6, 7}, .map = {{0}}};

//-----------------------------------------------------------------------------------------------------

// Demo main
int main(void) {

    printf("\nXCP on Ethernet C xcplib demo\n");

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
    A2lCreateParameterWithLimits(params, counter_max, "maximum counter value", "", 0, 2000);
    A2lCreateParameterWithLimits(params, delay_us, "mainloop delay time in us", "us", 0, 1000000);
    A2lCreateParameter(params, test_byte1, "", "");
    A2lCreateParameter(params, test_byte2, "", "");
    A2lCreateCurve(params, curve, 8, "", "");
    A2lCreateMap(params, map, 8, 8, "", "");

    // Variables on stack
    uint8_t counter8 = 0;
    uint16_t counter16 = 0;
    uint32_t counter32 = 0;
    uint64_t counter64 = 0;
    int8_t counter8s = 0;
    int16_t counter16s = 0;
    int32_t counter32s = 0;
    int64_t counter64s = 0;

    // Create a measurement event for local variables
    DaqCreateEvent(mainloop);

    // Register measurement variables located on stack
    A2lSetStackAddrMode(mainloop);
    A2lCreatePhysMeasurement(counter8, "Measurement variable", 1.0, 0.0, "");
    A2lCreatePhysMeasurement(counter16, "Measurement variable", 1.0, 0.0, "");
    A2lCreatePhysMeasurement(counter32, "Measurement variable", 1.0, 0.0, "");
    A2lCreatePhysMeasurement(counter64, "Measurement variable", 1.0, 0.0, "");
    A2lCreatePhysMeasurement(counter8s, "Measurement variable", 1.0, 0.0, "");
    A2lCreatePhysMeasurement(counter16s, "Measurement variable", 1.0, 0.0, "");
    A2lCreatePhysMeasurement(counter32s, "Measurement variable", 1.0, 0.0, "");
    A2lCreatePhysMeasurement(counter64s, "Measurement variable", 1.0, 0.0, "");

    // Multidimensional measurements on stack
    float curve_f32[8] = {000, 100, 200, 300, 400, 500, 600, 700};
    float map_f32[4][8] = {
        {0, 100, 200, 300, 400, 500, 600, 700},
        {0, 200, 300, 400, 500, 600, 700, 800},
        {0, 300, 400, 500, 600, 700, 800, 900},
        {0, 400, 500, 600, 700, 800, 900, 1000},

    };

    A2lCreateMeasurementArray(curve_f32, "array float[8]");
    A2lCreateMeasurementMatrix(map_f32, "matrix float[4][8]");

    // Create a measurement typedef for the calibration parameter struct
    A2lTypedefBegin(params_t, "The calibration parameter struct as measurement typedef");
    A2lTypedefMeasurementComponent(test_byte1, params_t);
    A2lTypedefMeasurementComponent(test_byte2, params_t);
    A2lTypedefMeasurementComponent(counter_max, params_t);
    A2lTypedefMeasurementComponent(delay_us, params_t);
    A2lTypedefEnd();

    // Demo
    // Create a static measurement variable which is a copy of the calibration parameter segment to verify calibration changes and consistency
    static params_t params_copy;
    A2lSetAbsoluteAddrMode(mainloop);
    A2lCreateTypedefInstance(params_copy, params_t, "A copy of the current calibration parameters");

    for (;;) {
        // Lock the calibration parameter segment for consistent and safe access
        // Calibration segment locking is completely lock-free and wait-free (no mutexes, system calls or CAS operations )
        // It returns a pointer to the active page (working or reference) of the calibration segment
        params_t *params = (params_t *)XcpLockCalSeg(calseg);

        // Sleep for the specified delay parameter in microseconds
        sleepNs(params->delay_us * 1000);

        // Local variables for measurement
        counter16++;
        if (counter16 > params->counter_max) {
            counter16 = 0;

            for (int i = 0; i < 8; i++) {
                curve_f32[i] += i;
                if (curve_f32[i] > 2000) {
                    curve_f32[i] = 0;
                }
                for (int j = 0; j < 4; j++) {
                    map_f32[j][i] += i + j;
                    if (map_f32[j][i] > 2000) {
                        map_f32[j][i] = 0;
                    }
                }
            }
        }
        counter8 = (uint8_t)(counter16 & 0xFF);
        counter32 = (uint32_t)counter16;
        counter64 = (uint64_t)counter16;
        counter8s = (int8_t)counter8;
        counter16s = (int16_t)counter16;
        counter32s = (int32_t)counter32;
        counter64s = (int64_t)counter64;

        // Calibration demo
        // Visualizes calibration consistency and page switching
        // Copies the current calibration page to a static measurement variable
        // Insert params.test_byte1 and params.test_byte2 into a CANape calibration window, enable indirect calibration
        // Use the update button in the calibration window to trigger consistent modifications, the message below should never appear
        // There should be also no message when switching from RAM ro FLASH
        params_copy = *params;
        if (params_copy.test_byte1 != -params_copy.test_byte2) {
            char buffer[64];
            SNPRINTF(buffer, sizeof(buffer), "Inconsistent %u:  %d -  %d", counter16, params_copy.test_byte1, params_copy.test_byte2);
            XcpPrint(buffer);
            printf("%s\n", buffer);
        }

        // Unlock the calibration segment
        XcpUnlockCalSeg(calseg);

        // Trigger the measurement event
        DaqEvent(mainloop);

        // Check server status
        if (!XcpEthServerStatus()) {
            printf("\nXCP Server failed\n");
            break;
        }

        A2lFinalize(); // Optional: Finalize the A2L file generation early, to write the A2L now, not when the client connects

    } // for (;;)

    // Force disconnect the XCP client
    XcpDisconnect();

    // Stop the XCP server
    XcpEthServerShutdown();

    return 0;
}
