use crate::types::*;
use collection::GeneratorCollection;
use lazy_static::lazy_static;

mod collection;

lazy_static! {
    pub static ref GENERATOR_COLLECTION: GeneratorCollection = GeneratorCollection::new();
}

pub trait Generator: Send {
    fn translate(&self, input: &Struct) -> String;
    fn translate_fields(&self, input: &Struct) -> String;
}

pub fn generate(idl_type: IDL, input: &Struct) -> String {
    let translation = GENERATOR_COLLECTION.translate(&idl_type, input).unwrap();
    translation
}
