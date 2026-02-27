Feature: Axis Output Event Stream
  As a flight simulation enthusiast
  I want axis outputs to be streamable as structured events
  So that external tools can monitor and react to axis values in real time

  Background:
    Given the OpenFlight service is running with axis inputs active

  Scenario: Event stream contains axis-id, value, and stage-values
    Given a client subscribes to the axis output event stream
    When an axis produces an output value
    Then the event contains the axis ID, final output value, and per-stage intermediate values

  Scenario: Stream is delivered via gRPC server-side streaming
    When a client connects to the StreamAxisOutputs gRPC endpoint
    Then axis output events are delivered as a continuous server-side stream

  Scenario: Stream backpressure drops oldest events if consumer is slow
    Given a client is consuming the axis output stream slowly
    When the axis engine produces events faster than the client consumes
    Then the oldest undelivered events are dropped to relieve backpressure

  Scenario: Event stream can be paused and resumed by client
    Given a client is subscribed to the axis output stream
    When the client sends a pause request
    Then the stream stops delivering events until a resume request is sent
