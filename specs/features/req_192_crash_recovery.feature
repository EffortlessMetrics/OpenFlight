@REQ-192 @infra
Feature: Service recovers cleanly from unexpected crashes without data loss  @AC-192.1
  Scenario: Active profile persisted atomically every 30 seconds
    Given the OpenFlight service is running with an active profile
    When 30 seconds elapse
    Then the active profile SHALL have been written atomically to disk  @AC-192.2
  Scenario: Last known profile loaded on restart after crash
    Given the service has previously persisted a profile and then crashes unexpectedly
    When the service restarts
    Then the last persisted profile SHALL be loaded automatically without user intervention  @AC-192.3
  Scenario: Crash report generated with stack trace
    Given the service experiences an unexpected panic or fatal error
    When the crash occurs
    Then a crash report containing the stack trace SHALL be generated and stored locally  @AC-192.4
  Scenario: Watchdog restarts service within 5 seconds of unexpected exit
    Given the watchdog is monitoring the service process
    When the service exits unexpectedly
    Then the watchdog SHALL initiate a service restart within 5 seconds  @AC-192.5
  Scenario: Uncommitted profile edits discarded after crash
    Given a profile edit is in progress and has not been committed when the service crashes
    When the service restarts
    Then the in-progress edit SHALL be discarded and the last committed profile SHALL be active  @AC-192.6
  Scenario: Crash loop guard prevents repeated restarts
    Given the service has crashed 3 times within 60 seconds
    When the watchdog detects the third crash
    Then the watchdog SHALL stop attempting restarts to prevent a boot loop
