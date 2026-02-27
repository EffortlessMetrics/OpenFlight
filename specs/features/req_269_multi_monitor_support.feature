@REQ-269 @product
Feature: UI supports secondary monitors with per-profile placement and graceful disconnect handling  @AC-269.1
  Scenario: UI displayed on secondary monitor
    Given a system with two or more monitors and a profile specifying the secondary display
    When the UI window is opened
    Then the window SHALL appear on the monitor identified in the profile  @AC-269.2
  Scenario: Monitor preference persisted per profile
    Given the user moves the UI window to a secondary monitor while a profile is active
    When the profile is saved and reloaded
    Then the UI window SHALL open on the same secondary monitor as before  @AC-269.3
  Scenario: Service survives secondary monitor disconnect
    Given the UI is displayed on a secondary monitor
    When that monitor is physically disconnected
    Then the service SHALL remain running and no crash SHALL occur  @AC-269.4
  Scenario: UI falls back to primary monitor on secondary disconnect
    Given the preferred secondary monitor has been disconnected
    When the UI window is shown
    Then the window SHALL appear on the primary monitor and a log message SHALL record the fallback  @AC-269.5
  Scenario: DPI scaling respected on each monitor
    Given monitors with different DPI scaling factors
    When the UI window is moved between monitors
    Then all UI elements SHALL render at the correct physical size on each monitor without manual rescaling  @AC-269.6
  Scenario: StreamDeck layout scales to monitor DPI
    Given a StreamDeck panel layout rendered on a high-DPI monitor
    When the layout is displayed
    Then button icons and labels SHALL scale proportionally to the monitor DPI and remain legible
