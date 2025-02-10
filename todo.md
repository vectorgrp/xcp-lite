
# Possible improvements

- Create a lock free algorithm to acquire an entry in the mpsc event queue
- Support specialized types of calibration parameters, including types for curves and maps with axis
- Avoid the mutex lock in CalSeg::Sync when there is no pending parameter modification or switch to a mcu algorithm  
- Improve the meta data annotations of the A2L serializer
- Add support to describe the application clock domain

# Ideas

- Provide a no-std version and create an embassy example


# Suggested simplifications and optimizations for the XCP on ETH standard
- Support 64 Bit addresses in SHORT_UPLOAD, SHORT_DONWLOAD and SET_MTA
- Support more than (256-4)/(128-4) ODTs (ODT number as u16), avoid event based queue overload indication
- Add an option for 64 bit transport layer packet alignment
- Add an option to require a 1:1 association from event to daq list to simplify data structures and reduce event runtime
- Support variable length ODT entries for serialized data types
- Support segmented DTOs larger than segment size
- Add GET_ID type to upload (from ECU) binary schemas (.ZIP, .desc), referenced in A2L file
- Make GET_DAQ_CLOCK obsolete (when PTP TAI time is provided), by supporting 64 Bit DAQ timestamps
- Remove clock properties legacy mode



# Detailed current TODO list

## TODO xcp-daemon Repo:

Transport layer independant queue
XcpTlNotifyTransmitQueueHandler in QueuePush
if (clock == gXcpTl.queue_event_time) entfernen

Remove dead code in XCP_ENABLE_MULTITHREAD_DAQ_EVENTS and XCP_ENABLE_TIMESTAMP_CHECK


## TODO xcp-lite Repo:


Integrations test für TCP
Unit test für das Queue API




Queue Benchmarks: https://github1.vg.vector.int/rdm/mpmc Da befindet sich der queue.h Header drin und allgemeiner Benchmark code drin. Ich würde vorschlagen da dann die Queue Implementierungen nach diesem gemeinsamen Header und Benchmark zu messen. 


------------------------
https://google.github.io/styleguide/cppguide.html#Names_and_Order_of_Includes

https://google.github.io/styleguide/cppguide.html#Include_What_You_Use



------------------------


Liste der Anpassungen im xcp-daemon Repo inklusive stylistischer Anpassungen. 


Zu klären:
* #undef _WIN32 entfernt


Done:

* Abgeflachter Code durch early returns
	* Bsp. handleXcpCommand nesting kann man vereinfachen, siehe https://github1.vg.vector.int/rdm/xcp-lite/issues/21
* Portable printf strings
	* printf("I64", value) zu printf("%lli", (long long)value) 
* stdint.h und stdbool.h statt #define
* Externe symbole für .c Datei ausschließlich im assoziertem .h



Prio 1:
* Single threaded XCP on Ethernet server
Extraktion der Transport Layer Implementierung innerhalb des XCP on Ethernet Servers (Schichtenarchitektur)
	* TL Header sollte beim Lesen sofort enfternt und erst beim Senden so spät wie möglich konstruiert werden
	* => Reduziert Komplexität mMn. massiv, da Frame Packing und Counter zentral umgesetzt sind
* Getrennte Queues für DAQ Measurement und Commands + Events (XCP EV, nicht Measurment Events)
* Multi-Instanz Queue Implementierung mit opaque handles
	* queue handle als Parameter als Operationen
	* buffer handle beinhaltet alle Information, statt globalem tail_len, etc.
* 

Prio 2:

* Minimaler test.c unit Tests der Queue -> einfache Regressionstests in CI halte ich für wertvoll



Sonstige:
* pragma lib link gegen if in build.rs und Makefile ersetzt
	* Präprozessor sollte mMn. keine Compiler flags modfizieren
* Makefile um außerhalb von cargo zu compilen
* 


* Sortierungsunabhängige includes 
	* Begründung: https://google.github.io/styleguide/cppguide.html#Include_What_You_Use
	* Sortierung: https://google.github.io/styleguide/cppguide.html#Names_and_Order_of_Includes
* Minimierung externer Symbole
	* static Funktionen wenn möglich -> internal linkage
	* Typdeklaration in .c Datei, wenn Implementierungspezfisch, bsp. tQueueHandle und tQueue
* Variablen immer Initialisieren, niemals undefined lassen
	* auch bei out Parametern: "struct sockaddr_in client_address = {0};"
	* Ausnahme, nur bei signifikantem Overhead durch große Speicherbereiche
* Keine Magic Values, immer benannte Konstanten
	* auch für POSIX flags:
	```c
	  static int const kSendmsgFlags = 0;
    ssize_t const bytes_sent = sendmsg(socket_descriptor, &message_header, kSendmsgFlags);
	```
* Pedantische Compileflags
	* expliziter Standard: -std=c11
	* Warnings as errors: -Wall -Wextra -Wpedantic -Werror
* clang-format und clangd
	* siehe .clang-format und compile-flags.txt für Konfiguration
* keine Abkürzungen (nur sehr wenige Ausnahmen)
	* Bsp.: ctr -> counter, dlc -> data_length
	* mMn.: Abkürzungen erzeugen großen unnötigen mentalen Overhead, insbesondere wenn man noch nicht alle XCP Begrifft verinnerlicht hat
		XcpTriggerDaqList ist dadurch sehr schwer zu lesen, obwohl dort nur ein Baum traversiert wird, weil die Abkürzungen den Lesefluss unterbrechen
* #define Konstanten nur in Ausnahmefällen
	* stattdessen: static int const kWhatever = 123;
* "east const" -> "int const *abc", statt "const int *abc"
* mehr Einsatz von "const"
	* jedoch nicht bei Funktionsparametern


  --------------------------------


RDE und Mercedes das "VDP" Protokoll unter dem dann XCP verwendet wird. 
Ist das relevant? 
Wäre ggf. noch eine weitere Offboard Schnittstelle für den Daemon. 

https://github1.vg.vector.int/rde/daimler-swfw-vdp/blob/master/ideas/vdp-xcp.md#proposed-actions-for-collecting-xcp-data-with-vdp



----------------------------------



