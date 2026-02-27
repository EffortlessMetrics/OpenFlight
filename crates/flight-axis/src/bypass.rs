//! Axis bypass mode for raw passthrough.
//!
//! In bypass mode, the raw hardware value is passed directly to output
//! without any processing stages. Useful for debugging and sim integration.

/// Bypass mode configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct BypassConfig {
    /// Whether bypass mode is enabled.
    pub enabled: bool,
}

impl BypassConfig {
    pub const fn enabled() -> Self {
        Self { enabled: true }
    }
    pub const fn disabled() -> Self {
        Self { enabled: false }
    }
}

/// Tracks bypass state and transitions.
#[derive(Debug, Clone)]
pub struct BypassGate {
    config: BypassConfig,
    last_raw: f32,
    transition_count: u64,
}

impl BypassGate {
    pub fn new(config: BypassConfig) -> Self {
        Self {
            config,
            last_raw: 0.0,
            transition_count: 0,
        }
    }

    /// Process a value through the bypass gate.
    ///
    /// If bypass is enabled: returns raw value directly.
    /// If bypass is disabled: returns None (caller should run normal pipeline).
    pub fn process(&mut self, raw: f32) -> Option<f32> {
        self.last_raw = raw;
        if self.config.enabled { Some(raw) } else { None }
    }

    /// Enable bypass mode, counting the transition.
    pub fn enable(&mut self) {
        if !self.config.enabled {
            self.config.enabled = true;
            self.transition_count += 1;
        }
    }

    /// Disable bypass mode, counting the transition.
    pub fn disable(&mut self) {
        if self.config.enabled {
            self.config.enabled = false;
            self.transition_count += 1;
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.config.enabled
    }
    pub fn transition_count(&self) -> u64 {
        self.transition_count
    }
    pub fn last_raw(&self) -> f32 {
        self.last_raw
    }
    pub fn config(&self) -> BypassConfig {
        self.config
    }
}

impl Default for BypassGate {
    fn default() -> Self {
        Self::new(BypassConfig::disabled())
    }
}

/// Bank of bypass gates for multiple axes.
#[derive(Debug, Clone)]
pub struct BypassBank<const N: usize> {
    gates: [BypassGate; N],
}

impl<const N: usize> BypassBank<N> {
    pub fn new_all_disabled() -> Self {
        Self {
            gates: std::array::from_fn(|_| BypassGate::default()),
        }
    }

    pub fn new_all_enabled() -> Self {
        Self {
            gates: std::array::from_fn(|_| BypassGate::new(BypassConfig::enabled())),
        }
    }

    pub fn gate(&self, axis: usize) -> Option<&BypassGate> {
        self.gates.get(axis)
    }
    pub fn gate_mut(&mut self, axis: usize) -> Option<&mut BypassGate> {
        self.gates.get_mut(axis)
    }

    pub fn process(&mut self, axis: usize, raw: f32) -> Option<f32> {
        self.gates.get_mut(axis)?.process(raw)
    }

    pub fn enable_all(&mut self) {
        for gate in &mut self.gates {
            gate.enable();
        }
    }

    pub fn disable_all(&mut self) {
        for gate in &mut self.gates {
            gate.disable();
        }
    }

    pub fn enable(&mut self, axis: usize) {
        if let Some(gate) = self.gates.get_mut(axis) {
            gate.enable();
        }
    }

    pub fn disable(&mut self, axis: usize) {
        if let Some(gate) = self.gates.get_mut(axis) {
            gate.disable();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bypass_disabled_returns_none() {
        let mut gate = BypassGate::new(BypassConfig::disabled());
        assert_eq!(gate.process(0.5), None);
    }

    #[test]
    fn test_bypass_enabled_returns_raw() {
        let mut gate = BypassGate::new(BypassConfig::enabled());
        assert_eq!(gate.process(0.75), Some(0.75));
    }

    #[test]
    fn test_enable_counts_transition() {
        let mut gate = BypassGate::default();
        assert_eq!(gate.transition_count(), 0);
        gate.enable();
        assert_eq!(gate.transition_count(), 1);
        assert!(gate.is_enabled());
    }

    #[test]
    fn test_disable_counts_transition() {
        let mut gate = BypassGate::new(BypassConfig::enabled());
        assert_eq!(gate.transition_count(), 0);
        gate.disable();
        assert_eq!(gate.transition_count(), 1);
        assert!(!gate.is_enabled());
    }

    #[test]
    fn test_double_enable_no_extra_transition() {
        let mut gate = BypassGate::default();
        gate.enable();
        gate.enable();
        assert_eq!(gate.transition_count(), 1);
    }

    #[test]
    fn test_last_raw_updated() {
        let mut gate = BypassGate::default();
        gate.process(0.3);
        assert_eq!(gate.last_raw(), 0.3);
        gate.process(-0.9);
        assert_eq!(gate.last_raw(), -0.9);
    }

    #[test]
    fn test_bank_all_disabled_initially() {
        let mut bank = BypassBank::<4>::new_all_disabled();
        for i in 0..4 {
            assert_eq!(bank.process(i, 1.0), None);
        }
    }

    #[test]
    fn test_bank_enable_all() {
        let mut bank = BypassBank::<4>::new_all_disabled();
        bank.enable_all();
        for i in 0..4 {
            assert_eq!(bank.process(i, 0.5), Some(0.5));
        }
    }

    #[test]
    fn test_bank_enable_single_axis() {
        let mut bank = BypassBank::<4>::new_all_disabled();
        bank.enable(2);
        assert_eq!(bank.process(0, 1.0), None);
        assert_eq!(bank.process(1, 1.0), None);
        assert_eq!(bank.process(2, 1.0), Some(1.0));
        assert_eq!(bank.process(3, 1.0), None);
    }

    #[test]
    fn test_bank_out_of_bounds_returns_none() {
        let mut bank = BypassBank::<2>::new_all_enabled();
        assert_eq!(bank.process(5, 1.0), None);
    }

    #[test]
    fn test_bypass_config_default_disabled() {
        let config = BypassConfig::default();
        assert!(!config.enabled);
    }
}
