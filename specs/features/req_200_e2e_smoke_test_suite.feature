@REQ-200 @infra
Feature: Full end-to-end smoke tests verify core OpenFlight workflows  @AC-200.1
  Scenario: Service start to axis event smoke test
    Given a fresh service instance is started
    When a fake HID device is connected
    Then axis events SHALL appear on the event bus confirming end-to-end data flow  @AC-200.2
  Scenario: Profile deadzone update smoke test
    Given a profile is loaded with an initial deadzone of 0.05
    When the deadzone is updated to 0.15 via the configuration API
    Then axis behavior SHALL reflect the new deadzone value within one configuration cycle  @AC-200.3
  Scenario: Fake sim telemetry bus snapshot smoke test
    Given a fake simulator adapter is connected
    When telemetry data is injected via the fake adapter
    Then a bus snapshot SHALL contain the injected telemetry values  @AC-200.4
  Scenario: Watchdog restart recovery smoke test
    Given the watchdog is monitoring the service
    When a simulated crash triggers a watchdog restart
    Then the service SHALL recover and resume normal operation  @AC-200.5
  Scenario: Smoke test suite completes within 60 seconds
    Given the full smoke test suite is executed on a standard CI runner
    When all smoke tests have run
    Then the total elapsed time SHALL be less than 60 seconds  @AC-200.6
  Scenario: Smoke test failure blocks PR merge
    Given a smoke test has failed in the CI pipeline
    When a pull request build is evaluated
    Then the PR merge SHALL be blocked until the failing smoke test is resolved
