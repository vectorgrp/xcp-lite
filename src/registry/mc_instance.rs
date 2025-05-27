// Module mc_instance
// Types:
//  McInstance, McInstanceList, McInstanceListIterator

use std::borrow::Cow;

use regex::Regex;

use super::McAddress;
use super::McDimType;
use super::McIdentifier;
use super::McObjectType;
use super::McSupportData;
use super::McValueType;
use super::Registry;
use super::RegistryError;
use serde::Deserialize;
use serde::Serialize;

//-------------------------------------------------------------------------------------------------
// Instance
// Measurement and calibration object instances

/// Instance which may be a parameter, axis, or measurement value of complex type
#[derive(Debug, Serialize, Deserialize)]
pub struct McInstance {
    pub name: McIdentifier,
    pub dim_type: McDimType, // Type, metadata and matrix dimensions, recursion here if McValueType::TypeDef
    pub address: McAddress,  // Addressing information for the instance
}

impl McInstance {
    pub fn new<T: Into<McIdentifier>>(name: T, dim_type: McDimType, address: McAddress) -> McInstance {
        McInstance {
            name: name.into(),
            dim_type,
            address,
        }
    }

    pub fn get_dim_type(&self) -> &McDimType {
        &self.dim_type
    }

    pub fn get_mc_support_data(&self) -> Option<&McSupportData> {
        self.dim_type.get_mc_support_data()
    }

    pub fn get_address(&self) -> &McAddress {
        &self.address
    }

