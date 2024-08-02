pub mod cdr;

use super::{Struct, Generator, IDL};
use cdr::CdrGenerator;
use std::{collections::HashMap, sync::Arc};

type TranslatorBox = Arc<dyn Generator + Send + Sync>;

pub struct GeneratorCollection(HashMap<IDL, TranslatorBox>);

impl GeneratorCollection {
    pub fn new() -> Self {
        let mut map = HashMap::new();
        map.insert(IDL::CDR, create_translator_box(CdrGenerator::new()));
        GeneratorCollection(map)
    }

    pub fn translate(&self, idl_type: &IDL, input: &Struct) -> Option<String> {
        self.0
            .get(idl_type)
            .map(|translator| translator.translate(input))
    }
}

fn create_translator_box<T>(translator: T) -> TranslatorBox
where
    T: Generator + Send + Sync + 'static,
{
    Arc::new(translator) as TranslatorBox
}
