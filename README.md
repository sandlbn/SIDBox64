# DrumBox64

A desktop port of the C64 [drumbox64](https://github.com/) drum sequencer

7-track / 16-step pattern sequencer with swing, 4 kits, 36 built-in presets, MIDI out, and optional USBSID-Pico hardware playback.

## Features

- **7 tracks** — Kick, Snare, Closed Hat, Open Hat, Tom, Clap, Crash
- **16-step grid** with 4 velocity levels (Off / Soft / Medium / Loud)
- **Swing timing** — even/odd step pairs sum to a constant tempo
- **4 kits** — TR-909, TR-808, Rock, SID — ported from the original C64 synthesis params
- **36 built-in presets** — every pattern from the original drumbox64
- **MIDI out** — GM drum map on channel 10 (Logic, Ableton, hardware modules)
- **MIDI clock IN/OUT** — sync to or from a DAW (24 PPQN)
- **Per-track volume + pan** — saved with the pattern
- **ASID / USBSID-Pico** — real C64 SID chip playback over USB (optional)
- **Save / load** — backward-compatible `.db64` binary file format

## Build & run

```sh
cargo run -p drumbox64
```

With USBSID-Pico hardware:

```sh
cargo run -p drumbox64 --features hardware
```

The `core` crate has no audio or UI dependencies, so it can be wrapped in an AU/VST shell later.

## Hardware notes

USBSID-Pico communicates via libusb. On Linux you may need a udev rule; on macOS the device should enumerate without setup. Connect from the **ASID → Connect** button in the UI; the status bar reports success or the underlying error.

## File format

`.db64` patterns are 149 bytes — magic + kit + tempo + swing + name + 16-step velocities for 7 tracks + per-track volume + per-track pan. Old 135-byte files (without vol/pan) still load with sensible defaults.
