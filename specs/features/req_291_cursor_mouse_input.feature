@REQ-291 @product
Feature: Cursor/Mouse Input Integration  @AC-291.1
  Scenario: Service exposes a virtual mouse cursor controlled by joystick axes
    Given a profile with virtual mouse cursor enabled and two joystick axes mapped to X and Y mouse movement
    When the joystick axes are deflected
    Then the virtual mouse cursor SHALL move proportionally on screen  @AC-291.2
  Scenario: Joystick axis to mouse movement mapping is configurable
    Given a profile with virtual mouse sensitivity set to 2.5 and acceleration enabled
    When the mapped joystick axis is deflected at half travel
    Then the cursor movement speed SHALL reflect the configured sensitivity and acceleration curve  @AC-291.3
  Scenario: Virtual cursor can be toggled on/off per profile
    Given profile A has virtual mouse enabled and profile B has it disabled
    When the active profile is switched from A to B
    Then joystick axis movements SHALL no longer move the virtual cursor  @AC-291.4
  Scenario: Mouse click can be mapped to joystick button
    Given a profile mapping joystick button 1 to left mouse click
    When the joystick button is pressed
    Then a left mouse click event SHALL be injected at the current cursor position  @AC-291.5
  Scenario: Virtual mouse does not conflict with real mouse input
    Given the virtual mouse cursor is active and the user also moves the physical mouse
    When both inputs occur within the same frame
    Then neither input SHALL interfere with or suppress the other  @AC-291.6
  Scenario: Virtual mouse is available only on Windows and Linux
    Given the service is running on macOS
    When a profile that enables virtual mouse is loaded
    Then the service SHALL report that virtual mouse is unsupported on this platform and disable the feature gracefully
