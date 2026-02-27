@REQ-488 @product
Feature: FFB DirectInput Backend — Windows DirectInput8 Force Feedback  @AC-488.1
  Scenario: FFB backend uses DirectInput8 on Windows when devices are present
    Given a Windows system with an FFB-capable device connected
    When the FFB engine initialises
    Then it SHALL select the DirectInput8 backend  @AC-488.2
  Scenario: DirectInput effects map to FFB engine effect types
    Given the DirectInput8 backend is active
    When an FFB engine effect of any supported type is submitted
    Then it SHALL be translated to a corresponding DirectInput effect type  @AC-488.3
  Scenario: Gain and envelope parameters are applied via DirectInput
    Given an active DirectInput8 FFB effect
    When gain or envelope parameters are updated
    Then the parameters SHALL be forwarded to the DirectInput device  @AC-488.4
  Scenario: Fallback to stub backend when no FFB devices are connected
    Given a Windows system with no FFB-capable devices connected
    When the FFB engine initialises
    Then it SHALL fall back to the stub backend without error
