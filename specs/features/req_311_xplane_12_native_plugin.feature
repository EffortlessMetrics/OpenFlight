@REQ-311 @product
Feature: X-Plane 12 Native Plugin

  @AC-311.1
  Scenario: Service ships an X-Plane 12 plugin xpl file for native integration
    Given a release build of the service is produced
    When the distribution package is assembled
    Then the package SHALL include an X-Plane 12 native plugin xpl file for installation in the X-Plane plugins directory

  @AC-311.2
  Scenario: Plugin reads aircraft DataRefs and sends to service via localhost UDP
    Given the X-Plane 12 plugin is loaded in X-Plane
    When X-Plane is running a flight
    Then the plugin SHALL read relevant aircraft DataRefs and transmit their values to the service via localhost UDP

  @AC-311.3
  Scenario: Plugin auto-enables on X-Plane launch via plugin discovery
    Given the plugin xpl file is installed in the X-Plane plugins directory
    When X-Plane 12 launches
    Then X-Plane SHALL automatically discover and enable the plugin through its standard plugin discovery mechanism

  @AC-311.4
  Scenario: Plugin version is checked against service version on connect
    Given the X-Plane plugin has started and is transmitting data
    When the plugin establishes contact with the service
    Then the plugin SHALL exchange version information with the service and report a mismatch if versions are incompatible

  @AC-311.5
  Scenario: Plugin crash does not crash X-Plane via XPLM error boundary
    Given the X-Plane 12 plugin is loaded
    When an unhandled error occurs inside the plugin
    Then the XPLM error boundary SHALL prevent the plugin error from propagating to X-Plane and crashing the simulator

  @AC-311.6
  Scenario: Plugin source compiles with X-Plane SDK 4.0 and later
    Given the plugin source code and the X-Plane SDK 4.0 headers
    When the plugin is compiled against the SDK
    Then the build SHALL succeed without errors against X-Plane SDK version 4.0 or any later compatible version
