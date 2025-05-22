use crate::STRUCTS;
use crate::domain::VECTOR_NAMESPACE;
use crate::r#gen::Generator;
use crate::r#gen::TypeMapping;
use crate::types::Struct;
use regex::Regex;
use std::sync::Once;

//TODO Move to common package
fn extract_types(input: &str) -> Vec<&str> {
    let re = Regex::new(r"[^\w]+").unwrap();
    re.split(input).filter(|s| !s.is_empty()).collect()
}

pub struct CdrGenerator;

impl CdrGenerator {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Self {}
    }

    fn translate_fields(&self, input: &Struct) -> String {
        input
            .fields()
            .iter()
            .map(|field| {
                let mut translated_type = field.value_type().to_string();

                for (key, value) in self.type_mapping().iter() {
                    translated_type = translated_type.replace(key, value);
                }

                format!("\"{} {};\"", translated_type, field.name())
            })
            .collect::<Vec<String>>()
            .join("\n")
    }
}

impl Generator for CdrGenerator {
    fn generate(&self, input: &Struct) -> String {
        let type_name = input.type_name();
        let fields_str = self.translate_fields(input);

        let mut translation = format!(
            r#"
            /begin ANNOTATION ANNOTATION_LABEL "ObjectDescription" ANNOTATION_ORIGIN "application/dds"
                /begin ANNOTATION_TEXT
                    "<DynamicObject> "
                    "<RootType>{VECTOR_NAMESPACE}::{type_name}</RootType>"
                    "</DynamicObject>"
                    "module {VECTOR_NAMESPACE} {{"
                    "  struct {type_name} {{"
                          {fields_str}
                    "  }};"
                    "}};"
                /end ANNOTATION_TEXT
            /end ANNOTATION
            "#
        );

        let struct_collection = STRUCTS.lock().unwrap();

        let mut processed: Vec<&str> = Vec::new();

        for field in input.fields().iter() {
            let extracted_type_tree = extract_types(field.value_type());

            for value_type in extracted_type_tree.iter() {
                match self.type_mapping().get(value_type) {
                    None => {
                        if processed.contains(value_type) {
                            continue;
                        }

                        let s_slice: &str = value_type;
                        let description = struct_collection.get(s_slice).unwrap();

                        let inner_type_name = description.type_name();
                        let inner_fields_str = self.translate_fields(description);

                        let idl_str = format!(
                            r#""struct {inner_type_name} {{"
                                    {inner_fields_str}
                                "}};"#
                        );

                        let tag = format!("module {VECTOR_NAMESPACE} {{");
                        translation = translation.replace(&tag, &format!("module {VECTOR_NAMESPACE} {{\"\n{}", idl_str));

                        processed.push(value_type);
                    }
                    Some(_) => { /* Rust primitive -> Ignored */ }
                }
            }
        }

        translation
    }

    //TODO Add other type mappings
    fn type_mapping(&self) -> &'static TypeMapping {
        static mut MAPPING: Option<TypeMapping> = None;
        static INIT: Once = Once::new();

        // @@@@ UNSAFE - Mutable static, TODO
        unsafe {
            INIT.call_once(|| {
                let mut mapping = TypeMapping::new();
                mapping.insert("u32", "uint32");
                mapping.insert("f32", "float");
                mapping.insert("Vec", "sequence");

                MAPPING = Some(mapping);
            });
            #[allow(static_mut_refs)]
            MAPPING.as_ref().unwrap() //TODO Error Handling??
        }
    }
}
