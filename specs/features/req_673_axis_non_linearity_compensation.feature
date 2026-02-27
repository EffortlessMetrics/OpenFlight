@REQ-673
Feature: Axis Non-Linearity Compensation
  @AC-673.1
  Scenario: Hardware non-linearity is measured during calibration
    Given the system is configured for REQ-673
    When the feature condition is met
    Then hardware non-linearity is measured during calibration

  @AC-673.2
  Scenario: Compensation lookup table is generated from calibration data
    Given the system is configured for REQ-673
    When the feature condition is met
    Then compensation lookup table is generated from calibration data

  @AC-673.3
  Scenario: Compensated output is linear within 1% of ideal
    Given the system is configured for REQ-673
    When the feature condition is met
    Then compensated output is linear within 1% of ideal

  @AC-673.4
  Scenario: Compensation is bypassed when custom response curves are active
    Given the system is configured for REQ-673
    When the feature condition is met
    Then compensation is bypassed when custom response curves are active
