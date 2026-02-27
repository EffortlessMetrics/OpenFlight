@REQ-549 @product
Feature: IPC Message Authentication — IPC channel should support authenticated connections

  @AC-549.1
  Scenario: Service issues tokens for CLI authentication
    Given the service is running with authentication enabled
    When a CLI client requests an authentication token
    Then the service SHALL issue a signed token to the client

  @AC-549.2
  Scenario: Tokens have configurable TTL
    Given an authentication token TTL of 3600 seconds is configured
    When a token is issued
    Then the token SHALL expire after 3600 seconds

  @AC-549.3
  Scenario: Unauthenticated requests are rejected with 401 error
    Given an IPC request with no authentication token
    When the request reaches the service
    Then the service SHALL respond with a 401 Unauthenticated error

  @AC-549.4
  Scenario: Token revocation is immediate
    Given a valid authentication token has been issued
    When the token is revoked via the CLI
    Then subsequent requests using that token SHALL be rejected immediately
