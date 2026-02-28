Feature: Thread Deadlock Detection
  As a flight simulation enthusiast
  I want thread deadlock detection
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Thread progress is monitored to detect potential deadlock conditions
    Given the system is configured for thread deadlock detection
    When the feature is exercised
    Then thread progress is monitored to detect potential deadlock conditions

  Scenario: Deadlock detection reports involved threads and held resources
    Given the system is configured for thread deadlock detection
    When the feature is exercised
    Then deadlock detection reports involved threads and held resources

  Scenario: Watchdog timer triggers recovery action when thread becomes unresponsive
    Given the system is configured for thread deadlock detection
    When the feature is exercised
    Then watchdog timer triggers recovery action when thread becomes unresponsive

  Scenario: Deadlock events are logged with full stack trace for post-mortem analysis
    Given the system is configured for thread deadlock detection
    When the feature is exercised
    Then deadlock events are logged with full stack trace for post-mortem analysis