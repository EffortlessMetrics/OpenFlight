@REQ-308 @product
Feature: AeroFly FS Integration

  @AC-308.1
  Scenario: Service connects to AeroFly FS via UDP telemetry plugin
    Given AeroFly FS is running with the telemetry plugin enabled
    When the service starts with an AeroFly FS profile
    Then the service SHALL connect to the AeroFly FS UDP telemetry interface to receive aircraft state data

  @AC-308.2
  Scenario: AeroFly sends aircraft state on port 34401
    Given the AeroFly FS telemetry plugin is active
    When AeroFly FS transmits aircraft state data
    Then the service SHALL listen on UDP port 34401 for incoming AeroFly aircraft state packets

  @AC-308.3
  Scenario: Packet format includes magic bytes for validation
    Given the service is listening for AeroFly FS UDP packets
    When a UDP packet arrives on port 34401
    Then the service SHALL validate the expected magic bytes in the packet header before processing the payload

  @AC-308.4
  Scenario: Connection state is tracked as connected or disconnected
    Given the service is monitoring the AeroFly FS UDP stream
    When packets are received or stop arriving
    Then the service SHALL track and expose the connection state as either connected or disconnected

  @AC-308.5
  Scenario: Profile switch is triggered by aircraft type detection
    Given the service is receiving AeroFly FS telemetry
    When the aircraft type in the received data changes
    Then the service SHALL trigger a profile switch to match the newly detected aircraft type

  @AC-308.6
  Scenario: Integration replay test validates packet parsing
    Given a set of captured AeroFly FS UDP packet recordings
    When the integration replay test plays those packets back to the service listener
    Then the service SHALL parse each packet correctly and produce the expected aircraft state field values
