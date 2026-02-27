@REQ-218 @infra
Feature: Portable ZIP distribution runs without installation on Windows/Linux  @AC-218.1
  Scenario: ZIP contains flightd flightctl default config and README
    Given the portable ZIP artifact produced by CI
    When the ZIP contents are listed
    Then flightd, flightctl, a default config file, and a README SHALL all be present  @AC-218.2
  Scenario: Service starts from extracted directory without PATH modification
    Given the ZIP extracted to an arbitrary directory with no PATH changes
    When flightd is launched from that directory
    Then the service SHALL start successfully and reach the running state  @AC-218.3
  Scenario: Config directory defaults to user home on first run if not found
    Given the portable build is run for the first time with no existing config directory
    When the service initialises
    Then the config directory SHALL default to a subdirectory within the user home directory  @AC-218.4
  Scenario: Portable mode does not write to system paths
    Given the portable service is running
    When all file writes during a normal session are monitored
    Then no files SHALL be written outside the extraction directory and the user home config directory  @AC-218.5
  Scenario: Checksum file included for artifact integrity verification
    Given the portable ZIP artifact
    When the artifact directory is inspected
    Then a checksum file SHALL be present containing cryptographic hashes for the ZIP and its contents  @AC-218.6
  Scenario: Portable build produced as CI artifact on every release tag
    Given a release tag is pushed to the repository
    When the CI pipeline completes
    Then the portable ZIP SHALL be published as a downloadable CI artifact for that release tag
