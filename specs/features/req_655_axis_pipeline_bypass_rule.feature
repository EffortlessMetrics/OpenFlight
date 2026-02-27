Feature: Axis Pipeline Bypassing by Rule
  As a flight simulation enthusiast
  I want profile rules to be able to bypass axis pipeline stages
  So that I can dynamically adjust axis processing behaviour at runtime

  Background:
    Given the OpenFlight service is running

  Scenario: Rule enables or disables a named pipeline stage at runtime
    Given a profile rule targeting a named axis pipeline stage
    When the rule condition is satisfied
    Then the named stage is enabled or disabled accordingly

  Scenario: Stage bypass takes effect within one axis tick
    Given a pipeline stage bypass rule is active
    When the rule triggers a stage bypass
    Then the bypass is reflected in the axis output within one 250Hz tick

  Scenario: Bypassed stage is indicated in axis diagnostics
    Given a pipeline stage has been bypassed by a rule
    When axis diagnostics are queried
    Then the bypassed stage is identified in the diagnostic output

  Scenario: Rule-driven bypass is documented in profile schema
    When the profile schema documentation is inspected
    Then rule-driven pipeline stage bypass options are described
