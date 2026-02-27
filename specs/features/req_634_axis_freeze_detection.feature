Feature: Axis Engine Freeze Detection
  As a flight simulation enthusiast
  I want the axis engine to detect and recover from processing freeze
  So that my controls remain responsive even after unexpected freezes

  Background:
    Given the OpenFlight service is running

  Scenario: Watchdog thread detects if axis tick has not run in 50ms
    Given the axis engine watchdog is enabled
    When the axis tick does not execute for 50 milliseconds
    Then the watchdog detects the freeze and triggers recovery

  Scenario: Freeze detection logs the current tick state
    Given a freeze is detected
    Then the current tick state is written to the service log

  Scenario: Recovery restarts the axis thread with last known good state
    Given a freeze has been detected
    When recovery is initiated
    Then the axis thread restarts using the last known good state

  Scenario: Freeze event count is tracked in metrics
    When a freeze and recovery cycle completes
    Then the freeze event counter in metrics is incremented
