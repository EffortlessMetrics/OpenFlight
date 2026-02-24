@REQ-21
Feature: Telemetry bus snapshot and publisher

  @AC-21.1
  Scenario: Construct a valid BusSnapshot
    Given a BusSnapshot is constructed with valid flight data
    When the snapshot is validated
    Then validation SHALL succeed
    And core kinematics fields SHALL be accessible

  @AC-21.1
  Scenario: BusSnapshot validation rejects out-of-range values
    Given a BusSnapshot with an out-of-range field value
    When the snapshot is validated
    Then validation SHALL return an error indicating the invalid field

  @AC-21.2
  Scenario: Percentage type rejects out-of-range values
    Given a Percentage value of 1.5 (outside 0.0..1.0)
    When the Percentage is constructed
    Then construction SHALL fail with a range error

  @AC-21.2
  Scenario: GForce and Mach types enforce ranges
    Given G-force and Mach values at boundary conditions
    When each is constructed
    Then valid values SHALL succeed and invalid values SHALL fail

  @AC-21.3
  Scenario: Subscriber IDs are unique
    Given multiple subscribers registered with a BusPublisher
    When their IDs are compared
    Then each subscriber SHALL have a unique ID

  @AC-21.3
  Scenario: Publisher enforces rate limiting
    Given a rate limiter configured for 10 Hz
    When 100 updates are submitted in rapid succession
    Then only approximately 10 updates per second SHALL be forwarded

  @AC-21.4
  Scenario: MSFS cruise scenario produces expected telemetry
    Given an MSFS cruise flight scenario fixture
    When the end-to-end test runs
    Then telemetry SHALL include valid speed, altitude, and attitude data

  @AC-21.4
  Scenario: X-Plane cruise scenario produces expected telemetry
    Given an X-Plane cruise flight scenario fixture
    When the end-to-end test runs
    Then telemetry SHALL include valid speed, altitude, and attitude data

  @AC-21.4
  Scenario: DCS cruise scenario produces expected telemetry
    Given a DCS cruise flight scenario fixture
    When the end-to-end test runs
    Then telemetry SHALL include valid speed, altitude, and attitude data
