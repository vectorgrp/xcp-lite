// Module mc_typedef
// Types:
//  McTypeDef, McTypeDefField

use serde::Deserialize;
use serde::Serialize;

use super::McDimType;
use super::McIdentifier;
use super::McSupportData;
use super::McValueType;
use super::RegistryError;

//-------------------------------------------------------------------------------------------------
// McTypeDef

// Type definition for McValueType::TypeDef(type_name)
#[derive(Debug, Serialize, Deserialize)]
pub struct McTypeDef {
    pub name: McIdentifier,
    pub fields: McTypeDefFieldList, // Fields of the struct type_name
    pub size: usize,                // Size of the struct type_name in bytes
}

impl McTypeDef {
    pub fn new<T: Into<McIdentifier>>(type_name: T, size: usize) -> McTypeDef {
        let type_name: McIdentifier = type_name.into();
        McTypeDef {
            name: type_name,
            fields: McTypeDefFieldList::new(),
            size,
        }
    }

    pub fn get_name(&self) -> &'static str {
        self.name.as_str()
    }

    pub fn find_field(&self, name: &str) -> Option<&McTypeDefField> {
        self.fields.into_iter().find(|field| field.name == name)
    }

    pub fn add_field<T: Into<McIdentifier>>(&mut self, name: T, dim_type: McDimType, mc_support_data: McSupportData, offset: u16) -> Result<(), RegistryError> {
        let name: McIdentifier = name.into();

        // Error if duplicate field name
        if self.find_field(name.as_str()).is_some() {
            return Err(RegistryError::Duplicate(name.to_string()));
        }

        // Add field
        self.fields.push(McTypeDefField::new(name, dim_type, mc_support_data, offset));
        Ok(())
    }
}

//----------------------------------------------------------------------------------------------
// McTypeDefFieldList

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct McTypeDefList(Vec<McTypeDef>);

impl McTypeDefList {
    pub fn new() -> Self {
        McTypeDefList(Vec::with_capacity(16))
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }
    pub fn get(&self, index: usize) -> Option<&McTypeDef> {
        self.0.get(index)
    }
    pub fn get_mut(&mut self, index: usize) -> &mut McTypeDef {
        &mut self.0[index]
    }
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
    pub fn push(&mut self, object: McTypeDef) {
        self.0.push(object);
    }
    pub fn clear(&mut self) {
        self.0.clear();
    }
    pub fn find_typedef_mut(&mut self, name: &str) -> Option<&mut McTypeDef> {
        self.0.iter_mut().find(|i| i.name == name)
    }
    pub fn find_typedef(&self, name: &str) -> Option<&McTypeDef> {
        self.0.iter().find(|i| i.name == name)
    }

    pub fn sort_by_name(&mut self) {
        self.0.sort_by(|a, b| a.name.cmp(&b.name));
    }
}

//-------------------------------------------------------------------------------------------------
// TypeDefListIterator

/// Iterator for TypeDefList
pub struct McTypeDefListIterator<'a> {
    index: usize,
    list: &'a McTypeDefList,
}

impl<'a> McTypeDefListIterator<'_> {
    pub fn new(list: &'a McTypeDefList) -> McTypeDefListIterator<'a> {
        McTypeDefListIterator { index: 0, list }
    }
}

impl<'a> Iterator for McTypeDefListIterator<'a> {
    type Item = &'a McTypeDef;

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

impl<'a> IntoIterator for &'a McTypeDefList {
    type Item = &'a McTypeDef;
    type IntoIter = McTypeDefListIterator<'a>;

    fn into_iter(self) -> McTypeDefListIterator<'a> {
        McTypeDefListIterator::new(self)
    }
}

// Just pass iter_mut up
impl McTypeDefList {
    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut McTypeDef> {
        self.0.iter_mut()
    }
}

//-------------------------------------------------------------------------------------------------
// McTypeDefField

#[derive(Debug, Serialize, Deserialize)]
pub struct McTypeDefField {
    pub name: McIdentifier,
    pub dim_type: McDimType,            // Type name and matrix dimensions, recursion here if McValueType::TypeDef
    pub mc_support_data: McSupportData, // Metadata for the field
    pub offset: u16,                    // Offset of the field in the struct ABI
}

impl McTypeDefField {
    pub fn new<T: Into<McIdentifier>>(field_name: T, dim_type: McDimType, mc_support_data: McSupportData, offset: u16) -> McTypeDefField {
        McTypeDefField {
            name: field_name.into(),
            dim_type,
            mc_support_data,
            offset,
        }
    }

    pub fn get_name(&self) -> &'static str {
        self.name.as_str()
    }

    /// Check if the value type is a typedef and return the typedef name if it is
    pub fn get_typedef_name(&self) -> Option<&'static str> {
        match self.dim_type.value_type {
            McValueType::TypeDef(typedef_name) => Some(typedef_name.as_str()),
            _ => None,
        }
    }

    /// Get the offset of the field in the struct ABI
    pub fn get_offset(&self) -> u16 {
        self.offset
    }

    /// Get type
    pub fn get_dim_type(&self) -> &McDimType {
        &self.dim_type
    }

    /// Get metadata
    pub fn get_mc_support_data(&self) -> &McSupportData {
        &self.mc_support_data
    }
}

//----------------------------------------------------------------------------------------------
// McTypeDefFieldList

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct McTypeDefFieldList(Vec<McTypeDefField>);

impl McTypeDefFieldList {
    pub fn new() -> Self {
        McTypeDefFieldList(Vec::with_capacity(8))
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
    pub fn push(&mut self, object: McTypeDefField) {
        self.0.push(object);
    }

    pub fn find_typedef_field(&self, name: &str) -> Option<&McTypeDefField> {
        self.0.iter().find(|i| i.name == name)
    }
}

//-------------------------------------------------------------------------------------------------
// TypeDefFieldListIterator

/// Iterator for TypeDefFieldList
pub struct McTypeDefFieldListIterator<'a> {
    index: usize,
    list: &'a McTypeDefFieldList,
}

impl<'a> McTypeDefFieldListIterator<'_> {
    pub fn new(list: &'a McTypeDefFieldList) -> McTypeDefFieldListIterator<'a> {
        McTypeDefFieldListIterator { index: 0, list }
    }
}

impl<'a> Iterator for McTypeDefFieldListIterator<'a> {
    type Item = &'a McTypeDefField;

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

impl<'a> IntoIterator for &'a McTypeDefFieldList {
    type Item = &'a McTypeDefField;
    type IntoIter = McTypeDefFieldListIterator<'a>;

    fn into_iter(self) -> McTypeDefFieldListIterator<'a> {
        McTypeDefFieldListIterator::new(self)
    }
}

// Just pass iter_mut up
impl McTypeDefFieldList {
    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut McTypeDefField> {
        self.0.iter_mut()
    }
}
