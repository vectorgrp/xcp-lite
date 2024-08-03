use crate::domain::{RUST_VECTOR, VECTOR_NAMESPACE};
use crate::gen::Generator;
use crate::gen::TypeMapping;
use crate::types::Struct;
use std::sync::Once;

pub struct CdrGenerator;

impl CdrGenerator {
    pub fn new() -> Self {
        Self {}
    }

    fn translate_fields(&self, input: &Struct) -> String {
        input
            .fields()
            .iter()
            .map(|field| {
                let datatype = field.datatype();
                let translated_type = self.type_mapping().get(&datatype).unwrap(); //TODO: Error Handling
                format!("{} {};", translated_type, field.name())
            })
            .collect::<Vec<String>>()
            .join("\n      ")
    }
}

impl Generator for CdrGenerator {
    fn generate(&self, input: &Struct) -> String {
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

    fn type_mapping(&self) -> &'static TypeMapping {
        static mut MAPPING: Option<TypeMapping> = None;
        static INIT: Once = Once::new();

        unsafe {
            INIT.call_once(|| {
                let mut mapping = TypeMapping::new();
                mapping.insert("u32", "uint32");

                MAPPING = Some(mapping);
            });
            MAPPING.as_ref().unwrap() //TODO: Error Handling??
        }
    }
}
