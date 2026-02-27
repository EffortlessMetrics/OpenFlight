@REQ-169 @product
Feature: FFB envelope limits

  @AC-169.1
  Scenario: Max force magnitude limit enforced
    Given an FFB device is active with a configured maximum force magnitude
    When a force effect requests a magnitude above the configured limit
    Then the output force magnitude SHALL be clamped to the configured maximum

  @AC-169.2
  Scenario: Max slew rate clamped to safe value
    Given an FFB device is active with a configured maximum slew rate
    When a force effect requests a change exceeding the maximum slew rate
    Then the force change rate SHALL be clamped to the configured safe slew rate limit

  @AC-169.3
  Scenario: Temperature threshold reduces max force
    Given an FFB device reports a temperature above the configured thermal threshold
    When a force effect is applied
    Then the maximum allowable force magnitude SHALL be reduced according to the thermal derating curve

  @AC-169.4
  Scenario: Envelope tested at all four device corners
    Given an FFB device is active
    When force effects are applied at all four axis extremes
    Then the envelope limits SHALL be enforced correctly at each corner position

  @AC-169.5
  Scenario: Force magnitude zero on disconnect
    Given an FFB device is active and outputting force
    When the device is disconnected
    Then the force output SHALL immediately drop to zero

  @AC-169.6
  Scenario: Force re-enabled on reconnect
    Given an FFB device was disconnected and has now reconnected
    When the device is re-initialized
    Then force effects SHALL be re-enabled and the envelope limits re-applied

  @AC-169.7
  Scenario: Envelope violation event logged
    Given the tracing subsystem is active
    When a force effect violates the configured envelope limits
    Then an envelope-violation event SHALL be logged with the requested and clamped values

  @AC-169.8
  Scenario: Emergency stop kills all effects immediately
    Given an FFB device is outputting one or more force effects
    When the emergency stop command is issued
    Then all force effects SHALL be cancelled and the output force SHALL be zero within one RT tick
