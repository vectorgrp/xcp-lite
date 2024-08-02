use std::collections::HashMap;
use lazy_static::lazy_static;

lazy_static! {
    pub static ref CDR_TYPE_TRANSLATION : HashMap<&'static str, &'static str> = {
        //TODO: With capacity when the type cnt is defined
        let mut map = HashMap::new();
        map.insert("u32", "uint32");
        map
    };
}