@REQ-232 @product
Feature: Rules DSL compiles and executes event→action mappings correctly  @AC-232.1
  Scenario: Rule condition uses event type and id comparison syntax
    Given a rules file containing the condition `event.type == "button_press" && event.id == 42`
    When the rule condition is parsed
    Then it SHALL be accepted as valid DSL syntax without error  @AC-232.2
  Scenario: Rule action uses set_axis with arithmetic expression
    Given a rules file containing the action `set_axis("pitch_trim", value + 0.01)`
    When the rule action is parsed
    Then it SHALL be accepted as valid DSL syntax without error  @AC-232.3
  Scenario: Rules are compiled to bytecode at profile load not at runtime
    Given a profile containing one or more DSL rules
    When the profile is loaded by the service
    Then all rules SHALL be compiled to bytecode before the RT spine begins processing  @AC-232.4
  Scenario: Compilation error includes line number and column
    Given a rules file containing a syntax error on a specific line
    When the rules are compiled
    Then the error message SHALL include the line number and column where the error was detected  @AC-232.5
  Scenario: Runtime bytecode executes within 10 microseconds per rule on RT spine
    Given a compiled rule is registered on the RT spine
    When the rule is evaluated during a 250Hz tick
    Then the execution time for that rule SHALL not exceed 10 microseconds  @AC-232.6
  Scenario: Rules DSL documented with working examples in docs reference
    Given the documentation directory docs/reference exists
    When it is inspected for DSL documentation
    Then it SHALL contain at least 10 working DSL rule examples with explanations
