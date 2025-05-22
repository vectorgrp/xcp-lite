pub mod prelude;

use std::vec::IntoIter;

pub trait XcpTypeDescription {
    fn type_description(&self, flat: bool) -> Option<StructDescriptor> {
        let _ = flat;
        None
    }
}

/// FieldDescriptor contains properties and attributes for a struct field
#[derive(Debug)]
pub struct FieldDescriptor {
    // Datatype
    name: &'static str,                          // Identifier of the field
    struct_descriptor: Option<StructDescriptor>, // Inner StructDescriptor
    value_type: &'static str,                    // u8, u16, u32, u64, i8, i16, i32, i74, f32, f64, bool, InnerStruct, [InnerStruct; x_dim], [[InnerStruct; x_dim]; _dim]

    // Attributes
    classifier: &'static str, // "axis", "characteristic", "measurement" or empty ""
    qualifier: &'static str,  //"volatile", "readonly"
    comment: &'static str,
    min: Option<f64>,
    max: Option<f64>,
    step: Option<f64>,
    factor: f64,
    offset: f64,
    unit: &'static str,
    x_dim: u16,
    y_dim: u16,
    x_axis_ref: &'static str,
    y_axis_ref: &'static str,
    addr_offset: u16,
}

impl FieldDescriptor {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        name: &'static str,
        struct_descriptor: Option<StructDescriptor>,
        value_type: &'static str,
        classifier: &'static str, // "axis", "characteristic", "measurement" or empty ""
        qualifier: &'static str,  //"volatile", "readonly"
        comment: &'static str,
        min: Option<f64>,
        max: Option<f64>,
        step: Option<f64>,
        factor: f64,
        offset: f64,
        unit: &'static str,
        x_dim: u16,
        y_dim: u16,
        x_axis_ref: &'static str,
        y_axis_ref: &'static str,
        addr_offset: u16,
    ) -> Self {
        FieldDescriptor {
            name,
            struct_descriptor,
            value_type,
            classifier,
            qualifier,
            comment,
            min,
            max,
            step,
            factor,
            offset,
            unit,
            x_dim,
            y_dim,
            x_axis_ref,
            y_axis_ref,
            addr_offset,
        }
    }

    pub fn name(&self) -> &'static str {
        self.name
    }
    pub fn struct_descriptor(&self) -> Option<&StructDescriptor> {
        self.struct_descriptor.as_ref()
    }

    pub fn value_type(&self) -> &'static str {
        self.value_type
    }

    pub fn comment(&self) -> &'static str {
        self.comment
    }

    pub fn min(&self) -> Option<f64> {
        self.min
    }
    pub fn max(&self) -> Option<f64> {
        self.max
    }
    pub fn step(&self) -> Option<f64> {
        self.step
    }
    pub fn factor(&self) -> f64 {
        self.factor
    }
    pub fn offset(&self) -> f64 {
        self.offset
    }
    pub fn unit(&self) -> &'static str {
        self.unit
    }

    pub fn x_dim(&self) -> u16 {
        if self.x_dim == 0 { 1 } else { self.x_dim }
    }
    pub fn y_dim(&self) -> u16 {
        if self.y_dim == 0 { 1 } else { self.y_dim }
    }

    pub fn x_axis_ref(&self) -> Option<&'static str> {
        if self.is_axis() || self.x_axis_ref.is_empty() { None } else { Some(self.x_axis_ref) }
    }
    pub fn y_axis_ref(&self) -> Option<&'static str> {
        if self.is_axis() || self.y_axis_ref.is_empty() { None } else { Some(self.y_axis_ref) }
    }

    pub fn is_measurement(&self) -> bool {
        self.classifier == "measurement"
    }
    pub fn is_characteristic(&self) -> bool {
        self.classifier == "characteristic"
    }
    pub fn is_axis(&self) -> bool {
        self.classifier == "axis"
    }
    pub fn is_volatile(&self) -> bool {
        self.qualifier == "volatile"
    }
    pub fn is_readonly(&self) -> bool {
        self.qualifier == "readonly"
    }
    pub fn addr_offset(&self) -> u16 {
        self.addr_offset
    }

    pub fn set_addr_offset(&mut self, offset: u16) {
        self.addr_offset = offset;
    }

    pub fn set_name(&mut self, name: &'static str) {
        self.name = name;
    }
}

// The XcpTypeDescription trait implementation for Rust primitives is
// simply a blanket (empty) trait implementation. This macro is used
// to automatically generate the implementation for Rust primitives
macro_rules! impl_xcp_type_description_for_primitive {
    ($($t:ty),*) => {
        $(
            impl XcpTypeDescription for $t {}
        )*
    };
}

impl_xcp_type_description_for_primitive!(u8, u16, u32, u64, usize, i8, i16, i32, i64, isize, f32, f64, bool, char);

// The implementation of the XcpTypeDescription trait for
// arrays is also a blanket (empty) trait implementation
impl<T, const N: usize> XcpTypeDescription for [T; N] {}

/// StructDescriptor is a vec of FieldDescriptor
/// It it created with the XcpTypeDescription proc-macro trait
#[derive(Debug, Default)]
pub struct StructDescriptor {
    name: &'static str,
    size: usize,
    fields: Vec<FieldDescriptor>,
}

impl StructDescriptor {
    pub fn new(name: &'static str, size: usize) -> Self {
        StructDescriptor { name, size, fields: Vec::new() }
    }

    pub fn name(&self) -> &'static str {
        self.name
    }

    pub fn size(&self) -> usize {
        self.size
    }

    pub fn push(&mut self, field_descriptor: FieldDescriptor) {
        self.fields.push(field_descriptor);
    }

    pub fn sort(&mut self) {
        self.fields.sort_by(|a, b| a.name.cmp(b.name));
    }

    pub fn iter(&self) -> std::slice::Iter<FieldDescriptor> {
        self.fields.iter()
    }
}

impl IntoIterator for StructDescriptor {
    type Item = FieldDescriptor;
    type IntoIter = IntoIter<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        self.fields.into_iter()
    }
}

impl Extend<FieldDescriptor> for StructDescriptor {
    fn extend<T: IntoIterator<Item = FieldDescriptor>>(&mut self, iter: T) {
        self.fields.extend(iter);
    }
}
