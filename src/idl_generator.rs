//TODO: Remove
#![allow(warnings)]

pub trait IdlGenerator {
    fn generate_idl() -> IdlStruct;
}

pub fn translate_idl_struct(input: &IdlStruct) -> String {
    let name = input.name();
    let lowercase_name = name.to_ascii_lowercase();
    let fields_str = input
        .fields()
        .iter()
        .map(|field| format!("    {} {};", field.datatype(), field.name()))
        .collect::<Vec<_>>()
        .join("\n");

    let annotation = format!(
        r#"
        /begin ANNOTATION ANNOTATION_LABEL "ObjectDescription" ANNOTATION_ORIGIN "application/dds" /begin ANNOTATION_TEXT
            "<DynamicObject> "
            "<RootType>Vector::{name}Vector</RootType>"
            "</DynamicObject>"
            "module Vector {{"
            "  struct {name} {{"
            "{fields_str}"
            "  }};"
            "  struct {name}Vector {{"
            "    sequence<{name}> {lowercase_name}s;"
            "}}; }};"
        /end ANNOTATION_TEXT /end ANNOTATION
        "#
    );

    annotation
}

#[derive(Debug)]
pub struct IdlStructField(String, String);

impl IdlStructField {
    pub fn new(name: String, field_type: String) -> Self {
        IdlStructField(name, field_type)
    }

    pub fn name(&self) -> &str {
        &self.0
    }

    pub fn datatype(&self) -> &str {
        &self.1
    }
}

#[derive(Debug)]
pub struct IdlStruct(String, IdlStructFieldVec);

impl IdlStruct {
    pub fn new(name: String, fields: IdlStructFieldVec) -> Self {
        IdlStruct(name, fields)
    }

    pub fn name(&self) -> &str {
        &self.0
    }

    pub fn fields(&self) -> &IdlStructFieldVec {
        &self.1
    }
}

#[derive(Debug)]
pub struct IdlStructFieldVec(Vec<IdlStructField>);

impl IdlStructFieldVec {
    pub fn new() -> Self {
        IdlStructFieldVec(Vec::new())
    }

    pub fn with_capacity(capacity: usize) -> Self {
        IdlStructFieldVec(Vec::with_capacity(capacity))
    }

    pub fn push(&mut self, field: IdlStructField) {
        self.0.push(field);
    }

    pub fn iter(&self) -> impl Iterator<Item = &IdlStructField> {
        self.0.iter()
    }
}
