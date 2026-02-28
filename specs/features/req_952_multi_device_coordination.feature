Feature: Multi-Device Coordination
  As a flight simulation enthusiast
  I want multi-device coordination
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Synchronized output is delivered across multiple connected devices simultaneously
    Given the system is configured for multi-device coordination
    When the feature is exercised
    Then synchronized output is delivered across multiple connected devices simultaneously

  Scenario: Device coordination maintains sub-millisecond timing alignment
    Given the system is configured for multi-device coordination
    When the feature is exercised
    Then device coordination maintains sub-millisecond timing alignment

  Scenario: Failure of one device does not disrupt coordination of remaining devices
    Given the system is configured for multi-device coordination
    When the feature is exercised
    Then failure of one device does not disrupt coordination of remaining devices

  Scenario: Coordination groups are configurable per profile
    Given the system is configured for multi-device coordination
    When the feature is exercised
    Then coordination groups are configurable per profile