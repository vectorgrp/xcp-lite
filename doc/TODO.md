# TODO list

# Refactor event management to use XCPlite

For calibration segments, the non deterministc creation problem solved by linkme is minor.
We will use it for event creation as well, with much larger benefit.
The events have a pretty complex machinery to map to name order.
I want to switch event managent over to XCPlite and get rid of this complexity. The event creation will be deterministic and race free, and the event name order will be stable across runs, independent of creation order or threads.

## Why it makes sense

Today Rust keeps a parallel `EventList` (src/xcp/mod.rs) whose only real job is to make the A2L
event numbers deterministic: `create_event_ext` allocates an id by `self.0.len()` (call order),
then `register()` does `sort_by_name_and_index()` plus a remap. All of this exists only because
C's `XcpCreateEvent` returns ids in call order.

XCPlite now owns a real event list (`XCP_ENABLE_DAQ_EVENT_LIST`, `tXcpEventList`) and exposes
everything needed to drop the Rust-side bookkeeping:
- `XcpCreateEvent(name, cycle_time_ns, priority)` / `XcpCreateEventInstance(...)` (the latter is
  thread-safe and appends the instance index for duplicate names).
- `XcpFindEvent`, `XcpGetEventCount`, `XcpGetEvent`, `XcpGetEventName`, `XcpGetEventIndex`.

So C becomes the single source of truth, and the A2L generator queries it back instead of
mirroring it. That removes the sort/remap complexity.

## The wrinkle: events are partly dynamic

Cal segs are fully static, so link-time collection (linkme) enumerates all of them. Events are
NOT all static: "indexed" events are per-thread instances created at runtime when a thread
spawns. A pure link-time / linkme approach cannot enumerate instances whose count is only known
at runtime. => use a hybrid.

## Plan (hybrid)

- Single-instance, statically-declared events -> a `daq_event!`-style macro using the same linkme
  distributed-slice + name-sorted first-use creation as `cal_seg!`. Fully deterministic A2L numbers.
- Per-thread instance events -> keep a runtime creation path, but delegate to
  `XcpCreateEventInstance` and query the id/index back. Determinism here is "stable name + instance
  index" (C-guaranteed), not a stable global number - which matches current instance behaviour.
- Either way, Rust stops doing its own sort/remap; delete the parallel `EventList`.

## Caveats to plan for

- Lifetime/encoding bridge: `XcpGetEventName` returns a C-owned `const char*`; today `get_name()`
  returns `&'static str`. Need a safe wrapper. C caps names at `XCP_MAX_EVENT_NAME` (15) - confirm
  event names fit.
- A2L metadata: pull cycle time / priority / index from `XcpGetEvent`, not from the removed Rust struct.
- Trigger path is unaffected - it already uses C ids via `XcpEventExt`.
- Same linkme direct-dependency caveat applies to any crate using the new event macro (like `cal_seg!`).
- Ordering vs init: events must be created after `Xcp::init`, same as the cal seg registry.


# Refactor the IDL generation derive macro 

Adjust to the stype the new type registration macro has
Support multi dimension type without limitations
Add more supported container types (Vec, HashMap, BTreeMap, etc.)

