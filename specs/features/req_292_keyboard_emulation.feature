@REQ-292 @product
Feature: Keyboard Emulation  @AC-292.1
  Scenario: Service emits virtual keystrokes via profile action
    Given a profile with a keystroke action "ctrl+shift+b" bound to a button press
    When the button is pressed
    Then the keystroke "ctrl+shift+b" SHALL be injected into the OS input stream  @AC-292.2
  Scenario: Keystroke action triggered by axis threshold
    Given a profile with a keystroke action bound to axis crossing threshold 0.9
    When the axis value rises above 0.9
    Then the configured keystroke SHALL be emitted exactly once  @AC-292.3
  Scenario: Key mappings use standard key names
    Given a profile action specifying key name "ctrl+alt+delete"
    When the profile is validated at load time
    Then the service SHALL accept the key name without error  @AC-292.4
  Scenario: Keystrokes do not repeat unless explicitly configured
    Given a profile keystroke action with repeat not configured
    When the triggering button is held down continuously
    Then the keystroke SHALL be emitted only once until the button is released and pressed again  @AC-292.5
  Scenario: Virtual keyboard is disabled by default
    Given a newly created profile with no explicit keyboard emulation settings
    When the profile is loaded
    Then the virtual keyboard feature SHALL be inactive until explicitly enabled in the profile  @AC-292.6
  Scenario: Key emit does not block axis processing
    Given the service is processing axes at 250Hz and a keystroke action is triggered
    When the keystroke is queued for emission
    Then axis processing SHALL continue without measurable latency increase and the keystroke SHALL be delivered asynchronously
