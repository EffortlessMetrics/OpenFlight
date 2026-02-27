@REQ-460 @product
Feature: Input Macro Recording — CLI Record and Playback of Input Macros

  @AC-460.1
  Scenario: flightctl macro record starts capturing button and axis events
    Given a running service with a connected device
    When the command "flightctl macro record --name startup_sequence" is executed
    Then the service SHALL begin capturing all button and axis events to a recording buffer

  @AC-460.2
  Scenario: flightctl macro stop saves the recording to a named macro file
    Given an active macro recording named "startup_sequence"
    When the command "flightctl macro stop" is executed
    Then recording SHALL cease and events SHALL be persisted to the macro file "startup_sequence"

  @AC-460.3
  Scenario: flightctl macro play replays events in configured loop mode
    Given a saved macro file "startup_sequence" with loop mode set to once
    When the command "flightctl macro play --name startup_sequence" is executed
    Then the recorded events SHALL be replayed once in their original timing and order

  @AC-460.4
  Scenario: Macros can be assigned to hardware buttons in profile
    Given a profile with a button binding that references macro "startup_sequence"
    When the bound hardware button is pressed
    Then the macro SHALL be triggered and begin playback as configured
