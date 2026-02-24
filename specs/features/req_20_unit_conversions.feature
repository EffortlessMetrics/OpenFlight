@REQ-20
Feature: Unit conversions and angle normalization

  @AC-20.1
  Scenario: Normalize degrees to signed range
    Given angle values 270, -270, and 360 degrees
    When each is normalized to the signed range
    Then 270 SHALL become -90
    And -270 SHALL become 90
    And 360 SHALL become 0

  @AC-20.1
  Scenario: Normalize degrees to unsigned range
    Given angle values -90, 360, and 450 degrees
    When each is normalized to the unsigned range
    Then -90 SHALL become 270
    And 360 SHALL become 0
    And 450 SHALL become 90

  @AC-20.2
  Scenario: Convert between kph and mps
    Given a speed of 36 kph
    When converted to meters per second
    Then the result SHALL be approximately 10 mps
    And converting back SHALL recover 36 kph

  @AC-20.2
  Scenario: Knots-to-mps round-trip preserves value
    Given any speed in knots
    When converted to mps and back to knots
    Then the result SHALL equal the original value within 0.1% tolerance

  @AC-20.2
  Scenario: kph-to-mps round-trip preserves value
    Given any speed in kph
    When converted to mps and back to kph
    Then the result SHALL equal the original value within 0.1% tolerance

  @AC-20.3
  Scenario: Feet-to-meters round-trip preserves altitude
    Given any altitude in feet
    When converted to meters and back to feet
    Then the result SHALL equal the original value within 0.1% tolerance
