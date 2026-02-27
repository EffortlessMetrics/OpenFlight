Feature: Axis Feedback Loop Detection
  As a flight simulation enthusiast
  I want the service to detect and break axis feedback loops
  So that oscillating control inputs do not disrupt my flight simulation

  Background:
    Given the OpenFlight service is running

  Scenario: Feedback detector identifies oscillating axis output
    Given the feedback loop detector is enabled
    When an axis output oscillates rapidly beyond the detection threshold
    Then the feedback detector identifies the oscillation as a feedback loop

  Scenario: Detected feedback loop triggers automatic dampening
    Given a feedback loop has been detected on an axis
    When the detector activates its response
    Then automatic dampening is applied to reduce the oscillation

  Scenario: Feedback detection threshold is configurable
    When the feedback detection threshold is updated in the profile
    Then the detector uses the new threshold for subsequent loop detection

  Scenario: Feedback loop events are logged and reported
    Given a feedback loop has been detected
    When the event log is inspected
    Then feedback loop detection events are recorded with axis ID and timestamp
