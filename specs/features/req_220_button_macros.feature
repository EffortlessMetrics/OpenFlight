@REQ-220 @product
Feature: Button macros allow single button to trigger complex action sequences  @AC-220.1
  Scenario: Macro defined as sequence of events with optional delays
    Given a macro definition containing a sequence of events with inter-step delays
    When the macro definition is validated at profile load time
    Then the macro SHALL be accepted and all steps including delays SHALL be stored  @AC-220.2
  Scenario: Macro triggered on button press not hold or release
    Given a button with a macro assigned in the active profile
    When the button is pressed once
    Then the macro SHALL execute exactly once and SHALL NOT execute on hold or release  @AC-220.3
  Scenario: Maximum macro length of 32 steps enforced
    Given a macro definition containing more than 32 steps
    When the profile containing the macro is loaded
    Then the service SHALL reject the macro with a validation error citing the 32-step maximum  @AC-220.4
  Scenario: Macro in progress cancelled by second press
    Given a long macro is currently executing and has not yet completed
    When the trigger button is pressed a second time
    Then the macro SHALL be cancelled immediately and remaining steps SHALL not execute  @AC-220.5
  Scenario: Macros isolated per profile with no cross-profile bleeding
    Given two profiles each defining different macros on the same button
    When the active profile is switched from the first to the second
    Then only the macro from the newly active profile SHALL execute on subsequent button press  @AC-220.6
  Scenario: Macro execution does not block RT spine tick
    Given a macro containing delays totalling longer than one RT tick period
    When the macro is triggered during normal RT operation
    Then the RT spine SHALL continue ticking at 250 Hz with no measurable jitter increase
