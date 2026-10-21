//! A simulator's screen off macOS, where there are no simulators to stream.

use std::sync::{Arc, Mutex};

pub type Picture = Arc<Mutex<Option<Frame>>>;

pub enum Frame {}

pub struct Decoder;

impl Decoder {
    pub fn new(_: Picture) -> Self {
        Self
    }

    pub fn decode(&mut self, _: &[u8]) -> Result<(), String> {
        Err("the simulator's screen needs macOS".into())
    }
}
