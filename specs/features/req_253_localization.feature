@REQ-253 @product
Feature: OpenFlight CLI and logs support localized output  @AC-253.1
  Scenario: CLI output respects LANG and LC_ALL environment variable
    Given the environment variable LANG is set to de_DE.UTF-8
    When a flightctl command is executed that produces user-facing output
    Then the output strings SHALL be drawn from the German message catalog  @AC-253.2
  Scenario: English German and Japanese message catalogs provided
    Given the flightctl binary is built
    When the list of bundled message catalogs is inspected
    Then catalogs for English, German, and Japanese SHALL be present  @AC-253.3
  Scenario: Log messages always in English regardless of locale
    Given LANG is set to ja_JP.UTF-8
    When the flightd service emits a structured log message
    Then the log message text SHALL be in English  @AC-253.4
  Scenario: Date and time output follows locale conventions
    Given LANG is set to de_DE.UTF-8
    When a flightctl command displays a timestamp to the user
    Then the date and time format SHALL follow German locale conventions  @AC-253.5
  Scenario: Device vendor names preserved as-is without translation
    Given a device with the vendor name Thrustmaster
    When flightctl lists devices with LANG set to ja_JP.UTF-8
    Then the vendor name SHALL appear as Thrustmaster without modification  @AC-253.6
  Scenario: Missing translation string falls back to English without crash
    Given a message key that exists in English but not in the active locale catalog
    When the CLI attempts to display that message
    Then the English string SHALL be displayed and no panic or crash SHALL occur
