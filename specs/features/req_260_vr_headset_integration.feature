@REQ-260 @product
Feature: VR headset provides head tracking without external tracker hardware  @AC-260.1
  Scenario: VR headset orientation exposed as virtual axes
    Given a VR headset connected via OpenVR/SteamVR and the VR adapter enabled
    When head pose data is sampled
    Then the adapter SHALL publish yaw, pitch, and roll as virtual axes on the bus  @AC-260.2
  Scenario: OpenVR SteamVR runtime queried for head pose
    Given the SteamVR runtime is running with a connected headset
    When the adapter queries the head pose
    Then the adapter SHALL use the OpenVR API to retrieve the HMD pose matrix at the current game frame rate  @AC-260.3
  Scenario: VR tracking loss detected and output frozen at last valid pose
    Given the VR adapter is streaming head pose data
    When the OpenVR runtime reports a tracking-lost condition
    Then the adapter SHALL freeze all VR axis outputs at the last valid pose and set a tracking-lost status  @AC-260.4
  Scenario: VR axes available independently of flight controller axes
    Given a VR headset and a flight controller both connected
    When both devices are active simultaneously
    Then the VR head-tracking axes SHALL be available on the bus independently without displacing any flight controller axes  @AC-260.5
  Scenario: VR adapter disabled automatically when OpenTrack is active
    Given both the VR adapter and the OpenTrack adapter are configured
    When the OpenTrack adapter receives its first valid packet
    Then the VR adapter SHALL be automatically disabled and a log message SHALL indicate the reason  @AC-260.6
  Scenario: VR headset integration documented with setup guide
    Given the docs/how-to directory in the repository
    When the directory is inspected for VR documentation
    Then a setup guide SHALL exist explaining how to configure OpenVR/SteamVR integration with OpenFlight
