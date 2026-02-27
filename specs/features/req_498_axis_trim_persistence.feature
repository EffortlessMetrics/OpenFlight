@REQ-498 @product
Feature: Axis Trim Persistence — Trim Values Saved Across Restarts  @AC-498.1
  Scenario: Trim values are saved to calibration store on change
    Given an axis trim value is modified at runtime
    When the change is applied
    Then the new trim value SHALL be persisted to the calibration store immediately  @AC-498.2
  Scenario: Trim values are loaded from calibration store on service start
    Given trim values exist in the calibration store from a previous session
    When the service starts
    Then it SHALL load and apply the stored trim values to the corresponding axes  @AC-498.3
  Scenario: Trim reset clears both in-memory and stored values
    Given an axis has a non-zero trim value in memory and in the calibration store
    When a trim reset command is issued for that axis
    Then the in-memory trim SHALL be zeroed and the stored value SHALL be removed  @AC-498.4
  Scenario: Trim values are included in diagnostic bundle
    Given the service has non-default trim values applied
    When a diagnostic bundle is generated
    Then the bundle SHALL include a trim values section listing all active trim offsets
