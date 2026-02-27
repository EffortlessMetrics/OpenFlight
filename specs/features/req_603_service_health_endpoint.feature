Feature: Service Health Endpoint
  As a flight simulation enthusiast
  I want the service to expose an HTTP health check endpoint
  So that monitoring tools and orchestrators can determine service health

  Background:
    Given the OpenFlight service is running with the health endpoint enabled

  Scenario: GET /health returns 200 OK when service is running normally
    Given the service is in a healthy operational state
    When an HTTP GET request is sent to /health
    Then the response status code is 200 OK

  Scenario: GET /health returns 503 when service is in degraded mode
    Given the service has entered a degraded operational state
    When an HTTP GET request is sent to /health
    Then the response status code is 503 Service Unavailable

  Scenario: Health response body includes service version and status
    When an HTTP GET request is sent to /health
    Then the JSON response body contains the service version and current status fields

  Scenario: Health endpoint is on configurable port separate from metrics
    Given the service config sets health_port to 9091 and metrics_port to 9090
    When the service starts
    Then the health endpoint is reachable on port 9091
    And the metrics endpoint is reachable on port 9090
