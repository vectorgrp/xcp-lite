pub mod domain;
pub mod r#gen;
pub mod prelude;
pub mod types;

use std::{collections::HashMap, sync::Mutex};

use types::Struct;

use lazy_static::lazy_static;

lazy_static! {
    pub static ref STRUCTS: Mutex<HashMap<&'static str, &'static Struct>> = Mutex::new(HashMap::new());
}

pub trait IdlGenerator {
    fn description(&self) -> &'static Struct;
}
