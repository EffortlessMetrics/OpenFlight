@REQ-261 @product
Feature: Hat switch decoder produces correct (x,y) vector for all 8 directions  @AC-261.1

  Scenario: 8-way hat direction N produces (0.0, -1.0)
    Given a hat switch reports direction 0 (North)
    When the hat decoder processes the value
    Then the output vector SHALL be (0.0, -1.0)

  Scenario: Center neutral hat position produces (0.0, 0.0)
    Given a hat switch reports the center/neutral position
    When the hat decoder processes the value
    Then the output vector SHALL be (0.0, 0.0)  @AC-261.2

  Scenario: Diagonal NE direction produces correct normalised vector
    Given a hat switch reports direction 1 (North-East)
    When the hat decoder processes the value
    Then the output vector SHALL be approximately (0.707, -0.707)  @AC-261.3

  Scenario: Hat switch output drives a virtual axis
    Given a hat switch is configured to drive virtual axes X and Y
    When the hat switch reports direction 2 (East)
    Then the virtual axis X SHALL read 1.0 and virtual axis Y SHALL read 0.0 on the bus  @AC-261.4

  Scenario: Multiple hats on same device are decoded independently
    Given a device with two hat switches where hat 0 reports North and hat 1 reports South
    When both hat values are decoded in the same tick
    Then hat 0 output SHALL be (0.0, -1.0) and hat 1 output SHALL be (0.0, 1.0) independently  @AC-261.5

  Scenario: Out-of-range hat value is safely rejected
    Given a hat switch reports the out-of-range value 9
    When the hat decoder processes the value
    Then the decoder SHALL reject the value without panic and output SHALL remain at the last valid vector  @AC-261.6
