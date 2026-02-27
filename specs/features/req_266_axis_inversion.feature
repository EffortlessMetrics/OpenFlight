@REQ-266 @product
Feature: Axis inversion remaps output symmetrically and is persisted in profile  @AC-266.1

  Scenario: Inverted axis maps 1.0 to -1.0 and -1.0 to 1.0
    Given an axis is configured with inversion enabled
    When the physical axis reads 1.0
    Then the output SHALL be -1.0, and when the physical axis reads -1.0 the output SHALL be 1.0

  Scenario: Inversion applied after deadzone and before curve
    Given an axis pipeline with deadzone, inversion, and curve stages configured
    When a raw value is processed through the full pipeline
    Then the inversion stage SHALL execute after deadzone and before the curve is applied  @AC-266.2

  Scenario: Inversion is configurable per axis in profile YAML
    Given a profile YAML with invert: true on the "pitch" axis and invert: false on the "roll" axis
    When the profile is loaded
    Then pitch output SHALL be inverted and roll output SHALL not be inverted  @AC-266.3

  Scenario: Inversion toggled at runtime via IPC API
    Given an axis with inversion currently disabled
    When the operator sends a SetAxisInvert IPC call to enable inversion
    Then the next tick SHALL produce inverted output without requiring a profile reload  @AC-266.4

  Scenario: Inverted axis output stays within valid range
    Given an axis configured with inversion enabled
    When the physical axis sweeps from -1.0 to 1.0
    Then all output samples SHALL remain within the closed interval [-1.0, 1.0]  @AC-266.5

  Scenario: Inversion state persisted on profile save
    Given inversion was toggled at runtime for the "throttle" axis
    When the profile is saved via the IPC SaveProfile call
    Then the saved profile YAML SHALL contain invert: true for the "throttle" axis  @AC-266.6
