// Module mc_event
// Types:
//  McTypeDef, McTypeDefField

use std::borrow::Cow;

use serde::Deserialize;
use serde::Serialize;

use crate::Registry;
use crate::RegistryError;

use super::McIdentifier;
use super::McText;

//----------------------------------------------------------------------------------------------
// McEvent

/// An event which may trigger consistent data acqisition
/// Events have unique id called id number
/// The name is not unique, events with the same name may be created by multiple thread instances of a task, this is indicated by index > 0
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct McEvent {
    pub name: McIdentifier,        // Name of the event, not unique name
    pub index: u16,                // Instance index 1..n, 0 if single instance
    pub id: u16,                   // Unique event id number used in A2L and XCP protocol, unique event identifier for an application
    pub target_cycle_time_ns: u32, // 0 -> no cycle time = sporadic event
    pub function: Option<McText>,  // Name of the function where the event is defined, used to find local variables for this event
    pub unit: Option<usize>,       // Index of the compilation unit where the event is defined, used to find local variables for this event
    pub cfa: i32,                  // Canonical stack frame address offset where the event is defined, used to access local variables for this event
}

impl McEvent {
    /// Create a new event with name, instance index and cycle time in ns
    pub fn new<T: Into<McIdentifier>>(name: T, index: u16, id: u16, target_cycle_time_ns: u32) -> Self {
        let name: McIdentifier = name.into();
        McEvent {
            name,
            index,
            id,
            target_cycle_time_ns,
            function: None,
            unit: None,
            cfa: 0,
        }
    }

    /// Get the event name
    pub fn get_name(&self) -> &'static str {
        self.name.as_str()
    }

    /// Get the event id
    /// This is the unique identifier for the event
    pub fn get_id(&self) -> u16 {
        self.id
    }

    // Set the event id
    // For internal use only
    // The event id must be unique, no checks
    pub fn set_id(&mut self, id: u16) {
        self.id = id;
    }

    /// Get the full indexed name of the event
    /// The event name may not be unique, events with the same name may be created by multiple thread instances of a task, this is indicated by index > 0
    pub fn get_unique_name(&self, registry: &Registry) -> Cow<'static, str> {
        if self.index > 0 {
            if registry.get_prefix_names_mode() {
                Cow::Owned(format!("{}.{}_{}", registry.application.get_name(), self.name, self.index))
            } else {
                Cow::Owned(format!("{}_{}", self.name, self.index))
            }
        } else {
            if registry.get_prefix_names_mode() {
                Cow::Owned(format!("{}_{}", self.name, self.index))
            } else {
                Cow::Borrowed(self.name.as_str())
            }
        }
    }
}

//----------------------------------------------------------------------------------------------
// McEventList

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct McEventList(Vec<McEvent>);

impl McEventList {
    pub fn new() -> Self {
        McEventList(Vec::with_capacity(100))
    }

    /// Add an XCP event with name, index and cycle time in ns
    pub fn add_event(&mut self, event: McEvent) -> Result<(), RegistryError> {
        log::debug!("Registry add_event: {:?} ", event);

        // Error if event with same name and index already exists
        if self.find_event(&event.name, event.index).is_some() {
            return Err(RegistryError::Duplicate(event.name.to_string()));
        }
        // Error if event with same unique id (id) already exists
        if self.find_event_id(event.id).is_some() {
            return Err(RegistryError::Duplicate(event.name.to_string()));
        }

        self.push(event);
        Ok(())
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
    pub fn push(&mut self, object: McEvent) {
        self.0.push(object);
    }

    pub fn sort_by_name(&mut self) {
        self.0.sort_by(|a, b| a.name.cmp(&b.name));
    }

    pub fn sort_by_id(&mut self) {
        self.0.sort_by(|a, b| a.id.cmp(&b.id));
    }

    /// Find an event by name
    pub fn find_event(&self, name: &str, index: u16) -> Option<&McEvent> {
        self.0.iter().find(|e| e.index == index && e.name == name)
    }

    /// find an event by id
    pub fn find_event_id(&self, id: u16) -> Option<&McEvent> {
        self.0.iter().find(|e| e.id == id)
    }

    /// Find an event by unit index and function name
    /// This is used to find local variables for this event
    /// If multiple events are defined in the same function, the first event is returned
    pub fn find_event_by_location(&self, unit_idx: usize, function: &str) -> Option<&McEvent> {
        self.0.iter().find(|e| e.unit == Some(unit_idx) && e.function.as_deref() == Some(function))
    }

    /// Store the unit index and function name where the event is defined
    /// This is used to find local variables for this event
    /// Multiple events may be defined in the same function
    pub fn set_event_location(&mut self, name: &str, unit_idx: usize, function: &str, cfa: i32) -> Result<(), RegistryError> {
        if let Some(event) = self.0.iter_mut().find(|e| e.name == name) {
            event.unit = Some(unit_idx);
            event.function = Some(function.to_string().into());
            event.cfa = cfa;
            Ok(())
        } else {
            Err(RegistryError::NotFound(name.to_string()))
        }
    }
}

//-------------------------------------------------------------------------------------------------
// EventListIterator

/// Iterator for EventList
pub struct McEventListIterator<'a> {
    index: usize,
    list: &'a McEventList,
}

impl<'a> McEventListIterator<'_> {
    pub fn new(list: &'a McEventList) -> McEventListIterator<'a> {
        McEventListIterator { index: 0, list }
    }
}

impl<'a> Iterator for McEventListIterator<'a> {
    type Item = &'a McEvent;

    fn next(&mut self) -> Option<Self::Item> {
        let index = self.index;
        if index < self.list.0.len() {
            self.index += 1;
            Some(&self.list.0[index])
        } else {
            None
        }
    }
}

impl<'a> IntoIterator for &'a McEventList {
    type Item = &'a McEvent;
    type IntoIter = McEventListIterator<'a>;

    fn into_iter(self) -> McEventListIterator<'a> {
        McEventListIterator::new(self)
    }
}

//-------------------------------------------------------------------------------------------------
// McEventListIteratorMut (Mutable Iterator)

/// Mutable iterator for EventList
pub struct McEventListIteratorMut<'a> {
    iter: std::slice::IterMut<'a, McEvent>,
}

impl<'a> McEventListIteratorMut<'a> {
    pub fn new(list: &'a mut McEventList) -> McEventListIteratorMut<'a> {
        McEventListIteratorMut { iter: list.0.iter_mut() }
    }
}

impl<'a> Iterator for McEventListIteratorMut<'a> {
    type Item = &'a mut McEvent;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next()
    }
}

impl<'a> IntoIterator for &'a mut McEventList {
    type Item = &'a mut McEvent;
    type IntoIter = McEventListIteratorMut<'a>;

    fn into_iter(self) -> McEventListIteratorMut<'a> {
        McEventListIteratorMut::new(self)
    }
}
