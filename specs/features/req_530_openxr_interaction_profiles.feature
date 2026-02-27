@REQ-530 @product
Feature: OpenXR Interaction Profile Support — Multi-Profile Action Binding  @AC-530.1
  Scenario: Service queries available interaction profiles on session start
    Given an OpenXR runtime is active with two supported interaction profiles
    When the OpenXR adapter initialises a session
    Then the adapter SHALL enumerate and log all available interaction profiles  @AC-530.2
  Scenario: Preferred interaction profile is configurable
    Given a profile config specifying preferred_openxr_profile = valve/index_controller
    When the OpenXR session starts
    Then the adapter SHALL select the valve/index_controller profile if supported  @AC-530.3
  Scenario: Action bindings are configured per interaction profile
    Given bindings defined for both oculus_touch and valve/index_controller profiles
    When the active OpenXR interaction profile is valve/index_controller
    Then only bindings for that profile SHALL be active  @AC-530.4
  Scenario: Profile selection is reported in OpenXR adapter diagnostics
    Given the OpenXR adapter is running
    When a diagnostic query is issued
    Then the response SHALL include the currently active interaction profile path
