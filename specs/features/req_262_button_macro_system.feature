@REQ-262 @product
Feature: Button macro system triggers actions on press, chord, and hold  @AC-262.1

  Scenario: Single button press triggers immediate action
    Given a macro is configured on button 1 with action type "immediate"
    When button 1 is pressed
    Then the configured action SHALL fire within one RT tick  @AC-262.1

  Scenario: Multi-button chord triggers combined action
    Given a macro is configured on the chord of buttons 1 and 2
    When buttons 1 and 2 are pressed simultaneously within the chord window
    Then the chord action SHALL fire and individual button actions SHALL be suppressed  @AC-262.2

  Scenario: Button hold triggers repeat action after configured duration
    Given a macro is configured on button 3 with a hold duration of 500 ms and action type "repeat"
    When button 3 is held continuously for 600 ms
    Then the repeat action SHALL have fired at least once after the hold threshold  @AC-262.3

  Scenario: Macro action modifies axis output value
    Given a macro is configured on button 4 with action type "axis-set" targeting axis "throttle" to 0.0
    When button 4 is pressed
    Then the "throttle" axis output SHALL read 0.0 on the bus  @AC-262.4

  Scenario: Macro disabled in profile suppresses its action
    Given a macro on button 5 is present in the profile but marked disabled
    When button 5 is pressed
    Then no action SHALL fire and the macro state machine SHALL remain idle  @AC-262.5

  Scenario: Macro timing is deterministic within one RT tick
    Given a macro with a 200 ms hold threshold running at 250 Hz
    When the hold threshold is crossed
    Then the action SHALL fire within ±4 ms (one RT tick) of the configured threshold  @AC-262.6
