Feature: Update Notification
  As a flight simulation enthusiast
  I want update notification
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: User is notified when a new update is available on their channel
    Given the system is configured for update notification
    When the feature is exercised
    Then user is notified when a new update is available on their channel

  Scenario: Notification includes version number and brief summary of changes
    Given the system is configured for update notification
    When the feature is exercised
    Then notification includes version number and brief summary of changes

  Scenario: Notification is delivered via system tray and IPC event
    Given the system is configured for update notification
    When the feature is exercised
    Then notification is delivered via system tray and IPC event

  Scenario: Notification frequency is rate-limited to prevent spam on rapid releases
    Given the system is configured for update notification
    When the feature is exercised
    Then notification frequency is rate-limited to prevent spam on rapid releases
