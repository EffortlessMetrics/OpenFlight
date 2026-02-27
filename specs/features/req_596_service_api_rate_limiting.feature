Feature: Service API Rate Limiting
  As a flight simulation enthusiast
  I want the service gRPC API to support rate limiting per client
  So that no single client can overwhelm the service with requests

  Background:
    Given the OpenFlight service is running with rate limiting enabled

  Scenario: Configurable request rate limit per connected client
    Given the service config sets a rate limit of 100 requests per second per client
    When a client connects
    Then the client is subject to the configured rate limit

  Scenario: Rate limited clients receive gRPC RESOURCE_EXHAUSTED error
    Given a client has exceeded its configured request rate limit
    When the client sends an additional gRPC request
    Then the service responds with a gRPC RESOURCE_EXHAUSTED status code

  Scenario: Rate limit applies separately to different RPC methods
    Given per-method rate limits are configured
    When a client exceeds the rate limit for one RPC method
    Then only requests to that method are throttled and other methods remain unaffected

  Scenario: Rate limit state is visible in service metrics
    Given a client has been rate limited
    When the service metrics endpoint is queried
    Then the rate limit hit count for that client is reflected in the metrics output
