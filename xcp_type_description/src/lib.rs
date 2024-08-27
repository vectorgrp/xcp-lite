pub mod prelude;

use std::vec::IntoIter;

pub trait XcpTypeDescription {
    fn type_description(&self) -> Option<StructDescriptor> {
        None
    }
}

/// FieldDescriptor contains properties and attributes for a struct field
#[derive(Debug)]
pub struct FieldDescriptor {
    name: String,
    datatype: &'static str,
    comment: &'static str,
    min: f64,
    max: f64,
    unit: &'static str,
    x_dim: usize,
    y_dim: usize,
    offset: u16,
}

impl FieldDescriptor {
    #[allow(clippy::too_many_arguments)]
    pub fn new(name: String, datatype: &'static str, comment: &'static str, min: f64, max: f64, unit: &'static str, x_dim: usize, y_dim: usize, offset: u16) -> Self {
        FieldDescriptor {
            name,
            datatype,
            comment,
            min,
            max,
            x_dim,
            y_dim,
            unit,
            offset,
        }
    }

    pub fn name(&self) -> &str {
        self.name.as_str()
    }

    pub fn datatype(&self) -> &'static str {
        self.datatype
    }

    pub fn comment(&self) -> &'static str {
        self.comment
    }

    pub fn min(&self) -> f64 {
        self.min
    }

    pub fn max(&self) -> f64 {
        self.max
    }

    pub fn unit(&self) -> &'static str {
        self.unit
    }

    pub fn x_dim(&self) -> usize {
        self.x_dim
    }

    pub fn y_dim(&self) -> usize {
        self.y_dim
    }

    pub fn characteristic_type(&self) -> &'static str {
        if self.x_dim > 1 && self.y_dim > 1 {
            "MAP"
        } else if self.x_dim > 1 || self.y_dim > 1 {
            "CURVE"
        } else {
            "VALUE"
        }
    }

    pub fn offset(&self) -> u16 {
        self.offset
    }

    pub fn set_name(&mut self, name: String) {
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

impl_xcp_type_description_for_primitive!(u8, u16, u32, u64, usize, i8, i16, i32, i64, isize, f32, f64, bool, char, String);

// The implementation of the XcpTypeDescription trait for
// arrays is also a blanket (empty) trait implementation
impl<T, const N: usize> XcpTypeDescription for [T; N] {}

/// StructDescriptor is a vec of FieldDescriptor
/// It it created with the XcpTypeDescription proc-macro trait
#[derive(Debug, Default)]
pub struct StructDescriptor(Vec<FieldDescriptor>);

impl StructDescriptor {
    pub fn new() -> Self {
        StructDescriptor(Vec::new())
    }

    pub fn push(&mut self, field_descriptor: FieldDescriptor) {
        self.0.push(field_descriptor);
    }

    pub fn sort(&mut self) {
        self.0.sort_by(|a, b| a.name.as_str().cmp(b.name.as_str()));
    }

    pub fn iter(&self) -> std::slice::Iter<FieldDescriptor> {
        self.0.iter()
    }
}

impl IntoIterator for StructDescriptor {
    type Item = FieldDescriptor;
    type IntoIter = IntoIter<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl Extend<FieldDescriptor> for StructDescriptor {
    fn extend<T: IntoIterator<Item = FieldDescriptor>>(&mut self, iter: T) {
        self.0.extend(iter);
    }
}
