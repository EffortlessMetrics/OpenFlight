// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

use crate::health::HealthStream;

use super::COMPONENT_NAME;

#[derive(Debug, Default)]
pub(super) struct HealthMessages {
    info: Vec<String>,
    warnings: Vec<String>,
    errors: Vec<String>,
}

impl HealthMessages {
    pub(super) fn info(&mut self, message: impl Into<String>) {
        self.info.push(message.into());
    }

    pub(super) fn warning(&mut self, message: impl Into<String>) {
        self.warnings.push(message.into());
    }

    pub(super) fn error(&mut self, message: impl Into<String>) {
        self.errors.push(message.into());
    }

    pub(super) async fn emit(self, health: &HealthStream) {
        for message in self.info {
            health.info(COMPONENT_NAME, &message).await;
        }
        for message in self.warnings {
            health.warning(COMPONENT_NAME, &message).await;
        }
        for message in self.errors {
            health.error(COMPONENT_NAME, &message, None).await;
        }
    }
}
