@REQ-24
Feature: LED rules engine compilation and parsing

  @AC-24.1
  Scenario: Rules schema validates supported version
    Given a rules schema document with a supported version field
    When the schema is validated
    Then validation SHALL succeed

  @AC-24.1
  Scenario: Rules schema rejects unsupported version
    Given a rules schema document with an unsupported version field
    When the schema is validated
    Then validation SHALL fail with a version error

  @AC-24.2
  Scenario: Rule compilation emits valid bytecode
    Given a rule definition with a condition and an LED action
    When the rule is compiled to bytecode
    Then the emitted bytecode SHALL be non-empty
    And the bytecode SHALL contain the expected opcodes for the rule

  @AC-24.3
  Scenario: Condition parsing recognizes all supported types
    Given rule condition strings for all supported condition types
    When each condition is parsed
    Then parsing SHALL succeed and return the expected condition variant

  @AC-24.3
  Scenario: Action parsing recognizes all supported LED action types
    Given rule action strings for all supported LED action types
    When each action is parsed
    Then parsing SHALL succeed and return the expected action variant
