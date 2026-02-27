@REQ-140 @product
Feature: Installer and update pipeline  @AC-140.1
  Scenario: Windows MSI installs binaries to expected paths
    Given a Windows MSI installer package for the current release
    When the installer is executed with default options
    Then flightd.exe and flightctl.exe SHALL be present under the installation directory  @AC-140.2
  Scenario: Linux deb installs to /usr/local/bin
    Given a Debian package for the current release
    When the package is installed with dpkg
    Then flightd and flightctl SHALL be present at /usr/local/bin  @AC-140.3
  Scenario: udev rules installed on Linux
    Given a Debian package for the current release
    When the package is installed with dpkg
    Then the OpenFlight udev rules file SHALL be present in /etc/udev/rules.d/  @AC-140.4
  Scenario: systemd user unit installed on Linux
    Given a Debian package for the current release
    When the package is installed with dpkg
    Then the flightd systemd user unit file SHALL be present under /usr/lib/systemd/user/  @AC-140.5
  Scenario: Uninstall reverses all file changes
    Given OpenFlight installed via the package manager
    When the package is uninstalled
    Then all files installed by the package SHALL be removed from the system  @AC-140.6
  Scenario: Update applies delta patch correctly
    Given version 1.0.0 installed and a delta patch targeting 1.1.0
    When the update pipeline applies the patch
    Then the installed binary version SHALL report 1.1.0  @AC-140.7
  Scenario: Rollback restores previous version
    Given version 1.1.0 installed with a stored rollback snapshot for 1.0.0
    When rollback is invoked
    Then the installed binary version SHALL report 1.0.0 and the system SHALL be in a consistent state  @AC-140.8
  Scenario: Signature verification passes for valid binary
    Given a release binary accompanied by its detached Ed25519 signature
    When the signature is verified against the binary and the release public key
    Then verification SHALL succeed and the binary SHALL be considered authentic
