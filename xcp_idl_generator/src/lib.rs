pub mod types;
pub mod domain;
pub mod translator;

use types::Struct;

pub trait IdlGenerator {
    fn description() -> Struct;
}
