Feature: Axis Power-On State
  As a flight simulation enthusiast
  I want axis engine to define a power-on default state
  So that axes have predictable initial values before any device input arrives

  Background:
    Given the OpenFlight service has just started

  Scenario: All axes initialize to 0.0 on startup
    When the axis engine starts
    Then all axis output values are 0.0

  Scenario: Power-on state is held until first device input is received
    Given the axis engine has started with no device input
    When axis output is queried before any HID input is received
    Then the output value is the power-on default state

  Scenario: Power-on state is configurable per axis in profile
    Given a profile configures axis "throttle" with a power-on state of 0.0
    And a profile configures axis "elevator" with a power-on state of 0.0
    When the axis engine starts
    Then each axis initializes to its configured power-on state

  Scenario: Time to first input is logged on startup
    Given the axis engine starts
    When the first device input is received on any axis
    Then the elapsed time from startup to first input is logged
