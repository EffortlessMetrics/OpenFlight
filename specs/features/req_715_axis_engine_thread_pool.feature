Feature: Axis Engine Thread Pool
  As a flight simulation enthusiast
  I want axis processing to support configurable worker thread count
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Worker thread count is configurable
    Given the service configuration specifies a worker thread count
    When the axis engine starts
    Then it creates the configured number of worker threads

  Scenario: Work is distributed across threads
    Given the axis engine is running with multiple worker threads
    When multiple axes are processed simultaneously
    Then the work is distributed across the configured threads

  Scenario: Default thread count uses physical cores
    Given no worker thread count is specified in configuration
    When the axis engine starts
    Then the thread count defaults to the number of physical cores minus one

  Scenario: Thread count change requires restart
    Given the service is running with a configured thread count
    When the thread count configuration is changed
    Then the change takes effect on the next service restart
