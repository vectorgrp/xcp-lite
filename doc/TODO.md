# TODO list

# Refactor event management to use XCPlite

For calibration segments, the non deterministc creation problem solved by linkme is minor.
We will use it for event creation as well, with much larger benefit.
The events have a pretty complex machinery to map to name order.
I want to switch event managent over to XCPlite and get rid of this complexity. The event creation will be deterministic and race free, and the event name order will be stable across runs, independent of creation order or threads.


# Refactor the IDL generation derive macro 

Adjust to the stype the new type registration macro has
Support multi dimension type without limitations
Add more supported container types (Vec, HashMap, BTreeMap, etc.)

