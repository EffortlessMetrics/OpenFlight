Feature: Analog Throttle Detent Support
  As a flight simulation enthusiast
  I want the axis engine to support analog throttle detent zones
  So that special throttle positions like military power behave realistically

  Background:
    Given the OpenFlight service is running

  Scenario: Detent zones define special behavior at specific positions
    Given a profile with a detent zone configured at a specific axis position
    When the throttle axis enters the detent zone
    Then the special detent behavior is applied

  Scenario: Military power detent snaps axis to detent value when within threshold
    Given a military power detent is configured at a specific axis position
    When the throttle axis moves within the snap threshold of the detent
    Then the axis output snaps to the detent position value

  Scenario: Detent click events are published on flight-bus
    Given a detent zone is configured for a throttle axis
    When the throttle axis enters or exits the detent zone
    Then a detent click event is published on the flight-bus

  Scenario: Detents are configurable per-axis in profile
    When a detent zone is defined for a specific axis in the profile
    Then only that axis exhibits the detent behavior
