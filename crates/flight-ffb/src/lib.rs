//! Force feedback engine

pub struct FfbEngine;

impl FfbEngine {
    pub fn new() -> Self {
        Self
    }
}

impl Default for FfbEngine {
    fn default() -> Self {
        Self::new()
    }
}
