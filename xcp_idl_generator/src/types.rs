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

#[derive(Eq, Hash, PartialEq)]
pub enum IDL {
    CDR,
}
