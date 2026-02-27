@REQ-577 @product
Feature: Device Auto-Bind on Connect — Service should auto-bind newly connected devices using config  @AC-577.1
  Scenario: New device matches against stored auto-bind rules
    Given auto-bind rules are configured for a device with a specific VID and PID
    When that device is connected
    Then the service SHALL match the device against the stored auto-bind rules  @AC-577.2
  Scenario: Auto-bind applies axis and button config from matching rule
    Given a device connects and matches an auto-bind rule
    When the auto-bind rule is applied
    Then the axis and button configuration from the matching rule SHALL be active for the device  @AC-577.3
  Scenario: Auto-bind can be disabled globally or per-device
    Given auto-bind is disabled in the service configuration
    When a device connects that would otherwise match an auto-bind rule
    Then no auto-bind SHALL be applied and the device SHALL remain unconfigured  @AC-577.4
  Scenario: Auto-bind result is published as a bus event
    Given auto-bind completes for a newly connected device
    When the bus processes the event queue
    Then an auto-bind result event SHALL be published on the flight-bus
