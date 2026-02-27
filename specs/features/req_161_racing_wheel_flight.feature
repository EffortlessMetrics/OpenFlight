@REQ-161 @product
Feature: G27/G29 racing wheel flight use

  @AC-161.1
  Scenario: G27 steering wheel axis normalized to [-1.0, 1.0]
    Given a Logitech G27 racing wheel is connected and bound for flight use
    When the steering wheel is moved across its full range
    Then the axis value SHALL be normalized to the range [-1.0, 1.0]

  @AC-161.2
  Scenario: G27 accelerator pedal normalized to [0.0, 1.0]
    Given a Logitech G27 racing wheel is connected and bound for flight use
    When the accelerator pedal is depressed across its full range
    Then the axis value SHALL be normalized to the range [0.0, 1.0]

  @AC-161.3
  Scenario: G29 steering axis 16-bit resolution
    Given a Logitech G29 racing wheel is connected and bound for flight use
    When the steering wheel is moved
    Then the axis SHALL be decoded at 16-bit resolution

  @AC-161.4
  Scenario: G29 pedal axes decoded
    Given a Logitech G29 racing wheel is connected and bound for flight use
    When the clutch, brake, and accelerator pedals are actuated
    Then each pedal axis SHALL produce an independent normalized value

  @AC-161.5
  Scenario: Shift paddles decoded as buttons
    Given a Logitech G27 or G29 racing wheel is connected and bound for flight use
    When a shift paddle is actuated
    Then the paddle SHALL be decoded as a button press event

  @AC-161.6
  Scenario: Profile remaps wheel to pitch/bank axes
    Given a Logitech G27 or G29 racing wheel is connected
    And a flight profile mapping the wheel axis to pitch and bank is loaded
    When the steering wheel is moved
    Then the pitch and bank sim variables SHALL be updated accordingly
