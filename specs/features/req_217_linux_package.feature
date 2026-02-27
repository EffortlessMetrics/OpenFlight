@REQ-217 @infra
Feature: Linux deb/rpm package installs and configures service correctly  @AC-217.1
  Scenario: Deb package installs binaries and udev rules to correct paths
    Given a clean Debian-based system with no prior OpenFlight installation
    When the deb package is installed via dpkg
    Then binaries SHALL be present in /usr/bin and udev rules SHALL be present in /etc/udev/rules.d  @AC-217.2
  Scenario: Postinst adds user to input group and reloads udev rules
    Given the deb package postinst script is executed
    When installation completes
    Then the current user SHALL be added to the input group and udev rules SHALL be reloaded  @AC-217.3
  Scenario: Systemd user unit installed and user can enable without root
    Given the package has been installed
    When a non-root user runs systemctl enable for the OpenFlight user unit
    Then the service SHALL be enabled without requiring root privileges  @AC-217.4
  Scenario: Prerm script stops service before uninstall
    Given the OpenFlight service is running
    When the package is removed and the prerm script executes
    Then the service SHALL be stopped cleanly before any files are removed  @AC-217.5
  Scenario: Postrm cleans up udev rules and systemd unit on purge
    Given the package has been removed
    When the package is purged via dpkg purge
    Then udev rules and the systemd unit file SHALL be removed from the system  @AC-217.6
  Scenario: Package metadata includes correct SPDX license identifier
    Given the deb or rpm package
    When the package metadata is inspected
    Then the license field SHALL contain the correct SPDX license identifier for the project
