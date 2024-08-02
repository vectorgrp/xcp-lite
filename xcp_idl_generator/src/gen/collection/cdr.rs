use std::collections::HashMap;

use crate::gen::Generator;
use crate::types::Struct;
use crate::domain::{RUST_VECTOR, VECTOR_NAMESPACE};

pub struct TypeMapping(HashMap<&'static str, &'static str>);

impl TypeMapping {
    pub fn new() -> Self {
        let mut map = HashMap::new();
        //TODO: Add other types
        map.insert("u32", "uint32");
        TypeMapping(map)
    }

    fn get(&self, key: &str) -> Option<&&'static str> {
        self.0.get(key)
    }
}

pub struct CdrGenerator {
    type_mapping: TypeMapping,
}

impl CdrGenerator {
    pub fn new() -> Self {
        Self {
            type_mapping: TypeMapping::new(),
        }
    }
}

impl Generator for CdrGenerator {
    fn translate(&self, input: &Struct) -> String {
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
                let translated_type = self.type_mapping.get(&datatype).unwrap_or(&datatype);
                format!("{} {};", translated_type, field.name())
            })
            .collect::<Vec<String>>()
            .join("\n      ")
    }
}