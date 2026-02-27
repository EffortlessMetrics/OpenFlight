@REQ-158 @product
Feature: Tactile feedback device  @AC-158.1
  Scenario: Bass shaker receives frequency envelope
    Given a tactile feedback device is connected and bound
    When a frequency envelope is submitted to the tactile engine
    Then the device SHALL receive the corresponding low-frequency signal output  @AC-158.2
  Scenario: Vibration intensity scales with the control parameter
    Given a tactile feedback device is connected and bound
    When the intensity parameter is varied across its full range
    Then the device output amplitude SHALL scale proportionally  @AC-158.3
  Scenario: Multiple tactile zones are independently controlled
    Given a tactile feedback device with multiple output zones is connected
    When distinct effects are sent to different zones simultaneously
    Then each zone SHALL reproduce its assigned effect independently  @AC-158.4
  Scenario: Effect stops on device disconnect
    Given a tactile feedback device is actively producing an effect
    When the device is disconnected
    Then the effect SHALL cease and no further output commands SHALL be issued  @AC-158.5
  Scenario: Low-frequency signal below 100 Hz is prioritised
    Given a tactile feedback device is connected
    When a mix of signals above and below 100 Hz is submitted
    Then signals below 100 Hz SHALL be prioritised in the output rendering  @AC-158.6
  Scenario: Device is identified and bound to the correct channel
    Given a tactile feedback device is connected
    When the device binding process runs
    Then the device SHALL be assigned to the correct output channel based on its identifier
