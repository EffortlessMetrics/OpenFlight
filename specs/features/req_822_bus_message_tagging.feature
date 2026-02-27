Feature: Bus Message Tagging
  As a flight simulation enthusiast
  I want bus message tagging
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Attach user-defined tags to messages
    Given the system is configured for bus message tagging
    When the feature is exercised
    Then bus messages support attaching user-defined string tags

  Scenario: Filter messages by tag patterns
    Given the system is configured for bus message tagging
    When the feature is exercised
    Then subscribers can filter incoming messages by tag patterns

  Scenario: Index tags for efficient filtering
    Given the system is configured for bus message tagging
    When the feature is exercised
    Then tags are indexed for efficient filtering without linear scan

  Scenario: Enforce maximum tag count per message
    Given the system is configured for bus message tagging
    When the feature is exercised
    Then maximum tag count per message is enforced to prevent abuse
