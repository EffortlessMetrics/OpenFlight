/// Overall system degradation level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum DegradationLevel {
    /// All systems operational.
    Full,
    /// Some non-critical components have failed.
    Reduced,
    /// Multiple failures — only essential features available.
    Minimal,
    /// Critical failure — only safe-mode operations permitted.
    SafeMode,
}

/// Health status of a single component.
#[derive(Debug, Clone)]
pub struct ComponentStatus {
    pub name: String,
    pub healthy: bool,
    /// If `true`, failure of this component triggers `SafeMode`.
    pub critical: bool,
}

/// Tracks component health and derives the current degradation level.
pub struct DegradationManager {
    components: Vec<ComponentStatus>,
    level: DegradationLevel,
}

impl DegradationManager {
    #[must_use]
    pub fn new() -> Self {
        Self {
            components: Vec::new(),
            level: DegradationLevel::Full,
        }
    }

    /// Register a new component for health tracking.
    pub fn register_component(&mut self, name: impl Into<String>, critical: bool) {
        self.components.push(ComponentStatus {
            name: name.into(),
            healthy: true,
            critical,
        });
        self.recompute_level();
    }

    /// Update the health of a named component and recompute the level.
    pub fn update_health(&mut self, name: &str, healthy: bool) {
        if let Some(c) = self.components.iter_mut().find(|c| c.name == name) {
            c.healthy = healthy;
        }
        self.recompute_level();
    }

    /// Current system-wide degradation level.
    #[must_use]
    pub fn current_level(&self) -> DegradationLevel {
        self.level
    }

    /// Names of features/components that are currently degraded (unhealthy).
    #[must_use]
    pub fn degraded_features(&self) -> Vec<String> {
        self.components
            .iter()
            .filter(|c| !c.healthy)
            .map(|c| c.name.clone())
            .collect()
    }

    /// Whether the system can continue operating (not in `SafeMode`
    /// unless there are no registered components).
    #[must_use]
    pub fn can_operate(&self) -> bool {
        self.level != DegradationLevel::SafeMode
    }

    /// Returns a snapshot of all component statuses.
    #[must_use]
    pub fn components(&self) -> &[ComponentStatus] {
        &self.components
    }

    fn recompute_level(&mut self) {
        let unhealthy: Vec<&ComponentStatus> =
            self.components.iter().filter(|c| !c.healthy).collect();

        if unhealthy.iter().any(|c| c.critical) {
            self.level = DegradationLevel::SafeMode;
        } else {
            self.level = match unhealthy.len() {
                0 => DegradationLevel::Full,
                1 => DegradationLevel::Reduced,
                _ => DegradationLevel::Minimal,
            };
        }
    }
}

impl Default for DegradationManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_manager_is_full() {
        let m = DegradationManager::new();
        assert_eq!(m.current_level(), DegradationLevel::Full);
        assert!(m.can_operate());
    }

    #[test]
    fn single_non_critical_failure_reduces() {
        let mut m = DegradationManager::new();
        m.register_component("panels", false);
        m.update_health("panels", false);
        assert_eq!(m.current_level(), DegradationLevel::Reduced);
        assert!(m.can_operate());
    }

    #[test]
    fn multiple_non_critical_failures_minimal() {
        let mut m = DegradationManager::new();
        m.register_component("panels", false);
        m.register_component("streamdeck", false);
        m.update_health("panels", false);
        m.update_health("streamdeck", false);
        assert_eq!(m.current_level(), DegradationLevel::Minimal);
        assert!(m.can_operate());
    }

    #[test]
    fn critical_failure_triggers_safe_mode() {
        let mut m = DegradationManager::new();
        m.register_component("axis_engine", true);
        m.update_health("axis_engine", false);
        assert_eq!(m.current_level(), DegradationLevel::SafeMode);
        assert!(!m.can_operate());
    }

    #[test]
    fn recovery_restores_level() {
        let mut m = DegradationManager::new();
        m.register_component("hid", false);
        m.update_health("hid", false);
        assert_eq!(m.current_level(), DegradationLevel::Reduced);
        m.update_health("hid", true);
        assert_eq!(m.current_level(), DegradationLevel::Full);
    }

    #[test]
    fn degraded_features_lists_unhealthy() {
        let mut m = DegradationManager::new();
        m.register_component("a", false);
        m.register_component("b", false);
        m.register_component("c", false);
        m.update_health("b", false);
        let deg = m.degraded_features();
        assert_eq!(deg, vec!["b".to_owned()]);
    }

    #[test]
    fn no_degraded_when_all_healthy() {
        let mut m = DegradationManager::new();
        m.register_component("x", false);
        assert!(m.degraded_features().is_empty());
    }

    #[test]
    fn register_starts_healthy() {
        let mut m = DegradationManager::new();
        m.register_component("foo", true);
        assert!(m.components()[0].healthy);
        assert_eq!(m.current_level(), DegradationLevel::Full);
    }

    #[test]
    fn update_unknown_component_is_no_op() {
        let mut m = DegradationManager::new();
        m.register_component("a", false);
        m.update_health("nonexistent", false);
        assert_eq!(m.current_level(), DegradationLevel::Full);
    }

    #[test]
    fn level_ordering() {
        assert!(DegradationLevel::Full < DegradationLevel::Reduced);
        assert!(DegradationLevel::Reduced < DegradationLevel::Minimal);
        assert!(DegradationLevel::Minimal < DegradationLevel::SafeMode);
    }
}
