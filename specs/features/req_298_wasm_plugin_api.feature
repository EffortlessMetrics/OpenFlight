@REQ-298 @product
Feature: OpenFlight Plugin API (WASM)  @AC-298.1
  Scenario: Plugins are WASM modules loaded at runtime
    Given a WASM plugin file is present in the configured plugin directory
    When the service starts or detects the new plugin
    Then the plugin SHALL be loaded as a WASM module at runtime without recompiling the host  @AC-298.2
  Scenario: Plugin API exposes axis read write button read and event subscription
    Given a WASM plugin is loaded and active
    When the plugin calls the provided API
    Then it SHALL be able to read and write axis values, read button states, and subscribe to service events  @AC-298.3
  Scenario: Plugin execution is sandboxed with no host filesystem access
    Given a WASM plugin attempts to access the host filesystem
    When the plugin executes the filesystem access
    Then the sandbox SHALL deny the access and the plugin SHALL receive an error without the host being affected  @AC-298.4
  Scenario: Plugin panic does not crash the host service
    Given a WASM plugin encounters an unhandled error causing a panic
    When the panic propagates inside the WASM sandbox
    Then the host service SHALL remain running and log the plugin failure without crashing  @AC-298.5
  Scenario: Plugin is rate-limited to its declared Hz up to a maximum of 120 Hz
    Given a WASM plugin declares a desired execution rate of 200 Hz
    When the plugin scheduler runs
    Then the plugin SHALL be invoked at most 120 times per second  @AC-298.6
  Scenario: Plugin metadata includes name version and capability declarations
    Given a WASM plugin is loaded
    When its metadata is inspected
    Then the metadata SHALL contain at minimum a name, a version string, and a list of capability declarations
