//! Virtual device implementation for testing

pub struct VirtualDevice {
    pub name: String,
}

impl VirtualDevice {
    pub fn new(name: String) -> Self {
        Self { name }
    }
}
