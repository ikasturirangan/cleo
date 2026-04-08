# Slit Lamp Control System Plan

## Goal

Build a remote-controlled ophthalmic slit lamp microscope with:

- a Raspberry Pi Zero 2 W camera module acting as a USB device
- a BeagleBone Black as the main hardware controller
- a DLP2000 projector for programmable slit generation
- a TMC2209-based motion subsystem for focus and positioning
- a web application for operator control, monitoring, and calibration

The production target is a system that boots reliably, exposes deterministic hardware behavior, and can be deployed from versioned artifacts in this repository.

## Current State

### Completed

- Repository initialized and pushed to GitHub
- Raspberry Pi camera firmware scaffolded under `firmware/pi-camera`
- Pi runtime implemented in Rust
- Pi install flow implemented with OS integration and `systemd`
- Prebuilt Linux ARM64 Pi binary workflow added so the Pi does not need to compile Rust locally
- Pi boot configuration corrected for gadget-capable USB mode

### Blocked

- USB host enumeration is not reliable in the current hardware path
- `/sys/class/udc` remains empty in repeated tests with macOS and Windows
- The host machines do not currently detect the Pi as a USB device

### Tracking

- The current USB enumeration failure is documented in [`error.md`](/Users/ikasturirangan/Desktop/cleo/error.md)

## System Architecture

### Raspberry Pi Zero 2 W

Primary role:

- capture video from the Raspberry Pi Camera Module 3
- expose the camera stream over USB
- eventually provide a bidirectional control/data channel over the same cable

Current implementation direction:

- Rust runtime
- `uvc-gadget` for UVC transport
- `systemd` for lifecycle

Planned evolution:

- Phase 1: `UVC` only
- Phase 2: composite `UVC + CDC ECM`

### BeagleBone Black

Primary role:

- act as USB host for the Pi camera
- control the DLP2000
- control the TMC2209
- own the local device control plane
- bridge hardware control to the web application

Recommended implementation direction:

- Rust for controller runtime, state machine, and transport layers
- C or C++ only where vendor interfaces make it unavoidable

### DLP2000

Primary role:

- generate programmable slit patterns
- support brightness, width, angle, scan, and alignment workflows

Interface expectations:

- parallel RGB path
- I2C configuration/control path

### TMC2209

Primary role:

- focus or stage positioning
- repeatable motion with homing and safety limits

Interface expectations:

- UART configuration
- step/dir motion path if needed
- stall or endstop backed homing strategy

### Web Application

Primary role:

- operator UI
- live view display
- slit pattern controls
- motor control
- calibration tools
- device health and logs

Constraint:

- the Pi should not need the webapp in its working tree
- Pi checkout must remain firmware-only via sparse checkout or artifact install

## Repository Plan

Target repository layout:

```text
firmware/
  pi-camera/
  bbb-controller/
  shared/
webapp/
docs/
plan.md
error.md
README.md
```

### `firmware/pi-camera`

Owns:

- Pi camera runtime
- USB gadget setup
- install and service assets
- prebuilt Pi binaries

### `firmware/bbb-controller`

Owns:

- camera ingestion from the Pi over USB
- DLP pattern engine
- TMC2209 motion control
- local device server
- hardware watchdogs and safety logic

### `firmware/shared`

Owns:

- wire protocol definitions
- calibration schemas
- common constants
- message and telemetry structures

### `webapp`

Owns:

- frontend UI
- backend API
- operator workflow
- device control orchestration

## Technical Direction

### Primary Language Choices

- Rust for Pi firmware
- Rust for BeagleBone controller runtime
- TypeScript for webapp

Why Rust:

- high performance without GC pauses
- strong type safety around hardware state transitions
- good fit for long-running services
- easy to ship static or mostly self-contained binaries

Where C or C++ may still appear:

- vendor SDK bindings
- low-level DLP or Linux interface shims if required

### USB Strategy

#### Near Term

- stabilize `UVC` enumeration on at least one host

#### Production Direction

- move from `UVC-only` to composite `UVC + CDC ECM`

