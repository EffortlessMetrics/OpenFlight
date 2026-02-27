@REQ-212 @product
Feature: Standard gamepads mapped to flight control axes via profile  @AC-212.1
  Scenario: Left stick X and Y mapped to roll and pitch by default
    Given a standard gamepad connected with the default gamepad profile active
    When the left stick is moved along the X and Y axes
    Then the X axis SHALL control roll and the Y axis SHALL control pitch  @AC-212.2
  Scenario: Triggers mapped to throttle combined or split
    Given a gamepad profile configured for throttle mapping
    When the L2 or R2 triggers are pressed
    Then the triggers SHALL be mapped to throttle either as a combined or split axis as configured  @AC-212.3
  Scenario: Gamepad profile specifies axis response curves for analog precision
    Given a gamepad profile that includes axis response curve definitions
    When analog input is processed
    Then the configured response curves SHALL be applied to each axis for precision control  @AC-212.4
  Scenario: Gamepad works without FFB hardware and degrades gracefully
    Given a gamepad profile active with no FFB hardware connected
    When the service initialises
    Then FFB features SHALL be silently skipped and the gamepad SHALL function for all non-FFB axes  @AC-212.5
  Scenario: Button hold mapped to repeated trim increment or decrement
    Given a gamepad profile with a button mapped to trim increment
    When the button is held continuously
    Then repeated trim increment events SHALL be generated at the configured repeat rate  @AC-212.6
  Scenario: Profile supports multiple gamepad models without manual rebinding
    Given a gamepad profile defining mappings compatible with multiple gamepad models
    When any of the supported gamepad models is connected
    Then the correct mapping SHALL be applied automatically without requiring manual rebinding
