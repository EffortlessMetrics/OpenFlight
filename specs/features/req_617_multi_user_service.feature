Feature: Multi-User Service Mode
  As a flight simulation enthusiast
  I want OpenFlight to support multi-user installations on shared hardware
  So that multiple users can each have their own profiles on the same machine

  Background:
    Given the OpenFlight service is installed on a shared machine

  Scenario: Each user has isolated profile directory
    Given two users are configured on the system
    When each user loads their profiles
    Then each user's profiles are stored in their own isolated directory

  Scenario: Shared hardware devices are accessible to all users
    Given a joystick is connected to the shared machine
    When any configured user runs the service
    Then the joystick is accessible to that user

  Scenario: Per-user service instance is supported on Linux via systemd user
    Given the service is installed on Linux
    When a user starts their own service instance via systemd user
    Then the service runs in that user's context with their profile directory

  Scenario: Concurrent multi-user access does not cause device contention
    Given two user service instances are running simultaneously
    When both instances attempt to access the same device
    Then no device contention errors occur
