# zoom-midi-adaptor (Rust)

[![Rust CI](https://github.com/your-username/python-zoom-midi-adaptor/actions/workflows/rust.yml/badge.svg)](https://github.com/your-username/python-zoom-midi-adaptor/actions/workflows/rust.yml)

MIDI adaptor for routing incoming controller messages to a Zoom MS-50G.

## Behavior

- Listens to one configured MIDI channel (`targetOutputDeviceChannel`, 1-16)
- Forwards matching Program Change messages unchanged
- Translates mapped Control Change messages into MS-50G-safe output:
  - `changeEffectParam` -> SysEx parameter edit (`31`)
  - `effectOnOff` -> SysEx parameter edit on `param 0` (<=64 off, >64 on)
  - `tunerOnOff` -> CC74 on the configured channel
- Sends startup edit-mode SysEx (`F0 52 00 58 50 F7`) on boot

## Config

Default config path: `config/pedalboard.json`

You can pass a custom config path as the first argument:

```bash
zoom-midi-adaptor /path/to/config.json
```

### Config fields

- `targetInputDevices`: array of input device name prefixes (first match wins)
- `targetOutputDevice`: output device prefix string or array (first match wins)
- `targetOutputDeviceChannel`: MIDI channel 1-16
- `changeControlMaps`: CC mapping rules

`changeControlMaps` action values:

- `tunerOnOff` needs: `ccNum`
- `effectOnOff` needs: `ccNum`, `effectPosition` (0-5)
- `changeEffectParam` needs: `ccNum`, `effectPosition` (0-5), `paramNum` (0-127)

## Development

```bash
cargo fmt
cargo test
cargo build --release
```

Binary output:

```bash
target/release/zoom-midi-adaptor
```

## Raspberry Pi systemd setup

The repository includes `zoom-midi-adaptor.service` tuned for:

- fast boot startup (no network dependency)
- automatic restart if MIDI devices are not ready yet
- reliable long-running operation

### Install

```bash
sudo install -Dm755 target/release/zoom-midi-adaptor /usr/local/bin/zoom-midi-adaptor
sudo install -d /etc/zoom-midi-adaptor
sudo install -m644 config/pedalboard.json /etc/zoom-midi-adaptor/pedalboard.json
sudo install -Dm644 zoom-midi-adaptor.service /etc/systemd/system/zoom-midi-adaptor.service
```

Optional override file (if you want a different config path):

```bash
echo 'ZOOM_MIDI_CONFIG=/etc/zoom-midi-adaptor/pedalboard.json' | sudo tee /etc/default/zoom-midi-adaptor
```

Enable and start:

```bash
sudo systemctl daemon-reload
sudo systemctl enable --now zoom-midi-adaptor
```

Check status and logs:

```bash
sudo systemctl status zoom-midi-adaptor
journalctl -u zoom-midi-adaptor -f
```

If your Pi user is not `pi`, edit `User=` and `Group=` in the service file before enabling.

## Logging

The app logs with millisecond timestamps and, for each forwarded/translated message, logs:

- a human-readable input message (`PC <value>` or `CC <param> <value>`)
- input bytes in hex (`hex_in=[..]`)
- output bytes in hex (`hex_out=[..]`)

Example:

```text
2026-06-26T16:55:12.123Z INFO midi in=CC 11 99  hex_in=[B2 0B 63] hex_out=[F0 52 00 58 31 00 02 63 00 F7]
```

Default log level is `info`. Override with `RUST_LOG`, for example:

```bash
RUST_LOG=debug cargo run -- config/pedalboard.json
```
