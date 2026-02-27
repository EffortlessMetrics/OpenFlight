@REQ-103 @REQ-104 @REQ-105 @product
Feature: Rules DSL condition parsing, action parsing, and schema validation

  # REQ-103: Rules condition parsing

  @AC-103.1
  Scenario: Plain boolean variable condition parses as Boolean condition
    Given a rules condition string "gear_down"
    When the condition is parsed
    Then parsing SHALL succeed
    And the result SHALL be a Boolean condition with variable "gear_down" and negate false

  @AC-103.2
  Scenario: Numeric greater-than condition parses as Compare condition
    Given a rules condition string "altitude > 1000"
    When the condition is parsed
    Then parsing SHALL succeed
    And the result SHALL be a Compare condition with operator Greater and value 1000.0

  @AC-103.3
  Scenario: Empty condition in a rule schema fails validation
    Given a rules schema with version "flight.ledmap/1" containing a rule with an empty condition
    When the schema is validated
    Then validation SHALL fail with a condition error

  @AC-103.4
  Scenario: Negated boolean variable condition parses with negate flag set
    Given a rules condition string "!gear_down"
    When the condition is parsed
    Then parsing SHALL succeed
    And the result SHALL be a Boolean condition with variable "gear_down" and negate true

  @AC-103.5
  Scenario: AND compound condition parses as And variant
    Given a rules condition string "gear_down and ias < 250"
    When the condition is parsed
    Then parsing SHALL succeed
    And the result SHALL be an And condition with two sub-conditions

  @AC-103.6
  Scenario: OR compound condition parses as Or variant
    Given a rules condition string "gear_down or flaps >= 0.5"
    When the condition is parsed
    Then parsing SHALL succeed
    And the result SHALL be an Or condition with two sub-conditions

  @AC-103.7
  Scenario Outline: All six comparison operators parse correctly
    Given a rules condition string "<condition>"
    When the condition is parsed
    Then parsing SHALL succeed
    And the result SHALL be a Compare condition with operator <operator>

    Examples:
      | condition  | operator     |
      | ias > 200  | Greater      |
      | ias < 200  | Less         |
      | ias == 200 | Equal        |
      | ias != 200 | NotEqual     |
      | ias >= 200 | GreaterEqual |
      | ias <= 200 | LessEqual    |

  # REQ-104: Rules action parsing

  @AC-104.1
  Scenario: Panel LED on action parses as LedOn
    Given a rules action string "led.panel('GEAR').on()"
    When the action is parsed
    Then parsing SHALL succeed
    And the result SHALL be a LedOn action with target "GEAR"

  @AC-104.2
  Scenario: Panel LED blink action parses as LedBlink
    Given a rules action string "led.panel('STALL').blink(rate_hz=4)"
    When the action is parsed
    Then parsing SHALL succeed
    And the result SHALL be a LedBlink action with target "STALL" and rate 4 Hz

  @AC-104.3
  Scenario: Empty action string returns parse error
    Given a rules action string ""
    When the action is parsed
    Then parsing SHALL fail with an unsupported action syntax error

  @AC-104.4
  Scenario: Unknown action syntax returns parse error
    Given a rules action string "set_led(GEAR, ON)"
    When the action is parsed
    Then parsing SHALL fail with an unsupported action syntax error

  @AC-104.5
  Scenario: Indexer LED blink action parses correctly
    Given a rules action string "led.indexer.blink(rate_hz=6)"
    When the action is parsed
    Then parsing SHALL succeed
    And the result SHALL be a LedBlink action with target "indexer" and rate 6 Hz

  @AC-104.6
  Scenario: Panel LED brightness action parses as LedBrightness
    Given a rules action string "led.panel('WARN').brightness(0.75)"
    When the action is parsed
    Then parsing SHALL succeed
    And the result SHALL be a LedBrightness action with target "WARN" and brightness 0.75

  # REQ-105: Rules schema validation

  @AC-105.1
  Scenario: Schema with supported version and valid rule passes validation
    Given a rules schema with version "flight.ledmap/1" and a rule with condition "gear == DOWN" and action "led.panel('GEAR').on()"
    When the schema is validated
    Then validation SHALL succeed

  @AC-105.2
  Scenario: Schema with unsupported version fails validation
    Given a rules schema with version "flight.ledmap/2" and no rules
    When the schema is validated
    Then validation SHALL fail with a version error

  @AC-105.3
  Scenario: Rule with empty condition fails validation
    Given a rules schema with version "flight.ledmap/1" containing a rule with an empty condition
    When the schema is validated
    Then validation SHALL fail with a condition error

  @AC-105.4
  Scenario: Rule with empty action fails validation
    Given a rules schema with version "flight.ledmap/1" containing a rule with an empty action
    When the schema is validated
    Then validation SHALL fail with an action error

  @AC-105.5
  Scenario: Rule with invalid action syntax fails validation
    Given a rules schema with version "flight.ledmap/1" and a rule with condition "gear_down" and action "set_led(GEAR, ON)"
    When the schema is validated
    Then validation SHALL fail with an invalid action error

  @AC-105.6
  Scenario: Multiple valid rules all compile to non-empty bytecode
    Given a rules schema with version "flight.ledmap/1" containing multiple valid rules
    When the schema is compiled
    Then compilation SHALL succeed
    And the bytecode program SHALL contain instructions for all rules
