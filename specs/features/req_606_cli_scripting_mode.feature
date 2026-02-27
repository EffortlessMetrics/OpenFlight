Feature: OpenFlight CLI Scripting Mode
  As a flight simulation enthusiast
  I want the CLI to support a scripting mode for automation
  So that I can use flightctl reliably in scripts and CI pipelines

  Background:
    Given the OpenFlight service is running

  Scenario: flightctl --script outputs minimal formatted results
    When a flightctl command is run with the "--script" flag
    Then the output is minimal and machine-parseable without decorative formatting

  Scenario: Script mode suppresses progress indicators and color
    When a flightctl command is run with the "--script" flag
    Then no spinner, progress bar, or ANSI color codes appear in the output

  Scenario: Script mode exits with non-zero code on errors
    Given a flightctl command that encounters an error
    When it is run with the "--script" flag
    Then the process exits with a non-zero exit code

  Scenario: Script mode is documented in CLI help
    When the command "flightctl --help" is run
    Then the output documents the "--script" flag and its behaviour
