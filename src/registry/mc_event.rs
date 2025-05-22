// Module mc_event
// Types:
//  McTypeDef, McTypeDefField

use std::borrow::Cow;

use serde::Deserialize;
use serde::Serialize;

use crate::registry::Registry;
use crate::registry::RegistryError;

use super::McIdentifier;

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
        }
    }

    /// Get the event name
    pub fn get_name(&self) -> &'static str {
        self.name.as_str()
    }

    /// Get the full indexed name of the event
    /// The event name may not be unique, events with the same name may be created by multiple thread instances of a task, this is indicated by index > 0
    pub fn get_unique_name(&self, registry: &Registry) -> Cow<'static, str> {
        if self.index > 0 {
            if registry.prefix_names {
                Cow::Owned(format!("{}.{}_{}", registry.get_app_name(), self.name, self.index))
            } else {
                Cow::Owned(format!("{}_{}", self.name, self.index))
            }
        } else {
            if registry.prefix_names {
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
