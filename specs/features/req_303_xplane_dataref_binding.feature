@REQ-303 @product
Feature: X-Plane DataRef Binding

  @AC-303.1
  Scenario: Service reads X-Plane DataRefs specified in profile
    Given a profile that specifies one or more X-Plane DataRef paths
    When the service connects to X-Plane
    Then the service SHALL read the value of each specified DataRef

  @AC-303.2
  Scenario: DataRef binding uses UDP extplane or native X-Plane UDP
    Given the service is configured to connect to X-Plane
    When the DataRef binding is established
    Then the service SHALL communicate via the UDP extplane protocol or native X-Plane UDP data output

  @AC-303.3
  Scenario: DataRefs are polled at configurable rate between 1 and 50Hz
    Given a profile that specifies a DataRef poll rate of 10Hz
    When the service is running and connected to X-Plane
    Then the DataRef SHALL be read at approximately 10Hz within the allowed 1-50Hz range

  @AC-303.4
  Scenario: Invalid DataRef name produces a warning in logs
    Given a profile that references a DataRef path that does not exist in X-Plane
    When the service attempts to bind that DataRef
    Then the service SHALL log a warning and continue rather than producing a fatal error

  @AC-303.5
  Scenario: DataRef values are normalized to the range minus one to one for axis use
    Given a DataRef that returns raw values in a simulator-specific range
    When the service maps the DataRef value to an axis
    Then the output value SHALL be normalized to the range [-1.0, 1.0]

  @AC-303.6
  Scenario: Multiple DataRefs can be combined into a virtual axis
    Given a profile that defines a virtual axis composed of two DataRefs
    When both DataRefs return values
    Then the service SHALL combine the DataRef values according to the profile mixing rule to produce a single virtual axis value
