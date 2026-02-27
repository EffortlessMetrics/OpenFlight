// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Data subscription management for key aircraft state variables.
//!
//! Provides a focused subscription descriptor for the seven core aircraft
//! state variables required for axis processing, FFB synthesis, and
//! auto-profile switching. The actual SimConnect data definitions and
//! periodic requests are established by [`crate::mapping::VariableMapping`];
//! `DataSubscription` describes *which* variables are of interest and at
//! what rate.

/// A SimConnect variable to subscribe to, with its associated unit string.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SubscriptionVariable {
    /// SimConnect variable name (e.g. `"AIRSPEED INDICATED"`).
    pub name: &'static str,
    /// Unit string used when registering with SimConnect (e.g. `"knots"`).
    pub units: &'static str,
}

/// The seven core aircraft state variables used for axis processing and FFB.
pub const CORE_SUBSCRIPTION_VARS: &[SubscriptionVariable] = &[
    SubscriptionVariable {
        name: "AIRSPEED INDICATED",
        units: "knots",
    },
    SubscriptionVariable {
        name: "INDICATED ALTITUDE",
        units: "feet",
    },
    SubscriptionVariable {
        name: "PLANE PITCH DEGREES",
        units: "degrees",
    },
    SubscriptionVariable {
        name: "PLANE BANK DEGREES",
        units: "degrees",
    },
    SubscriptionVariable {
        name: "PLANE HEADING DEGREES GYRO",
        units: "degrees",
    },
    SubscriptionVariable {
        name: "GEAR POSITION",
        units: "enum",
    },
    SubscriptionVariable {
        name: "FLAPS HANDLE INDEX",
        units: "number",
    },
];

/// Configuration for a [`DataSubscription`].
#[derive(Debug, Clone)]
pub struct DataSubscriptionConfig {
    /// Variables to subscribe to. Defaults to [`CORE_SUBSCRIPTION_VARS`].
    pub variables: Vec<SubscriptionVariable>,
    /// Requested update rate in Hz.
    pub update_rate_hz: f32,
}

impl Default for DataSubscriptionConfig {
    fn default() -> Self {
        Self {
            variables: CORE_SUBSCRIPTION_VARS.to_vec(),
            update_rate_hz: 30.0,
        }
    }
}

/// Descriptor for a set of SimConnect data subscriptions.
///
/// This struct describes which variables should be requested and at what rate.
/// It does not interact with SimConnect directly — pass it to the adapter
/// during connection setup so it can register the appropriate data definitions.
pub struct DataSubscription {
    config: DataSubscriptionConfig,
}

impl DataSubscription {
    /// Create a new subscription descriptor with the given configuration.
    pub fn new(config: DataSubscriptionConfig) -> Self {
        Self { config }
    }

    /// Returns the configured variables.
    pub fn variables(&self) -> &[SubscriptionVariable] {
        &self.config.variables
    }

    /// Returns the number of subscribed variables.
    pub fn variable_count(&self) -> usize {
        self.config.variables.len()
    }

    /// Returns the configured update rate in Hz.
    pub fn update_rate_hz(&self) -> f32 {
        self.config.update_rate_hz
    }

    /// Returns `true` if the given SimConnect variable name is in this subscription.
    pub fn contains(&self, var_name: &str) -> bool {
        self.config.variables.iter().any(|v| v.name == var_name)
    }
}

impl Default for DataSubscription {
    fn default() -> Self {
        Self::new(DataSubscriptionConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_data_subscription_config_default_rate() {
        let config = DataSubscriptionConfig::default();
        assert_eq!(config.update_rate_hz, 30.0);
    }

    #[test]
    fn test_data_subscription_config_default_variable_count() {
        let config = DataSubscriptionConfig::default();
        assert_eq!(
            config.variables.len(),
            7,
            "default config must contain exactly 7 core variables"
        );
    }

    #[test]
    fn test_subscription_contains_airspeed_indicated() {
        let sub = DataSubscription::default();
        assert!(sub.contains("AIRSPEED INDICATED"));
    }

    #[test]
    fn test_subscription_contains_indicated_altitude() {
        let sub = DataSubscription::default();
        assert!(sub.contains("INDICATED ALTITUDE"));
    }

    #[test]
    fn test_subscription_contains_attitude_variables() {
        let sub = DataSubscription::default();
        assert!(sub.contains("PLANE PITCH DEGREES"));
        assert!(sub.contains("PLANE BANK DEGREES"));
        assert!(sub.contains("PLANE HEADING DEGREES GYRO"));
    }

    #[test]
    fn test_subscription_contains_gear_and_flaps() {
        let sub = DataSubscription::default();
        assert!(sub.contains("GEAR POSITION"));
        assert!(sub.contains("FLAPS HANDLE INDEX"));
    }

    #[test]
    fn test_subscription_does_not_contain_unknown_variable() {
        let sub = DataSubscription::default();
        assert!(!sub.contains("UNKNOWN VARIABLE"));
    }

    #[test]
    fn test_subscription_variable_count() {
        let sub = DataSubscription::default();
        assert_eq!(sub.variable_count(), 7);
    }

    #[test]
    fn test_subscription_update_rate() {
        let sub = DataSubscription::default();
        assert_eq!(sub.update_rate_hz(), 30.0);
    }

    #[test]
    fn test_subscription_custom_config() {
        let config = DataSubscriptionConfig {
            variables: vec![SubscriptionVariable {
                name: "AIRSPEED INDICATED",
                units: "knots",
            }],
            update_rate_hz: 60.0,
        };
        let sub = DataSubscription::new(config);
        assert_eq!(sub.variable_count(), 1);
        assert_eq!(sub.update_rate_hz(), 60.0);
        assert!(sub.contains("AIRSPEED INDICATED"));
        assert!(!sub.contains("INDICATED ALTITUDE"));
    }

    #[test]
    fn test_core_subscription_vars_units() {
        let expected: &[(&str, &str)] = &[
            ("AIRSPEED INDICATED", "knots"),
            ("INDICATED ALTITUDE", "feet"),
            ("PLANE PITCH DEGREES", "degrees"),
            ("PLANE BANK DEGREES", "degrees"),
            ("PLANE HEADING DEGREES GYRO", "degrees"),
            ("GEAR POSITION", "enum"),
            ("FLAPS HANDLE INDEX", "number"),
        ];
        assert_eq!(CORE_SUBSCRIPTION_VARS.len(), expected.len());
        for (var, (name, units)) in CORE_SUBSCRIPTION_VARS.iter().zip(expected.iter()) {
            assert_eq!(var.name, *name);
            assert_eq!(var.units, *units);
        }
    }

    #[test]
    fn test_subscription_variables_returns_slice() {
        let sub = DataSubscription::default();
        let vars = sub.variables();
        assert_eq!(vars.len(), 7);
        assert_eq!(vars[0].name, "AIRSPEED INDICATED");
    }
}
