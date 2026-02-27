Feature: Axis Engine Power Efficiency Mode
  As a flight simulation enthusiast
  I want the axis engine to support a reduced-frequency mode
  So that power consumption is reduced when full performance is not needed

  Background:
    Given the OpenFlight service is running

  Scenario: Reduced-frequency mode runs axis engine at 60Hz instead of 250Hz
    When reduced-frequency power efficiency mode is activated
    Then the axis engine processes ticks at 60Hz instead of 250Hz

  Scenario: Mode is activatable when no sim is connected
    Given no simulator is connected
    When power efficiency mode is enabled
    Then the mode activates successfully

  Scenario: Transition between modes is seamless with no discontinuity
    Given the axis engine is running in 250Hz mode
    When the mode transitions to 60Hz power efficiency mode
    Then no axis position discontinuity is observed during the transition

  Scenario: Current mode is shown in service diagnostics
    When the service diagnostics endpoint is queried
    Then the response includes the current axis engine frequency mode