    /// Get the instance name
    /// The instance name may not be unique
    pub fn get_name(&self) -> &'static str {
        self.name.as_str()
    }

    /// Get the instance name with optional application name prefix
    /// The instance name may not be unique
    pub fn get_prefixed_name(&self, registry: &Registry) -> Cow<'static, str> {
        if registry.prefix_names {
            return Cow::Owned(format!("{}.{}", registry.get_app_name(), self.name));
        }
        Cow::Borrowed(self.name.as_str())
    }

    /// Get the unique instance name with optional application name prefix and postfixed by instance index
    /// If there is an event event index > 0, which means there are multiple instances of the same name
    pub fn get_unique_name(&self, registry: &Registry) -> Cow<'static, str> {
        if let Some(event_id) = self.address.event_id() {
            if let Some(event) = registry.event_list.find_event_id(event_id) {
                if event.index > 0 {
                    if registry.prefix_names {
                        return Cow::Owned(format!(r#"{}.{}_{}"#, registry.get_app_name(), self.name, event.index));
                    } else {
                        return Cow::Owned(format!(r#"{}_{}"#, self.name, event.index));
                    }
                }
            }
        }
        if registry.prefix_names {
            Cow::Owned(format!(r#"{}.{}"#, registry.get_app_name(), self.name))
        } else {
            Cow::Borrowed(self.name.as_str())
        }
    }

    /// Get the instance event index
    /// If there is an event event index > 0, which means there are multiple instances of the same name
    pub fn get_index(&self, registry: &Registry) -> u16 {
        if let Some(event_id) = self.address.event_id() {
            if let Some(event) = registry.event_list.find_event_id(event_id) {
                if event.index > 0 {
                    return event.index;
                }
            }
        }
        0
    }

    // Shortcuts to dim_type
    pub fn size(&self) -> usize {
        self.dim_type.get_size()
    }
    pub fn value_size(&self) -> usize {
        self.dim_type.value_type.get_size()
    }
    pub fn value_type(&self) -> &McValueType {
        &self.dim_type.value_type
    }
    // Check if the value type is a typedef and return the typedef name if it is
    pub fn get_typedef_name(&self) -> Option<&'static str> {
        match self.dim_type.value_type {
            McValueType::TypeDef(typedef_name) => Some(typedef_name.as_str()),
            _ => None,
        }
    }

    // Shortcuts to mc_support_data
    pub fn object_type(&self) -> McObjectType {
        self.dim_type.get_object_type()
    }
    pub fn is_calibration_object(&self) -> bool {
        self.dim_type.is_calibration_object()
    }
    pub fn is_measurement_object(&self) -> bool {
        self.dim_type.is_measurement_object()
    }
    pub fn is_axis(&self) -> bool {
        self.dim_type.is_axis()
    }
    pub fn is_characteristic(&self) -> bool {
        self.dim_type.is_characteristic()
    }
    pub fn x_dim(&self) -> u16 {
        self.dim_type.get_dim()[0]
    }
    pub fn y_dim(&self) -> u16 {
        self.dim_type.get_dim()[1]
    }
    pub fn unit(&self) -> &'static str {
        self.dim_type.get_unit()
    }
    pub fn comment(&self) -> &'static str {
        self.dim_type.get_comment()
    }
    pub fn x_axis_ref(&self) -> Option<McIdentifier> {
        self.dim_type.get_x_axis_ref()
    }
    pub fn y_axis_ref(&self) -> Option<McIdentifier> {
        self.dim_type.get_y_axis_ref()
    }
    pub fn x_axis_conv(&self) -> Option<McIdentifier> {
        self.dim_type.get_x_axis_conv()
    }
    pub fn y_axis_conv(&self) -> Option<McIdentifier> {
        self.dim_type.get_y_axis_conv()
    }
    pub fn get_min(&self) -> Option<f64> {
        self.dim_type.get_min()
    }
    pub fn get_max(&self) -> Option<f64> {
        self.dim_type.get_max()
    }

    // Shortcuts to address
    pub fn event_id(&self) -> Option<u16> {
        self.address.event_id()
    }
    pub fn addr_offset(&self) -> i32 {
        self.address.get_addr_offset()
    }
    pub fn calseg_name(&self) -> Option<McIdentifier> {
        self.address.calseg_name()
    }
}

//-------------------------------------------------------------------------------------------------
// InstanceList

#[derive(Debug, Serialize, Deserialize)]
pub struct McInstanceList(Vec<McInstance>);

impl Default for McInstanceList {
    fn default() -> Self {
        McInstanceList::new()
    }
}

impl McInstanceList {
    pub fn new() -> Self {
        McInstanceList(Vec::with_capacity(100))
    }

    pub fn push(&mut self, object: McInstance) {
        self.0.push(object);
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn append(&mut self, other: &mut Self) {
        self.0.append(other.0.as_mut());
    }

    pub fn sort_by_name(&mut self) {
        self.0.sort_by(|a, b| a.name.cmp(&b.name));
    }

    pub fn sort_by_name_and_event(&mut self) {
        self.0.sort_by(|a, b| {
            if a.name == b.name {
                a.address.get_event_id_unchecked().cmp(&b.address.get_event_id_unchecked())
            } else {
                a.name.cmp(&b.name)
            }
        });
    }

    //---------------------------------------------------------------------------------------------------------

    /// Add a measurement and calibration object instance
    /// # Results
    ///   Ok(()) if the instance was added successfully
    ///   Err(RegistryError::Duplicate) if the instance already exists in the registry
    /// # Panics
    ///   Panics if the object type is unspecified
    /// # Arguments
    ///   * `name` - Name of the instance
    ///   * `dim_type` - Type, metadata and array dimensions
    ///   * `address` - Addressing information for the instance
    #[allow(clippy::too_many_arguments)]
    pub fn add_instance<T: Into<McIdentifier>>(&mut self, name: T, dim_type: McDimType, address: McAddress) -> Result<(), RegistryError> {
        let name = name.into();

        log::debug!("Registry add_instance: {} dim_type={:?}  addr={}", name, dim_type, address);
        assert!(dim_type.get_object_type() != McObjectType::Unspecified, "Object type must be specified");

        // Error if duplicate in instance namespace (A2l characteristics, measurements, axis and instances)
        // Names may not be unique, when there is a unique event_id
        if self.into_iter().any(|i| i.get_address() == &address && i.name == name) {
            log::error!("Duplicate instance {}!", name);
            return Err(RegistryError::Duplicate(name.to_string()));
        }

        let c: McInstance = McInstance::new(name, dim_type, address);
        self.push(c);
        Ok(())
    }

    /// Find an instance by regular expression, optional by object type (set to Unspecified if any) or by event_id (set to None if any)
    /// Returns the first instance that matches the criteria or None
    pub fn find_instance(&self, regex: &str, object_type: McObjectType, event_id: Option<u16>) -> Option<&McInstance> {
        if let Ok(regex) = Regex::new(regex) {
            self.into_iter().find(|i| {
                (i.get_address().event_id() == event_id || event_id.is_none())
                    && ((object_type == McObjectType::Unspecified || i.object_type() == object_type) && regex.is_match(i.name.as_str()))
            })
        } else {
            None
        }
    }
    /// Find all instances by regular expression, optional by object type (set to Unspecified if any) or by event_id (set to None if any)
    /// Return all instances that match the criteria
    pub fn find_instances(&self, regex: &str, object_type: McObjectType, event_id: Option<u16>) -> Vec<String> {
        if let Ok(regex) = Regex::new(regex) {
            self.into_iter()
                .filter(|i| {
                    (i.get_address().event_id() == event_id || event_id.is_none())
                        && ((object_type == McObjectType::Unspecified || i.object_type() == object_type) && regex.is_match(i.name.as_str()))
                })
                .map(|i| i.name.to_string())
                .collect()
        } else {
            Vec::new()
        }
    }
}

//-------------------------------------------------------------------------------------------------
// InstanceListIterator

/// Iterator for InstanceList
pub struct McInstanceListIterator<'a> {
    index: usize,
    list: &'a McInstanceList,
}

impl<'a> McInstanceListIterator<'_> {
    pub fn new(list: &'a McInstanceList) -> McInstanceListIterator<'a> {
        McInstanceListIterator { index: 0, list }
    }
}

impl<'a> Iterator for McInstanceListIterator<'a> {
    type Item = &'a McInstance;

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

impl<'a> IntoIterator for &'a McInstanceList {
    type Item = &'a McInstance;
    type IntoIter = McInstanceListIterator<'a>;

    fn into_iter(self) -> McInstanceListIterator<'a> {
        McInstanceListIterator::new(self)
    }
}

// Just pass iter_mut up
impl McInstanceList {
    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut McInstance> {
        self.0.iter_mut()
    }
}
