// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Metric collection trait.

use crate::types::Metric;

/// Common interface for metric collection backends.
pub trait MetricsCollector {
    /// Collect a snapshot of metrics.
    fn collect(&self) -> Vec<Metric>;

    /// Reset stored metrics.
    fn reset(&self);
}
