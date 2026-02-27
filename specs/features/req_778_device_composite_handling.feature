Feature: Device Composite Handling
  As a flight simulation enthusiast
  I want device composite handling
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Enumerate composite interfaces
    Given the system is configured for device composite handling
    When the feature is exercised
    Then service correctly enumerates interfaces of usb composite devices

  Scenario: Separate logical devices per interface
    Given the system is configured for device composite handling
    When the feature is exercised
    Then each interface is treated as a separate logical device

  Scenario: Track composite relationships
    Given the system is configured for device composite handling
    When the feature is exercised
    Then composite device relationship is tracked for status display

  Scenario: Remove all interfaces on disconnect
    Given the system is configured for device composite handling
    When the feature is exercised
    Then disconnecting a composite device removes all its interfaces
