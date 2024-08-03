use crate::types::*;
use std::collections::HashMap;

pub mod collection;

pub trait Generator: Send {
    fn generate(&self, input: &Struct) -> String;
    fn type_mapping(&self) -> &'static TypeMapping;
}

pub struct TypeMapping(HashMap<&'static str, &'static str>);

impl TypeMapping {
    pub fn new() -> Self {
        TypeMapping(HashMap::new())
    }

    pub fn insert(&mut self, key: &'static str, value: &'static str) {
        self.0.insert(key, value);
    }

    fn get(&self, key: &str) -> Option<&&'static str> {
        self.0.get(key)
    }
}
