@REQ-448 @product
Feature: Linux systemd User Service — Run as a systemd User Service on Linux

  @AC-448.1
  Scenario: Package installs a systemd user unit file
    Given the OpenFlight package is installed on a Linux system
    When the installation completes
    Then a systemd user unit file for openflight SHALL be present in the appropriate unit directory

  @AC-448.2
  Scenario: Service can be enabled with systemctl --user enable openflight
    Given the unit file is installed
    When the user runs systemctl --user enable openflight
    Then the service SHALL be enabled and set to start on subsequent graphical logins

  @AC-448.3
  Scenario: Service starts after graphical login and stops on logout
    Given the openflight user service is enabled
    When the user completes a graphical login
    Then flightd SHALL start automatically, and it SHALL stop when the user logs out

  @AC-448.4
  Scenario: udev rules are installed to grant device access without root
    Given the OpenFlight package is installed
    When a supported HID device is connected
    Then the udev rules SHALL grant the active user read/write access without requiring root privileges
