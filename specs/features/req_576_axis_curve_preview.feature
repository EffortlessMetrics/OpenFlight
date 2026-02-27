@REQ-576 @product
Feature: Axis Curve Editor Preview — Service should provide axis curve preview data for UI tools  @AC-576.1
  Scenario: GetAxisCurvePreview RPC returns output for 100 evenly-spaced inputs
    Given a configured axis with a curve stage
    When the GetAxisCurvePreview RPC is called
    Then the response SHALL contain exactly 100 output values corresponding to evenly-spaced inputs from -1.0 to 1.0  @AC-576.2
  Scenario: Preview data includes all applied stages
    Given an axis with deadzone, curve, and sensitivity stages configured
    When GetAxisCurvePreview is called
    Then the response SHALL reflect the combined output of all pipeline stages  @AC-576.3
  Scenario: Preview reflects current active profile
    Given the active profile has been changed to a different curve configuration
    When GetAxisCurvePreview is called
    Then the response SHALL reflect the newly active profile configuration  @AC-576.4
  Scenario: Preview is generated in under 1ms
    Given the axis engine is running
    When GetAxisCurvePreview is called
    Then the preview data SHALL be generated and returned in under 1 millisecond
