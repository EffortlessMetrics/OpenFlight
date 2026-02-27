@REQ-205 @product
Feature: Third-party plugins extend OpenFlight via defined plugin tiers  @AC-205.1
  Scenario: WASM plugins run sandboxed at 20 to 120 Hz with declared capabilities
    Given a WASM plugin with a declared capability manifest is installed
    When the plugin scheduler runs
    Then the plugin SHALL execute in a sandbox at a rate between 20 and 120 Hz  @AC-205.2
  Scenario: Native fast-path plugins use isolated helper process with shared-memory SPSC
    Given a native fast-path plugin is registered
    When the plugin communicates with the core
    Then communication SHALL occur via shared-memory SPSC in an isolated helper process  @AC-205.3
  Scenario: Service plugins run in managed thread with user consent
    Given a service plugin requests full access
    When user consent is granted
    Then the plugin SHALL run in a managed thread with full access enabled  @AC-205.4
  Scenario: Plugin capability manifest declares required axes buttons and FFB access
    Given a plugin provides a capability manifest
    When the manifest is validated at install time
    Then it SHALL declare all required axis, button, and FFB resource access  @AC-205.5
  Scenario: Misbehaving plugin terminated without affecting RT spine
    Given a plugin that exceeds its per-tick CPU budget or crashes
    When the plugin runtime detects the misbehaviour
    Then the plugin SHALL be terminated and the RT spine SHALL continue unaffected  @AC-205.6
  Scenario: Old plugins rejected gracefully on versioned API break
    Given a plugin compiled against an older plugin API version
    When the plugin is loaded after an API-breaking upgrade
    Then the plugin SHALL be rejected with a descriptive version mismatch error
