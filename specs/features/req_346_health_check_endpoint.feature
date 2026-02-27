@REQ-346 @product
Feature: Health Check Endpoint  @AC-346.1
  Scenario: Service exposes HTTP /health endpoint on default port
    Given the service is running with no explicit health port configured
    When an HTTP GET request is sent to http://localhost:8080/health
    Then the service SHALL respond with an HTTP 200 status  @AC-346.2
  Scenario: /health returns JSON with status, uptime, device count, and sim name
    Given the service is running and connected to a simulator
    When an HTTP GET request is sent to /health
    Then the response body SHALL be valid JSON containing status, uptime, device_count, and sim_name fields  @AC-346.3
  Scenario: /health/ready returns 200 only when a device is connected
    Given no HID devices are connected to the service
    When an HTTP GET request is sent to /health/ready
    Then the response SHALL return HTTP 503
    And when at least one device is connected the response SHALL return HTTP 200  @AC-346.4
  Scenario: /health/live returns 200 if service is not in panic state
    Given the service is running normally
    When an HTTP GET request is sent to /health/live
    Then the response SHALL return HTTP 200  @AC-346.5
  Scenario: Health endpoint respects --health-port flag
    Given the service is started with --health-port 9090
    When an HTTP GET request is sent to http://localhost:9090/health
    Then the service SHALL respond on that port instead of the default 8080  @AC-346.6
  Scenario: Health check is included in systemd watchdog
    Given the service is running under systemd with WatchdogSec configured
    When the systemd watchdog interval elapses
    Then the service SHALL notify systemd via the watchdog mechanism based on /health/live status
