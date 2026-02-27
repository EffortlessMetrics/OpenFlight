@REQ-447 @product
Feature: Windows Service Integration — Register and Run as a Windows Service

  @AC-447.1
  Scenario: Service can be installed as a Windows service via flightctl service install
    Given flightctl is installed on a Windows machine
    When the user runs flightctl service install
    Then flightd SHALL be registered in the Windows Service Control Manager

  @AC-447.2
  Scenario: Windows service starts automatically on user login
    Given flightd is registered as a Windows service with auto-start
    When the user logs in to Windows
    Then the service SHALL start automatically without manual intervention

  @AC-447.3
  Scenario: Service gracefully handles Windows service stop events
    Given flightd is running as a Windows service
    When the Service Control Manager sends a stop control
    Then the service SHALL complete in-progress work, release all resources, and exit cleanly

  @AC-447.4
  Scenario: Service event log entries are written for start, stop, and errors
    Given flightd is running as a Windows service
    When the service starts, stops, or encounters an error
    Then a corresponding entry SHALL be written to the Windows Application event log
