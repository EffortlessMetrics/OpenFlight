@REQ-571 @product
Feature: Axis Engine Watchdog — Axis engine should have a watchdog for stuck-tick detection  @AC-571.1
  Scenario: Watchdog fires when axis tick exceeds 10ms wall time
    Given the axis engine is running
    When a single tick takes longer than 10ms wall time
    Then the watchdog SHALL fire and record the violation  @AC-571.2
  Scenario: Stuck tick is logged with timestamp and axis state
    Given the watchdog has fired due to a stuck tick
    When the event is processed
    Then a log entry SHALL be written containing the timestamp and axis state at the time of the stuck tick  @AC-571.3
  Scenario: Watchdog resets axis engine to last known good state
    Given the watchdog has fired
    When the recovery procedure runs
    Then the axis engine SHALL be reset to the last known good state  @AC-571.4
  Scenario: Watchdog event is published on flight-bus
    Given the watchdog has fired and recovery is complete
    When the bus tick processes the event queue
    Then a watchdog event SHALL be published on the flight-bus
