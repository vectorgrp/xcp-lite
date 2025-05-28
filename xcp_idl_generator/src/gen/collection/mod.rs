pub mod cdr;

use super::{Generator, IDL, Struct};
use cdr::CdrGenerator;
use std::{
    collections::HashMap,
    sync::{Arc, Once},
};

pub struct GeneratorCollection(HashMap<IDL, ArcGenerator>);

impl GeneratorCollection {
    fn new() -> Self {
        let mut map = HashMap::new();
        map.insert(IDL::CDR, create_arc_generator(CdrGenerator::new()));
        GeneratorCollection(map)
    }

    pub fn generate(idl_type: &IDL, input: &Struct) -> Option<String> {
        let instance = GeneratorCollection::instance();
        let generator = instance.get(idl_type).expect("Generator not found for IDL type");

        Some(generator.generate(input))
    }

    pub fn instance() -> &'static GeneratorCollection {
        static mut INSTANCE: Option<GeneratorCollection> = None;
        static INIT: Once = Once::new();

        // @@@@ UNSAFE - Mutable static, TODO
        unsafe {
            INIT.call_once(|| {
                INSTANCE = Some(GeneratorCollection::new());
            });
            #[allow(static_mut_refs)]
            INSTANCE.as_ref().unwrap()
        }
    }

    fn get(&self, idl_type: &IDL) -> Option<&ArcGenerator> {
        self.0.get(idl_type)
    }
}

type ArcGenerator = Arc<dyn Generator + Send + Sync>;

fn create_arc_generator<T>(generator: T) -> ArcGenerator
where
    T: Generator + Send + Sync + 'static,
{
    Arc::new(generator) as ArcGenerator
}
