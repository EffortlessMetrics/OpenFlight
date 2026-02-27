@REQ-466 @product
Feature: Auto-Switch Confidence Threshold — Configurable Detection Confidence  @AC-466.1
  Scenario: Profile is not applied when detection confidence is below threshold
    Given an auto-switch confidence threshold of 0.8
    When aircraft detection produces a confidence score of 0.6
    Then the profile SHALL NOT be switched automatically  @AC-466.2
  Scenario: Confidence threshold is configurable per simulator in profile
    Given a profile with MSFS confidence threshold 0.9 and DCS confidence threshold 0.7
    When auto-detection runs for each simulator
    Then each simulator SHALL use its own configured threshold for the switch decision  @AC-466.3
  Scenario: Low-confidence detections are logged as warnings
    Given an auto-switch confidence threshold of 0.8
    When aircraft detection produces a confidence score below the threshold
    Then a warning SHALL be logged with the detection result and confidence score  @AC-466.4
  Scenario: Manual override bypasses confidence check
    Given an auto-switch confidence threshold of 0.9
    When a manual profile switch command is issued via flightctl
    Then the profile SHALL be applied immediately regardless of detection confidence
