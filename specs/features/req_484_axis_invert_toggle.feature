@REQ-484 @product
Feature: Axis Invert Toggle — CLI-Driven Axis Inversion  @AC-484.1
  Scenario: flightctl axis invert command toggles inversion for specified axis
    Given an axis that is currently not inverted
    When `flightctl axis invert <axis_id>` is executed
    Then the axis SHALL be marked as inverted and subsequent values SHALL be negated  @AC-484.2
  Scenario: Inversion state persists across service restarts
    Given an axis has been inverted via the CLI
    When the service is restarted
    Then the axis SHALL still be in the inverted state after restart  @AC-484.3
  Scenario: Inversion toggle takes effect on next axis tick
    Given an axis is actively processing input at 250Hz
    When the inversion toggle command is received
    Then the inverted output SHALL be applied no later than the following axis tick  @AC-484.4
  Scenario: Current inversion state is shown in flightctl axis status
    Given an axis with inversion enabled
    When `flightctl axis status <axis_id>` is executed
    Then the output SHALL clearly indicate that inversion is active for that axis
