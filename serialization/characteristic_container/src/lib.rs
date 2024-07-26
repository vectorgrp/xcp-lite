pub mod prelude;

pub trait CharacteristicContainer {
    fn characteristics(&self) -> Option<Vec<Characteristic>> {
        None
    }
}

/*  TODO: Calseg name is introduced as it is necessary to get
    the index from the CalSeg List of XCP. There has to be
    a more elegant way to solve this that does not force
    characteristics to have knowledge of cal segments
*/
#[derive(Debug)]
pub struct Characteristic {
    calseg_name: &'static str,
    name: String,
    datatype: &'static str,
    comment: &'static str,
    min: f64,
    max: f64,
    unit: &'static str,
    x_dim: usize,
    y_dim: usize,
    offset: u16,
    extension: u8, //TODO: Discuss hardcoding extension vs Xcp::get_calseg_ext_addr
}

impl Characteristic {
    pub fn new(
        calseg_name: &'static str,
        name: String,
        datatype: &'static str,
        comment: &'static str,
        min: f64,
        max: f64,
        unit: &'static str,
        x_dim: usize,
        y_dim: usize,
        offset: u16,
        extension: u8,
    ) -> Self {
        Characteristic {
            calseg_name,
            name,
            datatype,
            comment,
            min,
            max,
            x_dim,
            y_dim,
            unit,
            offset,
            extension,
        }
    }

    pub fn calseg_name<'a>(&'a self) -> &'static str {
        &self.calseg_name
    }

    //TODO: Check if returning &str is better
    pub fn name(&self) -> &String {
        &self.name
    }

    pub fn datatype(&self) -> &&str {
        &self.datatype
    }

    pub fn comment(&self) -> &&str {
        &self.comment
    }

    pub fn min(&self) -> &f64 {
        &self.min
    }

    pub fn max(&self) -> &f64 {
        &self.max
    }

    pub fn unit(&self) -> &&str {
        &self.unit
    }

    pub fn x_dim(&self) -> &usize {
        &self.x_dim
    }

    pub fn y_dim(&self) -> &usize {
        &self.y_dim
    }

    pub fn characteristic_type(&self) -> &'static str {
        if self.x_dim > 1 && self.y_dim > 1 {
            "MAP"
        } else if self.x_dim > 1 || self.y_dim > 1 {
            "CURVE"
        } else {
            "VALUE"
        }
    }

    pub fn offset(&self) -> &u16 {
        &self.offset
    }

    pub fn extension(&self) -> &u8 {
        &self.extension
    }

    pub fn set_name(&mut self, name: String) {
        self.name = name;
    }

    pub fn set_calseg_name(&mut self, name: &'static str) {
        self.calseg_name = name;
    }
}

// The CharacteristicContainer trait implementation for Rust primitives
// is simply a blanket (empty) trait implementation. This macro is used
// to automatically generate the implementation for Rust primitives
macro_rules! impl_characteristic_container_for_primitive {
    ($($t:ty),*) => {
        $(
            impl CharacteristicContainer for $t {}
        )*
    };
}

impl_characteristic_container_for_primitive!(
    u8, u16, u32, u64, usize, i8, i16, i32, i64, isize, f32, f64, bool, char, String
);

// The implementation of the CharacteristicContainer trait for
// arrays is also a blanket (empty) trait implementation
impl<T, const N: usize> CharacteristicContainer for [T; N] {}

#[derive(Debug)]
pub struct RegistryCharacteristicList(Vec<Characteristic>);

impl RegistryCharacteristicList {
    pub fn new() -> Self {
        RegistryCharacteristicList(Vec::new())
    }

    pub fn push(&mut self, characteristic: Characteristic) {
        self.0.push(characteristic);
    }

    pub fn sort(&mut self) {
        self.0.sort_by(|a, b| a.name.as_str().cmp(b.name.as_str()));
    }

    pub fn iter(&self) -> std::slice::Iter<Characteristic> {
        self.0.iter()
    }
}
