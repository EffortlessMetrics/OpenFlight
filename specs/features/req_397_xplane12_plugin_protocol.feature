@REQ-397 @product
Feature: X-Plane 12 Plugin Protocol — Support XP12 Named-Pipe IPC Mechanism

  @AC-397.1
  Scenario: XP12 plugin uses XPLM3 SDK APIs
    Given the X-Plane 12 plugin build
    When the plugin API version is inspected
    Then it SHALL use XPLM3 SDK APIs rather than XPLM2

  @AC-397.2
  Scenario: Plugin communicates via named pipe or shared memory instead of UDP
    Given an active X-Plane 12 session with the OpenFlight plugin loaded
    When the IPC transport is inspected
    Then communication SHALL use a named pipe or shared memory, not UDP

  @AC-397.3
  Scenario: Plugin carries the OpenFlight plugin API version
    Given the XP12 plugin binary
    When the plugin API version metadata is read
    Then it SHALL match the current OpenFlight plugin API version

  @AC-397.4
  Scenario: Fallback to UDP mode is automatic when named pipe is unavailable
    Given an XP12 session where the named pipe cannot be opened
    When the plugin initialises
    Then it SHALL automatically fall back to UDP communication mode

  @AC-397.5
  Scenario: XP12 game manifest notes the plugin requirement and API version
    Given the XP12 game manifest file
    When the plugin requirement section is inspected
    Then it SHALL declare the required plugin name and OpenFlight API version

  @AC-397.6
  Scenario: Plugin upgrade procedure is documented in docs/how-to/
    Given the repository documentation
    When the how-to directory is searched for XP12 plugin upgrade guidance
    Then a document describing the upgrade procedure SHALL be present
