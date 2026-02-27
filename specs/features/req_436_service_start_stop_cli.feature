@REQ-436 @product
Feature: Service Start Stop CLI Commands — Start, Stop, Status, and Restart via flightctl

  @AC-436.1
  Scenario: flightctl start launches service daemon and waits for ready signal
    Given the service is not running
    When `flightctl start` is executed
    Then it SHALL launch the service daemon and block until a ready signal is received

  @AC-436.2
  Scenario: flightctl stop sends graceful shutdown and waits for exit
    Given the service is running
    When `flightctl stop` is executed
    Then it SHALL send a graceful shutdown request and wait for the process to exit

  @AC-436.3
  Scenario: flightctl status returns service health, uptime, and version
    Given the service is running
    When `flightctl status` is executed
    Then the output SHALL include service health state, uptime duration, and version string

  @AC-436.4
  Scenario: flightctl restart performs stop then start with config reload
    Given the service is running
    When `flightctl restart` is executed
    Then it SHALL stop the running instance, reload config from disk, and start a new instance
