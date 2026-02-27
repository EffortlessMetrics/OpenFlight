@REQ-328 @product
Feature: Multi-Seat Support  @AC-328.1
  Scenario: Service can connect two sets of controls
    Given pilot and co-pilot device sets are configured
    When the service starts
    Then the service SHALL accept input from both pilot and co-pilot devices simultaneously  @AC-328.2
  Scenario: Each seat has independent device bindings
    Given a multi-seat configuration
    When device bindings are reviewed
    Then pilot and co-pilot device bindings SHALL be stored and applied independently  @AC-328.3
  Scenario: Active seat can be switched at runtime
    Given the service is running with pilot as the active seat
    When the operator runs flightctl seat --set copilot
    Then the service SHALL switch axis output to the co-pilot device set without restart  @AC-328.4
  Scenario: Both seats share the same profile but can use different slots
    Given a profile with multiple slots
    When pilot and co-pilot are both active
    Then both SHALL use the shared profile but the operator MAY assign each seat to a different slot  @AC-328.5
  Scenario: Seat switch event is emitted on the bus
    Given the service is running in multi-seat mode
    When the active seat changes
    Then the service SHALL emit a SeatSwitched event on the flight-bus  @AC-328.6
  Scenario: Co-pilot devices are listed separately in diagnostics
    Given a diagnostic bundle generated in multi-seat mode
    When the bundle is inspected
    Then co-pilot devices SHALL be listed under a separate co-pilot section distinct from pilot devices
