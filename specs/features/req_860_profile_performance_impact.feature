Feature: Profile Performance Impact
  As a flight simulation enthusiast
  I want profile performance impact
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Profile complexity cost is measured and reported as a score
    Given the system is configured for profile performance impact
    When the feature is exercised
    Then profile complexity cost is measured and reported as a score

  Scenario: Performance impact analysis runs without affecting RT spine latency
    Given the system is configured for profile performance impact
    When the feature is exercised
    Then performance impact analysis runs without affecting RT spine latency

  Scenario: Profiles exceeding a complexity threshold trigger a warning
    Given the system is configured for profile performance impact
    When the feature is exercised
    Then profiles exceeding a complexity threshold trigger a warning

  Scenario: Impact report breaks down cost by axis, curve, and mixer components
    Given the system is configured for profile performance impact
    When the feature is exercised
    Then impact report breaks down cost by axis, curve, and mixer components
