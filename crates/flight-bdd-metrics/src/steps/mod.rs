// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2025 Flight Hub Team

//! BDD step definition modules connecting Gherkin steps to real OpenFlight code.

pub mod axis_steps;
pub mod bus_steps;
pub mod device_steps;
pub mod profile_steps;

use crate::step_registry::StepRegistry;

/// Register all step definitions from every module into the given registry.
pub fn register_all(registry: &mut StepRegistry) {
    axis_steps::register(registry);
    profile_steps::register(registry);
    device_steps::register(registry);
    bus_steps::register(registry);
}
