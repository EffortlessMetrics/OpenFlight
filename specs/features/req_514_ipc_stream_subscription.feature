@REQ-514 @product
Feature: IPC Stream Subscription

  @AC-514.1 @AC-514.2
  Scenario: Client subscribes and receives axis value stream at configured rate
    Given an IPC client connected to the service
    When the client calls SubscribeAxisValues with a max rate of 50 Hz
    Then the client SHALL receive axis value updates at no more than 50 Hz

  @AC-514.3
  Scenario: Client disconnect cancels the subscription cleanly
    Given an IPC client with an active axis value stream subscription
    When the client disconnects
    Then the subscription SHALL be cancelled with no resource leaks

  @AC-514.4
  Scenario: Multiple concurrent subscribers are supported
    Given three IPC clients each subscribing to axis value streams
    When all three subscriptions are active simultaneously
    Then each client SHALL receive independent axis value updates without interference
