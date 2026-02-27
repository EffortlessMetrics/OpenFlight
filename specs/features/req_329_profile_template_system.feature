@REQ-329 @product
Feature: Profile Template System  @AC-329.1
  Scenario: Users can create profile templates for common aircraft types
    Given a user who has configured a profile for a specific aircraft type
    When the user runs flightctl template save --name "GA Single Engine"
    Then the service SHALL persist the current profile as a named template  @AC-329.2
  Scenario: Templates are applied as starting points for new profiles
    Given a saved template named "Airliner"
    When the user creates a new profile with flightctl profile new --template "Airliner"
    Then the new profile SHALL be pre-populated with the template's settings  @AC-329.3
  Scenario: Templates can include suggested axis curves and deadzones
    Given a template that contains axis curve and deadzone definitions
    When the template is applied to a new profile
    Then the new profile SHALL inherit the template's axis curves and deadzone settings  @AC-329.4
  Scenario: OpenFlight ships with default templates for common sim types
    Given a fresh installation of OpenFlight
    When the user runs flightctl template list
    Then the output SHALL include built-in templates covering at minimum MSFS, X-Plane, and DCS  @AC-329.5
  Scenario: Templates are listed via flightctl template list
    Given one or more templates exist (built-in and custom)
    When the user runs flightctl template list
    Then the CLI SHALL display all available templates with name and source (built-in vs custom)  @AC-329.6
  Scenario: Custom templates are stored in user config directory
    Given a user-created template
    When the template is saved
    Then the service SHALL write the template file to the user config directory (default: ~/.config/openflight/templates/)
