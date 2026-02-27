@REQ-281 @product
Feature: Linux systemd integration with user unit, notify readiness, udev rules, and journal logging  @AC-281.1
  Scenario: flightd ships with a systemd user unit file
    Given the OpenFlight Linux package is installed
    When the installed file tree is inspected
    Then a systemd user unit file SHALL exist at the expected unit directory path  @AC-281.2
  Scenario: Unit file specifies Type=notify for readiness signaling
    Given the installed systemd user unit file for flightd
    When the unit file contents are read
    Then the Type field SHALL be set to notify so systemd waits for the service ready signal  @AC-281.3
  Scenario: Service enables on user login via systemctl user enable
    Given flightd is installed with a valid systemd user unit file
    When the user runs systemctl --user enable flightd
    Then the service SHALL be enabled and will start automatically on subsequent user logins  @AC-281.4
  Scenario: udev rules are installed for device access
    Given the OpenFlight Linux package is installed
    When the udev rules directory is inspected
    Then udev rules for supported HID devices SHALL be present granting the flightd user read-write access  @AC-281.5
  Scenario: Group membership is configured in postinst
    Given the OpenFlight Debian package is being installed
    When the postinst script runs
    Then the installing user SHALL be added to the required hardware access group for HID device permissions  @AC-281.6
  Scenario: Journal logging works via tracing subscriber
    Given the flightd service is running under systemd
    When a structured log event is emitted by the tracing subscriber
    Then the log entry SHALL appear in the systemd journal and be retrievable via journalctl
