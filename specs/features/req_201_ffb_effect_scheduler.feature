@REQ-201 @product
Feature: FFB effect scheduler queues and plays effects without RT spine blocking  @AC-201.1
  Scenario: Up to 16 simultaneous FFB effects can be queued
    Given the FFB effect scheduler is running
    When 16 distinct effects are submitted to the queue
    Then all 16 effects SHALL be held in the queue without any being dropped  @AC-201.2
  Scenario: Effect scheduler runs off RT spine in dedicated thread
    Given the RT spine is ticking at 250Hz
    When the FFB scheduler submits an effect to the hardware
    Then the RT spine tick latency SHALL be unaffected by the FFB submission  @AC-201.3
  Scenario: New effect starts within 20ms of queue submission
    Given the FFB scheduler is idle
    When a new effect is submitted to the queue
    Then the effect SHALL begin playing within 20 milliseconds of submission  @AC-201.4
  Scenario: Effect cancellation removes effect within one tick
    Given an FFB effect is currently active in the scheduler queue
    When a cancellation request is issued for that effect
    Then the effect SHALL be removed from the queue within one scheduler tick  @AC-201.5
  Scenario: Effect priority ordering is emergency over tactile over ambiance
    Given effects of priority ambiance, tactile, and emergency are all queued
    When the scheduler selects the next effect to play
    Then the emergency effect SHALL play before tactile and tactile before ambiance  @AC-201.6
  Scenario: Scheduler degrades gracefully when FFB device disconnected
    Given the FFB scheduler has queued effects
    When the FFB device is disconnected
    Then the scheduler SHALL discard pending effects and log a warning without crashing
