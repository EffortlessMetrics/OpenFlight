// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Core panel rules evaluation and LED control.

pub mod evaluator;
pub mod led;

pub use evaluator::RulesEvaluator;
pub use led::{LatencyStats, LedController, LedState, LedTarget};
