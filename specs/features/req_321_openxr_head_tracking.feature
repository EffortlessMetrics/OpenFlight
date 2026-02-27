@REQ-321 @product
Feature: OpenXR Head Tracking  @AC-321.1
  Scenario: Service polls OpenXR runtime for head pose at 90Hz
    Given the OpenXR head tracking feature is enabled
    When the service is running
    Then the service SHALL poll the OpenXR runtime for head pose at a rate of 90Hz  @AC-321.2
  Scenario: Head pose yaw and pitch are exposed as a virtual axis pair
    Given OpenXR head tracking is active
    When the user moves their head
    Then yaw and pitch SHALL each be exposed as a separate virtual axis accessible to the profile pipeline  @AC-321.3
  Scenario: OpenXR session lifecycle is managed on demand
    Given the OpenXR feature is enabled
    When head tracking is started and later stopped
    Then the service SHALL create the OpenXR session on start and destroy it on stop without leaking resources  @AC-321.4
  Scenario: Head tracking works in OpenXR simulator mode without a physical headset
    Given no VR headset is connected but the OpenXR simulator runtime is available
    When the service attempts to start head tracking
    Then the service SHALL successfully create an OpenXR session using the simulator runtime  @AC-321.5
  Scenario: Pose data is filtered with EMA to reduce jitter
    Given raw head pose data contains high-frequency noise
    When the pose samples are processed
    Then the output virtual axis values SHALL be smoothed using an exponential moving average filter  @AC-321.6
  Scenario: OpenXR support requires the vr feature flag at compile time
    Given the service binary is compiled without the vr feature flag
    When the user attempts to enable OpenXR head tracking
    Then the service SHALL report that OpenXR support is not available in this build