Why `CDC ECM` instead of `CDC ACM`:

- proper bidirectional IP transport
- easier structured control plane
- easier future telemetry and remote diagnostics
- clean separation of video stream and control traffic

### Video Strategy

- Pi handles sensor-side capture
- host sees a standards-based USB camera
- BeagleBone consumes the camera as a normal Linux video device
- webapp receives video through the BeagleBone stack, not directly from the Pi

### Control Strategy

- BeagleBone is the single system authority
- Pi is treated as a camera appliance, not the system brain
- webapp sends commands to BeagleBone
- BeagleBone coordinates DLP, motion, capture, and safety

## Project Phases

## Phase 0: Bring-Up and Hardware Validation

Objective:

- prove each hardware subsystem is real, connected, and electrically stable

Deliverables:

- Pi camera detected reliably
- Pi USB gadget mode enters a valid host-connected state
- BeagleBone can see the Pi over USB
- DLP2000 can be reset and initialized
- TMC2209 responds over UART

Acceptance criteria:

- every subsystem has a repeatable smoke test
- all power and cabling assumptions are documented
- no subsystem requires manual guessing to recover after reboot

## Phase 1: Pi Camera Appliance MVP

Objective:

- make the Pi behave like a deployable USB camera appliance

Deliverables:

- stable Pi firmware install flow
- prebuilt Linux ARM64 artifact support
- `systemd` runtime with logs and cleanup
- UVC gadget working on at least one known-good host

Acceptance criteria:

- cold boot to active service without manual intervention
- host sees the Pi as a camera
- service survives disconnect and reconnect events

## Phase 2: Composite USB Transport

Objective:

- add bidirectional data over the same USB cable

Deliverables:

- composite `UVC + CDC ECM` gadget configuration
- host-side network interface bring-up
- Pi-side lightweight control service
- message protocol for health, sync, and commands

Acceptance criteria:

- one cable carries video and control data
- host can send structured commands to the Pi
- Pi can return telemetry, health, and configuration state

## Phase 3: BeagleBone Controller Foundation

Objective:

- create the BeagleBone main runtime

Deliverables:

- `firmware/bbb-controller` project scaffold
- hardware abstraction layers
- structured configuration
- logging and metrics
- watchdog process model

Acceptance criteria:

- BeagleBone runtime boots as a service
- config is versioned and environment-independent
- logs are actionable and not ad hoc

## Phase 4: Camera Ingest on BeagleBone

Objective:

- consume the Pi camera from the BeagleBone

Deliverables:

- USB camera discovery logic
- `/dev/video*` selection and validation
- capture pipeline
- frame timing instrumentation

Acceptance criteria:

- BeagleBone can reliably ingest frames from the Pi
- device discovery survives unplug/replug
- frame drop rate and latency are measurable

## Phase 5: DLP2000 Control

Objective:

- make slit pattern projection programmable and deterministic

Deliverables:

- display initialization sequence
- pattern generation engine
- slit width, offset, brightness, and orientation control
- test patterns and calibration patterns

Acceptance criteria:

- pattern changes are reproducible
- startup and shutdown are deterministic
- projection state can be queried and restored

## Phase 6: Motion Control with TMC2209

Objective:

- add safe repeatable motion

Deliverables:

- UART communication layer
- motion profile layer
- homing sequence
- safety limits and fault handling

Acceptance criteria:

- motor can home repeatably
- motion commands are bounded and validated
- fault states stop movement and surface clear diagnostics

## Phase 7: Unified Device Control Plane

Objective:

- expose one coherent API for camera, projection, and motion

Deliverables:

- BeagleBone local API server
- command and telemetry schema
- state machine for run modes
- calibration and preset persistence

Acceptance criteria:

- external clients talk only to the BeagleBone
- device state is observable and recoverable
- command sequencing prevents invalid hardware combinations

## Phase 8: Web Application

Objective:

- provide the operator interface

Deliverables:

- live view UI
- slit controls
- motion controls
- system status and logs
- calibration workflows

