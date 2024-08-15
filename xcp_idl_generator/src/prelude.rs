pub use crate::gen::collection::GeneratorCollection;
pub use crate::gen::Generator;
pub use crate::types::{Field, FieldList, Struct, IDL};

pub use crate::{IdlGenerator, STRUCTS};
pub use xcp_idl_generator_derive::IdlGenerator;

pub extern crate ctor;
pub extern crate lazy_static;

pub use ctor::ctor;
pub use lazy_static::lazy_static;
