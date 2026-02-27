@REQ-341 @product
Feature: WASM Plugin Sandbox  @AC-341.1
  Scenario: WASM plugins cannot access the host filesystem
    Given a WASM plugin that attempts to open a host filesystem path
    When the plugin is executed inside the sandbox
    Then the filesystem access SHALL be denied and the plugin SHALL receive an error  @AC-341.2
  Scenario: WASM plugins cannot make network calls
    Given a WASM plugin that attempts to open a network socket
    When the plugin is executed inside the sandbox
    Then the network call SHALL be blocked and the plugin SHALL receive an error  @AC-341.3
  Scenario: WASM plugins are killed when they exceed their CPU budget
    Given a WASM plugin configured with a CPU budget of 5ms per tick
    When the plugin's execution time exceeds 5ms during a tick
    Then the runtime SHALL terminate the plugin instance and log a budget-exceeded event  @AC-341.4
  Scenario: WASM plugin crash is isolated from the host process
    Given a WASM plugin that triggers a fatal trap (e.g., out-of-bounds memory access)
    When the plugin crashes
    Then the host service SHALL remain running and log the plugin crash without propagating the fault  @AC-341.5
  Scenario: Plugin capabilities are declared at install time
    Given a WASM plugin package that declares the "axis-read" and "led-write" capabilities
    When the plugin is installed
    Then the runtime SHALL record those capabilities and deny any API call outside the declared set  @AC-341.6
  Scenario: Plugin manifest is signed by the developer
    Given a WASM plugin whose manifest signature does not match the package content
    When the user attempts to install the plugin
    Then the service SHALL reject the installation with a signature verification failure
