# Project Guidelines

## Purpose and Priorities

This project is a Raspberry Pi MIDI adaptor for a Zoom MS-50G pedal.

Priorities, in order:
1. Reliability for live gigs
2. Runtime speed (near-realtime MIDI forwarding/translation)
3. Startup speed (fast recovery after reboot)

## Protocol and Behavior Rules

- Target pedal is always **Zoom MS-50G (original)**.
- Do not add startup device identity probing unless explicitly requested.
- Only process messages on the configured channel (`targetOutputDeviceChannel`, 1-16).
- Program Change handling is fixed: **forward matching PC messages unchanged**.
- Do not add PC mapping/remapping features unless explicitly requested.

### Allowed outbound message patterns (whitelist)

- Forwarded Program Change (`0xC? <value>`)
- Startup SysEx enable edit mode: `F0 52 00 58 50 F7`
- Parameter change SysEx: `F0 52 00 58 31 <effect> <param> <value> 00 F7`
- Tuner control mapped to CC74 on same channel (`0xB? 0x4A <value>`)

Do not introduce risky/unknown SysEx commands (firmware mode, reset, factory wipe, raw passthrough).

## Config Conventions

- Default runtime config path: `config/pedalboard.json`
- Keep compatibility with the existing pedalboard JSON shape:
  - `targetInputDevices`
  - `targetOutputDevice`
  - `targetOutputDeviceChannel`
  - `changeControlMaps`
- Supported CC actions:
  - `tunerOnOff`
  - `effectOnOff`
  - `changeEffectParam`
- Keep strict config validation (channel range, required fields, duplicate CC maps).

## Performance and Reliability Expectations

- Keep hot-path MIDI handling simple and allocation-light.
- Avoid blocking work in message callbacks.
- Fail fast on invalid config or missing required devices.
- Preserve deterministic behavior; avoid hidden fallback behavior.

## Build and Test

Run before finishing changes:

```bash
cargo fmt --all
cargo test
cargo build --release
```

## Deployment Notes

- Systemd unit file: `zoom-midi-adaptor.service`
- Binary install target: `/usr/local/bin/zoom-midi-adaptor`
- Default deployed config path: `/etc/zoom-midi-adaptor/pedalboard.json`
