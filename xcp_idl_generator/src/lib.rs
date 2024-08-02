pub mod types;
pub mod domain;
pub mod gen;
pub mod prelude;

use types::Struct;

pub trait IdlGenerator {
    fn description() -> Struct;
}
