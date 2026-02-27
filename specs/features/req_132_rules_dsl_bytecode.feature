@REQ-132 @infra
Feature: Rules DSL bytecode stability  @AC-132.1
  Scenario: Simple LED-on rule compiles to stable bytecode
    Given a rule "IF gear_down THEN led GearLed ON"
    When the rule is compiled twice independently
    Then both compilations SHALL produce byte-for-byte identical bytecode  @AC-132.2
  Scenario: AND condition compiles correctly
    Given a rule "IF gear_down AND speed_below_250 THEN led GearLed ON"
    When the rule is compiled
    Then the bytecode SHALL contain an AND opcode with two operand references  @AC-132.3
  Scenario: OR condition compiles correctly
    Given a rule "IF master_warning OR master_caution THEN led WarnLed BLINK"
    When the rule is compiled
    Then the bytecode SHALL contain an OR opcode with two operand references  @AC-132.4
  Scenario: Negation condition compiles correctly
    Given a rule "IF NOT autopilot_engaged THEN led ApLed OFF"
    When the rule is compiled
    Then the bytecode SHALL contain a NOT opcode wrapping the condition operand  @AC-132.5
  Scenario: Chained actions produce separate entries
    Given a rule with two actions "THEN led GearLed ON; led SpeedBrakeLed OFF"
    When the rule is compiled
    Then the bytecode action list SHALL contain exactly two separate action entries  @AC-132.6
  Scenario: Empty ruleset produces empty bytecode
    Given an empty ruleset with no rules
    When the ruleset is compiled
    Then the resulting bytecode SHALL be empty with zero instructions  @AC-132.7
  Scenario: Bytecode decompiles to equivalent rule
    Given a compiled bytecode for "IF gear_down THEN led GearLed ON"
    When the bytecode is decompiled back to a rule representation
    Then the decompiled rule SHALL be semantically equivalent to the original  @AC-132.8
  Scenario: Re-compilation of same rule produces identical bytecode
    Given a rule "IF flaps_full AND speed_below_180 THEN led FlapLed ON"
    When the rule is compiled ten times in succession
    Then every compilation output SHALL be byte-for-byte identical
