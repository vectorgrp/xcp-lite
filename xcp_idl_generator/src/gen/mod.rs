use crate::types::*;
use std::collections::HashMap;

pub mod collection;

pub trait Generator: Send {
    fn generate(&self, input: &Struct) -> String;
    fn type_mapping(&self) -> &'static TypeMapping;
}

#[derive(Debug)]
pub struct TypeMapping(HashMap<&'static str, &'static str>);

impl TypeMapping {
    pub fn new() -> Self {
        TypeMapping(HashMap::new())
    }

    pub fn insert(&mut self, key: &'static str, value: &'static str) {
        self.0.insert(key, value);
    }

    pub fn iter<'a>(&'a self) -> impl Iterator<Item = (&'static str, &'static str)> + 'a {
        self.0.iter().map(|(k, v)| (*k, *v))
    }
}
