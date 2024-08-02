const VECTOR_NAMESPACE: &'static &str = &"Vector";
const RUST_VECTOR: &'static &str = &"Vec";

use lazy_static::lazy_static;
use std::collections::HashMap;
use std::sync::Arc;

type TranslatorBox = Arc<dyn Translator + Send + Sync>;

lazy_static! {
    static ref TRANSLATORS: TranslatorCollection = TranslatorCollection::new();
}

fn create_translator_box<T>(translator: T) -> TranslatorBox
where
    T: Translator + Send + Sync + 'static,
{
    Arc::new(translator) as TranslatorBox
}

pub trait IdlGenerator {
    fn description() -> Struct;
}

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

#[derive(Eq, Hash, PartialEq)]
pub enum IDL {
    CDR,
}

pub fn translate_idl_struct(idl_type: IDL, input: &Struct) -> String {
    let translation = TRANSLATORS.translate(&idl_type, input).unwrap();
    translation
}

#[derive(Debug)]
pub struct Field(String, String);

impl Field {
    pub fn new(name: String, field_type: String) -> Self {
        Field(name, field_type)
    }

    pub fn name(&self) -> &str {
        &self.0
    }

    pub fn datatype(&self) -> &str {
        &self.1
    }
}

#[derive(Debug)]
pub struct Struct(String, FieldList);

impl Struct {
    pub fn new(name: String, fields: FieldList) -> Self {
        Struct(name, fields)
    }

    pub fn type_name(&self) -> &str {
        &self.0
    }

    pub fn fields(&self) -> &FieldList {
        &self.1
    }
}

#[derive(Debug)]
pub struct FieldList(Vec<Field>);

impl FieldList {
    pub fn new() -> Self {
        Self(Vec::new())
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self(Vec::with_capacity(capacity))
    }

    pub fn push(&mut self, field: Field) {
        self.0.push(field);
    }

    pub fn iter(&self) -> impl Iterator<Item = &Field> {
        self.0.iter()
    }
}
