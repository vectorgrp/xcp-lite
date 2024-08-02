use crate::domain::*;
use crate::types::*;
use lazy_static::lazy_static;
use std::{collections::HashMap, sync::Arc};

lazy_static! {
    static ref TRANSLATORS: TranslatorCollection = TranslatorCollection::new();
}

//TODO; Figure out how to move to types
pub type TranslatorBox = Arc<dyn Translator + Send + Sync>;

pub trait Translator: Send {
    fn translate(&self, input: &Struct) -> String;
    fn translate_fields(&self, input: &Struct) -> String;
}

struct TranslatorCollection {
    map: HashMap<IDL, TranslatorBox>,
}

struct CdrTranslator {
    type_translation: CdrTypeTranslation,
}

impl CdrTranslator {
    fn new() -> Self {
        Self {
            type_translation: CdrTypeTranslation::new(),
        }
    }
}

impl Translator for CdrTranslator {
    // This only returns a vector for now
    fn translate(&self, input: &Struct) -> String {
        // println!("Translating");
        let type_name = input.type_name();
        let lc_typename = type_name.to_ascii_lowercase();
        let fields_str = self.translate_fields(input);

        let translation = format!(
            r#"
            /begin ANNOTATION ANNOTATION_LABEL "ObjectDescription" ANNOTATION_ORIGIN "application/dds"
                /begin ANNOTATION_TEXT
                    "<DynamicObject> "
                    "<RootType>{VECTOR_NAMESPACE}::{type_name}{RUST_VECTOR}</RootType>"
                    "</DynamicObject>"
                    "module {VECTOR_NAMESPACE} {{"
                    "  struct {type_name} {{"
                    "      {fields_str}"
                    "  }};"
                    "
                    "  struct {type_name}{RUST_VECTOR} {{"
                    "    sequence<{type_name}> {lc_typename}s;"
                    "  }};
                    }};"
                /end ANNOTATION_TEXT
            /end ANNOTATION
            "#
        );

        translation
    }

    fn translate_fields(&self, input: &Struct) -> String {
        input
            .fields()
            .iter()
            .map(|field| {
                let datatype = field.datatype();
                let translated_type = self.type_translation.get(&datatype).unwrap_or(&datatype);
                format!("{} {};", translated_type, field.name())
            })
            .collect::<Vec<String>>()
            .join("\n      ")
    }
}

impl TranslatorCollection {
    fn new() -> Self {
        let mut map = HashMap::new();
        map.insert(IDL::CDR, create_translator_box(CdrTranslator::new()));
        Self { map }
    }

    fn translate(&self, idl_type: &IDL, input: &Struct) -> Option<String> {
        self.map
            .get(idl_type)
            .map(|translator| translator.translate(input))
    }
}

//TODO: Move to utils
fn create_translator_box<T>(translator: T) -> TranslatorBox
where
    T: Translator + Send + Sync + 'static,
{
    Arc::new(translator) as TranslatorBox
}

pub struct CdrTypeTranslation {
    map: HashMap<&'static str, &'static str>,
}

impl CdrTypeTranslation {
    pub fn new() -> Self {
        let mut map = HashMap::new();
        map.insert("u32", "uint32");
        Self { map }
    }

    pub fn get(&self, key: &str) -> Option<&&'static str> {
        self.map.get(key)
    }
}

pub fn translate_idl_struct(idl_type: IDL, input: &Struct) -> String {
    let translation = TRANSLATORS.translate(&idl_type, input).unwrap();
    translation
}