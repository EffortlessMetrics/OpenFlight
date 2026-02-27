@REQ-240 @product
Feature: Service reports per-axis capabilities and current operational limits  @AC-240.1
  Scenario: Capability report lists all active axes with resolution and range
    Given the service has active axes configured
    When a capability report is requested
    Then the report SHALL list every active axis with its bit resolution and value range  @AC-240.2
  Scenario: Capability report includes per-axis clamp count since service start
    Given an axis that has been clamped at least once since service start
    When the capability report is retrieved
    Then the report SHALL include the cumulative clamp count for that axis  @AC-240.3
  Scenario: Capability report indicates whether demo or kid mode limits are active
    Given the service is running with kid mode limits enabled on one axis
    When the capability report is retrieved
    Then the report SHALL indicate kid mode is active for that axis and inactive for others  @AC-240.4
  Scenario: Capability report lists filter chain stages currently applied per axis
    Given an axis with a deadzone filter and a curve filter applied
    When the capability report is retrieved
    Then the report SHALL list deadzone and curve as the filter chain stages for that axis  @AC-240.5
  Scenario: Capability report available via gRPC GetCapabilities call
    Given the service gRPC interface is running
    When a client calls GetCapabilities
    Then the service SHALL return a populated capability report with zero errors  @AC-240.6
  Scenario: Capability report updated within 100ms of profile hot-swap
    Given the service is running and a capability report has been retrieved
    When a profile hot-swap completes
    Then a subsequent GetCapabilities call within 100ms SHALL reflect the updated configuration