Acceptance criteria:

- operators can complete the basic exam workflow from the web UI
- UI reflects device state accurately
- errors are surfaced clearly and recoverably

## Phase 9: Production Hardening

Objective:

- make the stack deployable beyond the lab bench

Deliverables:

- build artifacts and release flow
- versioned configs
- rollback strategy
- service recovery and watchdog behavior
- manufacturing or setup checklist

Acceptance criteria:

- a known release can be installed repeatably
- field logs are enough to diagnose failures
- the system can recover from common disconnects and power events

## Major Workstreams

### Workstream A: Pi Camera Firmware

Tasks:

- resolve physical USB enumeration failure
- stabilize UVC gadget lifecycle
- add composite gadget mode
- define Pi control endpoint behavior

### Workstream B: BeagleBone Controller

Tasks:

- scaffold runtime
- integrate camera ingest
- integrate DLP2000
- integrate TMC2209
- expose control API

### Workstream C: Webapp

Tasks:

- define UI architecture
- build live view and control panels
- integrate API client
- add calibration and diagnostics screens

### Workstream D: System Integration

Tasks:

- define cable and power architecture
- validate boot ordering
- test hotplug behavior
- validate latency and synchronization

## Milestones

### Milestone 1: Pi Enumerates Reliably

Success means:

- at least one host consistently detects the Pi gadget
- Pi no longer loses the host connection across reboots

### Milestone 2: One-Cable Video + Control

Success means:

- composite `UVC + CDC ECM` working
- host can talk back to the Pi on the same cable

### Milestone 3: BeagleBone Owns The Stack

Success means:

- BeagleBone ingests camera, drives DLP, drives motion, exposes one API

### Milestone 4: Operator Workflow Runs End-To-End

Success means:

- live view
- slit adjustment
- motion adjustment
- calibration save/load

## Interfaces and Protocols

### Pi to Host

Current:

- `UVC`

Planned:

- `UVC + CDC ECM`

### Host to Pi

Planned:

- IP-based control channel over ECM
- simple binary or JSON message protocol

### Webapp to BeagleBone

Planned:

- HTTP or WebSocket API
- explicit command schema
- telemetry streaming for state updates

## Testing Plan

### Unit Tests

- Rust config parsing
- state machine logic
- message schema validation

### Hardware Integration Tests

- Pi camera detection
- USB enumeration
- BeagleBone video ingest
- DLP pattern switching
- TMC2209 motion and homing

### End-To-End Tests

- cold boot system startup
- reconnect after host unplug
- apply slit change while viewing live feed
- move optics and confirm updated image

### Operational Tests

- repeated reboot stability
- long-duration streaming
- thermal and power stability
- log completeness during failures

## Risks

### USB Enumeration Risk

- current highest blocker
- likely hardware or power-path related

### Power Integrity Risk

- host USB ports may not supply stable power
- one-cable operation may be insufficient for reliable bring-up

### DLP2000 Integration Risk

- vendor initialization details may be time-consuming
- Linux userspace path may require tight coordination with low-level interfaces

### Motion Safety Risk

- incorrect motion control can damage optics or mechanisms
- requires limit logic before any aggressive control work

## Immediate Next Actions

1. Resolve the Phase 0 USB host detection issue using separate power into `PWR IN` and a direct data connection into the Pi `USB` port.
2. Run the official `rpi-usb-gadget` Ethernet gadget package as a control experiment to separate firmware issues from hardware-path issues.
3. Once one host enumerates reliably, return to the Pi camera firmware and complete UVC validation.
4. Start `firmware/bbb-controller` with a Rust scaffold and clear module boundaries.
5. Define the composite `UVC + CDC ECM` USB plan before implementing bidirectional control.

## Definition of Done

The project is production-ready when:

- Pi firmware can be deployed from versioned artifacts without on-device toolchain work
- BeagleBone owns all control logic and presents a stable API
- one USB cable can carry video and control data when required
- webapp can operate the slit lamp end to end
- hardware failures produce actionable diagnostics instead of silent misbehavior
