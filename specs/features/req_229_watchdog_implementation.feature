@REQ-229 @infra
Feature: Service watchdog restarts flightd on unexpected failure within 5 seconds  @AC-229.1
  Scenario: Watchdog monitors flightd via IPC health ping every 5 seconds
    Given the watchdog process and flightd are both running
    When 5 seconds elapse
    Then the watchdog SHALL have sent an IPC health ping and received a response from flightd  @AC-229.2
  Scenario: Three consecutive failed health pings trigger restart
    Given the watchdog is monitoring flightd
    When 3 consecutive health pings receive no response
    Then the watchdog SHALL restart the flightd process  @AC-229.3
  Scenario: Watchdog restart logged with timestamp and exit code
    Given flightd has been restarted by the watchdog
    When the restart event occurs
    Then a log entry SHALL be written containing the UTC timestamp and the exit code of the failed process  @AC-229.4
  Scenario: Boot loop guard stops restarts after threshold
    Given flightd has been restarted 3 times within a 60-second window
    When a fourth failure occurs within the same window
    Then the watchdog SHALL stop attempting further restarts and log a boot-loop-guard event  @AC-229.5
  Scenario: Watchdog itself monitored by OS service manager
    Given OpenFlight is installed as a system service
    When the watchdog process exits unexpectedly
    Then the OS service manager (systemd or Windows SCM) SHALL restart the watchdog  @AC-229.6
  Scenario: Watchdog can be disabled via environment variable for development
    Given the OPENFLIGHT_DISABLE_WATCHDOG environment variable is set to 1
    When the service starts
    Then the watchdog SHALL not be activated and flightd SHALL run without watchdog supervision
