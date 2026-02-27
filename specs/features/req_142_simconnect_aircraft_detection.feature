@REQ-142 @product
Feature: SimConnect aircraft detection  @AC-142.1
  Scenario: ATC model used as primary aircraft identifier
    Given a SimConnect session is open
    When the ATC_MODEL simulation variable is available
    Then the adapter SHALL use the ATC model string as the primary aircraft identifier  @AC-142.2
  Scenario: Cessna 172 title maps to C172 type
    Given a SimConnect aircraft change event with title containing "Cessna 172"
    When aircraft detection processes the title
    Then the detected aircraft type SHALL be C172  @AC-142.3
  Scenario: Boeing 737 title maps to B738 type
    Given a SimConnect aircraft change event with title containing "Boeing 737"
    When aircraft detection processes the title
    Then the detected aircraft type SHALL be B738  @AC-142.4
  Scenario: Unknown aircraft returns None without crash
    Given a SimConnect aircraft change event with an unrecognised title
    When aircraft detection processes the title
    Then the result SHALL be None and no panic SHALL occur  @AC-142.5
  Scenario: Callback fires on first aircraft detection
    Given a SimConnect session with no prior aircraft detected
    When an aircraft change event is received for the first time
    Then the registered detection callback SHALL be invoked once  @AC-142.6
  Scenario: Callback not re-fired for same aircraft
    Given a SimConnect session where a Cessna 172 was already detected
    When a duplicate aircraft change event arrives for the same aircraft
    Then the registered detection callback SHALL NOT be invoked again  @AC-142.7
  Scenario: Short buffer returns InvalidFormat error
    Given a SimConnect aircraft title buffer that is shorter than the minimum expected length
    When the adapter attempts to parse the aircraft title
    Then the adapter SHALL return an InvalidFormat error
