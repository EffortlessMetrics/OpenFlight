@REQ-216 @infra
Feature: Windows MSI installer passes end-to-end installation verification  @AC-216.1
  Scenario: MSI installs flightd service and flightctl CLI to correct paths
    Given a clean Windows system with no prior OpenFlight installation
    When the MSI installer is run to completion
    Then flightd.exe and flightctl.exe SHALL be present in the configured installation directory  @AC-216.2
  Scenario: Installer creates Windows service with DEMAND_START
    Given the MSI installer has completed successfully
    When the Windows service control manager is queried
    Then the flightd service SHALL exist with START_TYPE set to DEMAND_START  @AC-216.3
  Scenario: SimConnect integration optionally installed via checkbox
    Given the MSI installer is presented to the user
    When the user selects or deselects the SimConnect optional component checkbox
    Then SimConnect integration files SHALL be installed or omitted accordingly  @AC-216.4
  Scenario: Uninstall removes installed files but preserves user config
    Given OpenFlight is installed and user configuration files are present
    When the MSI uninstaller is run
    Then all installed binaries SHALL be removed and user configuration files SHALL be preserved  @AC-216.5
  Scenario: Installer is signed with code signing certificate
    Given the MSI installer file
    When the Authenticode signature is verified
    Then the signature SHALL be valid and issued by the project code signing certificate  @AC-216.6
  Scenario: Installation log captures all installed files for audit
    Given the MSI installer is run with logging enabled
    When installation completes
    Then the installation log SHALL contain an entry for every file installed
