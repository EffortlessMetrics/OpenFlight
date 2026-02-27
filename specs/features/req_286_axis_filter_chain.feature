@REQ-286 @product
Feature: Axis filter chain with ordered stages, skippable steps, O(n) allocation-free application, and validation  @AC-286.1
  Scenario: Multiple filters can be chained in configurable order
    Given a profile defining a filter chain with deadzone, curve, and rate limiter stages
    When an axis value is processed through the chain
    Then all three filter stages SHALL be applied in the sequence defined in the profile  @AC-286.2
  Scenario: Default chain order follows the canonical pipeline sequence
    Given a profile with all filter stages enabled and no explicit ordering override
    When the filter chain is inspected at runtime
    Then the stage order SHALL be deadzone then curve then EMA smoothing then rate limiter then trim  @AC-286.3
  Scenario: Chain can skip stages via profile config
    Given a profile that disables the EMA smoothing and rate limiter stages
    When an axis value is processed
    Then only the deadzone, curve, and trim stages SHALL be applied and the disabled stages SHALL be bypassed  @AC-286.4
  Scenario: Chain is applied in O(n) time with no allocation
    Given the RT axis pipeline is executing a filter chain of n stages
    When the chain processes an axis value
    Then no heap allocation SHALL occur during chain application and processing time SHALL scale linearly with n  @AC-286.5
  Scenario: Chain configuration is validated on load
    Given a profile containing a filter chain with an unrecognised stage identifier
    When the profile is loaded by the service
    Then loading SHALL fail with a validation error identifying the unknown stage  @AC-286.6
  Scenario: Partial chain produces expected output for each active stage
    Given a profile enabling only the deadzone and trim stages
    When a raw axis value is processed through the partial chain
    Then the output SHALL reflect deadzone removal applied first followed by trim offset with no other transformations
