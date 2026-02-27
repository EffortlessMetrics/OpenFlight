@REQ-152 @product
Feature: Session management  @AC-152.1
  Scenario: Session created with unique ID on startup
    Given the flight service is starting up
    When a new session is initialised
    Then the session SHALL be assigned a unique identifier  @AC-152.2
  Scenario: Session saves profile context on aircraft switch
    Given an active session with a loaded profile
    When the active aircraft changes to a different type
    Then the session SHALL save the current profile context before switching  @AC-152.3
  Scenario: Session restores last profile on reconnect
    Given a session with a previously saved profile context
    When the service reconnects after a brief disconnection
    Then the session SHALL restore the last saved profile context  @AC-152.4
  Scenario: Session exports diagnostic bundle on request
    Given an active session with event history
    When the export diagnostic bundle command is issued
    Then the session SHALL produce a self-contained diagnostic archive  @AC-152.5
  Scenario: Multiple sessions do not conflict
    Given two concurrent sessions with different profile contexts
    When each session processes an event
    Then each session SHALL apply only its own profile context  @AC-152.6
  Scenario: Session log entries are timestamped
    Given an active session with logging enabled
    When a loggable event occurs
    Then the log entry SHALL include a UTC timestamp  @AC-152.7
  Scenario: Session terminated cleanly on service stop
    Given an active session with open resources
    When the service stop command is issued
    Then the session SHALL release all resources and flush logs before exit  @AC-152.8
  Scenario: Session UUID format is RFC 4122 compliant
    Given the flight service has created a new session
    When the session identifier is inspected
    Then the identifier SHALL conform to the RFC 4122 UUID format
