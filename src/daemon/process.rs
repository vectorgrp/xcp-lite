use std::fmt::Display;

pub trait Process {
    type Error: Display;

    fn init(&mut self) -> Result<(), Self::Error>;
    fn run(&mut self) -> Result<(), Self::Error>;
    fn deinit(&mut self) -> Result<(), Self::Error>;
}
