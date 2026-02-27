Feature: Axis Output Monitoring RPC
  As a developer or advanced user
  I want a gRPC stream for real-time axis output monitoring
  So that I can observe live axis values for diagnostics and calibration

  Background:
    Given the OpenFlight gRPC service is running

  Scenario: StreamAxisOutputs RPC sends output values at configurable rate
    When a client calls StreamAxisOutputs with a specified rate
    Then the stream delivers axis output values at approximately the requested rate

  Scenario: Stream includes axis ID, raw, pipeline, and output values
    When a client subscribes to StreamAxisOutputs
    Then each streamed message includes the axis ID, raw value, pipeline value, and final output value

  Scenario: Stream rate is bounded to 100Hz maximum
    When a client requests a stream rate above 100Hz
    Then the stream rate is capped at 100Hz

  Scenario: Stream handles client disconnect without crashing
    Given a client is subscribed to StreamAxisOutputs
    When the client disconnects abruptly
    Then the service continues operating without crashing or resource leaks
