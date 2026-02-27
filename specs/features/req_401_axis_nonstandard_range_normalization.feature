@REQ-401 @product
Feature: Axis Input Normalization for Non-Standard Ranges — Handle 0–65535 or 0–1023 Inputs

  @AC-401.1
  Scenario: Any [min, max] integer range is mapped to [-1.0, 1.0]
    Given an axis with a configured integer input range [min, max]
    When a value within that range is received
    Then the normalised output SHALL be in [-1.0, 1.0]

  @AC-401.2
  Scenario: Center detection uses the logical center from the HID descriptor
    Given a HID descriptor that specifies a logical center value
    When the axis normalisation center is computed
    Then it SHALL use the logical center from the descriptor

  @AC-401.3
  Scenario: Non-symmetric ranges are handled correctly
    Given an axis whose logical center is not at (max-min)/2
    When values around the center are normalised
    Then the output SHALL correctly reflect the asymmetric range

  @AC-401.4
  Scenario: Common bit-depth ranges are all supported
    Given axes with 10-bit, 12-bit, 14-bit, and 16-bit input ranges
    When values across each range are normalised
    Then all four bit-depth ranges SHALL produce valid [-1.0, 1.0] output

  @AC-401.5
  Scenario: Out-of-range values are clamped and counted
    Given an axis with a configured input range
    When a value outside that range is received
    Then it SHALL be clamped to the range boundary and the out-of-range counter SHALL be incremented

  @AC-401.6
  Scenario: Property test — any value in [0, 65535] produces output in [-1.0, 1.0]
    Given a 16-bit axis normaliser
    When any integer value in [0, 65535] is normalised
    Then the output SHALL always be in [-1.0, 1.0]
