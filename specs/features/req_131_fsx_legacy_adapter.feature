@REQ-131 @product
Feature: FSX legacy adapter  @AC-131.1
  Scenario: SimConnect connects to FSX with legacy SDK
    Given Microsoft Flight Simulator X is running with the legacy SimConnect server
    When the FSX adapter opens a SimConnect session using the FSX SDK
    Then the connection SHALL succeed and the adapter SHALL be in the connected state  @AC-131.2
  Scenario: Variable names match FSX-era dataref naming
    Given an active FSX SimConnect session
    When the adapter enumerates its supported simulation variables
    Then each variable name SHALL conform to the FSX-era naming convention
    And no variable name SHALL use an MSFS 2020 or later naming extension  @AC-131.3
  Scenario: FSUIPC offset mode available as fallback
    Given the FSX adapter is initialised without a SimConnect server
    When FSUIPC is installed and accessible
    Then the adapter SHALL activate FSUIPC offset mode as a fallback transport  @AC-131.4
  Scenario: Profile loads on aircraft change
    Given the FSX adapter is in the active state
    When the simulated aircraft changes to a different model
    Then the profile matching the new aircraft SHALL be loaded within 500 ms  @AC-131.5
  Scenario: Adapter marks itself as tier 3 in health report
    Given the FSX adapter is connected and active
    When a system health report is requested
    Then the FSX adapter entry SHALL have tier set to 3  @AC-131.6
  Scenario: Graceful degradation when FSUIPC not installed
    Given the FSX adapter starts without SimConnect and without FSUIPC
    When the adapter attempts to initialise
    Then the adapter SHALL log a warning and enter a limited-functionality mode
    And no panic or unhandled error SHALL occur
