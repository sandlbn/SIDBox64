mod theme;

use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::mpsc::SyncSender;
use std::sync::{Arc, Mutex};

use iced::{
    widget::{button, column, container, pick_list, row, slider, text, Space},
    Background, Element, Length, Subscription, Task, Theme,
};

use drumbox64_asid::AsidBackend;
use drumbox64_core::{
    presets, sequencer::step_duration_ms, Kit, Pattern, Track, Transport, NUM_STEPS, NUM_TRACKS,
};
use drumbox64_midi::{MidiBackend, MidiCommand, MidiInBackend, MidiInEvent};

const CLOCK_MS: u64 = 5;
/// 24 PPQN MIDI clocks per quarter note; the sequencer fires a step every 16th note,
/// i.e. every (24 / 4) = 6 incoming clocks.
const MIDI_CLOCKS_PER_STEP: u64 = 6;

struct DrumBox64 {
    pattern: Pattern,
    transport: Transport,
    tick_acc_ms: u64,
    /// Accumulator (microseconds) for MIDI clock OUT scheduling.
    midi_clock_acc_us: u64,

    presets: Vec<Pattern>,
    preset_index: usize,

    output_mode: OutputMode,

    // Per-track mixer
    track_volume: [u8; NUM_TRACKS], // 0..=127, default 100
    track_pan: [i8; NUM_TRACKS],    // -64..=63, default 0

    // MIDI OUT
    midi_ports: Vec<String>,
    selected_port_idx: Option<usize>,
    midi_sender: Option<SyncSender<MidiCommand>>,

    // MIDI IN (clock source)
    midi_in_ports: Vec<String>,
    selected_in_port_idx: Option<usize>,
    midi_in_backend: Option<MidiInBackend>,
    midi_in_queue: Arc<Mutex<VecDeque<MidiInEvent>>>,
    midi_in_clock_count: u64,

    // ASID
    asid_backend: Option<AsidBackend>,

    bpm_text: String,
    status_msg: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum OutputMode {
    #[default]
    Midi,
    Asid,
}

impl std::fmt::Display for OutputMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OutputMode::Midi => write!(f, "MIDI"),
            OutputMode::Asid => write!(f, "ASID"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PresetItem {
    index: usize,
    label: String,
}
impl std::fmt::Display for PresetItem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.label)
    }
}

#[derive(Debug, Clone)]
enum Message {
    StepToggled { track: usize, step: usize },
    PlayPressed,
    StopPressed,
    TempoSlider(f32),
    SwingChanged(f32),
    KitSelected(Kit),
    TrackVolumeChanged(usize, f32),
    TrackPanChanged(usize, f32),
    PresetSelected(PresetItem),
    OutputModeChanged(OutputMode),
    MidiPortSelected(String),
    MidiInPortSelected(String),
    MidiInDisconnect,
    AsidConnect,
    SavePressed,
    LoadPressed,
    SaveConfirmed(PathBuf),
    LoadConfirmed(PathBuf),
    ClockTick,
    Noop,
}

fn main() -> iced::Result {
    iced::application(init, update, view)
        .title(|_: &DrumBox64| "DrumBox64".to_string())
        .subscription(subscription)
        .theme(Theme::Dark)
        .window(iced::window::Settings {
            size: iced::Size::new(1180.0, 580.0),
            resizable: true,
            ..Default::default()
        })
        .run()
}

fn init() -> (DrumBox64, Task<Message>) {
    let all_presets = presets::all();
    let first = all_presets[0].clone();
    let midi_ports = MidiBackend::list_ports();
    let midi_in_ports = MidiInBackend::list_ports();
    let bpm_text = first.tempo.to_string();
    let state = DrumBox64 {
        transport: Transport {
            bpm: first.tempo,
            swing: first.swing,
            ..Default::default()
        },
        pattern: first,
        tick_acc_ms: 0,
        midi_clock_acc_us: 0,
        presets: all_presets,
        preset_index: 0,
        output_mode: OutputMode::Midi,
        track_volume: [100; NUM_TRACKS],
        track_pan: [0; NUM_TRACKS],
        midi_ports,
        selected_port_idx: None,
        midi_sender: None,
        midi_in_ports,
        selected_in_port_idx: None,
        midi_in_backend: None,
        midi_in_queue: Arc::new(Mutex::new(VecDeque::new())),
        midi_in_clock_count: 0,
        asid_backend: None,
        bpm_text,
        status_msg: String::new(),
    };
    (state, Task::none())
}

fn update(state: &mut DrumBox64, msg: Message) -> Task<Message> {
    match msg {
        Message::StepToggled { track, step } => {
            state.pattern.steps[track][step] = state.pattern.steps[track][step].cycle();
        }

        Message::PlayPressed => {
            state.transport.playing = true;
            state.transport.current_step = 0;
            state.tick_acc_ms = 0;
            state.midi_clock_acc_us = 0;
            state.midi_in_clock_count = 0;
            send_midi(state, MidiCommand::Start);
            fire_step(state, 0);
        }
        Message::StopPressed => {
            state.transport.playing = false;
            state.transport.current_step = 0;
            send_midi(state, MidiCommand::Stop);
            if let Some(ref mut backend) = state.asid_backend {
                backend.mute();
            }
        }

        Message::TempoSlider(v) => {
            let bpm = v as u16;
            state.transport.bpm = bpm;
            state.pattern.tempo = bpm;
            state.bpm_text = bpm.to_string();
        }
        Message::SwingChanged(v) => {
            let sw = v as u8;
            state.transport.swing = sw;
            state.pattern.swing = sw;
        }
        Message::KitSelected(kit) => {
            state.pattern.kit = kit;
            if let Some(ref mut backend) = state.asid_backend {
                backend.set_kit(kit);
            }
        }
        Message::TrackVolumeChanged(t, v) => {
            state.track_volume[t] = v.clamp(0.0, 127.0) as u8;
        }
        Message::TrackPanChanged(t, v) => {
            state.track_pan[t] = v.clamp(-64.0, 63.0) as i8;
        }

        Message::PresetSelected(item) => {
            let preset = state.presets[item.index].clone();
            state.transport.bpm = preset.tempo;
            state.transport.swing = preset.swing;
            state.bpm_text = preset.tempo.to_string();
            if let Some(ref mut backend) = state.asid_backend {
                backend.set_kit(preset.kit);
            }
            state.pattern = preset;
            state.preset_index = item.index;
            if state.transport.playing {
                state.transport.playing = false;
                send_midi(state, MidiCommand::Stop);
            }
        }

        Message::OutputModeChanged(mode) => {
            state.output_mode = mode;
            match mode {
                OutputMode::Midi => {
                    if let Some(ref mut backend) = state.asid_backend {
                        backend.mute();
                        backend.reset();
                    }
                    state.asid_backend = None;
                }
                OutputMode::Asid => {
                    state.midi_sender = None;
                }
            }
        }

        Message::MidiPortSelected(port_name) => {
            state.midi_sender = None;
            match MidiBackend::connect(&port_name) {
                Ok(backend) => {
                    let idx = state.midi_ports.iter().position(|p| p == &port_name);
                    state.selected_port_idx = idx;
                    let (tx, rx) = std::sync::mpsc::sync_channel::<MidiCommand>(256);
                    std::thread::spawn(move || {
                        let mut backend = backend;
                        for cmd in rx {
                            backend.handle(cmd);
                        }
                    });
                    state.midi_sender = Some(tx);
                    state.status_msg = format!("MIDI out: {port_name}");
                }
                Err(e) => {
                    state.status_msg = format!("MIDI error: {e}");
                }
            }
        }

        Message::MidiInPortSelected(port_name) => {
            state.midi_in_backend = None;
            let queue = Arc::clone(&state.midi_in_queue);
            match MidiInBackend::connect(&port_name, move |ev| {
                if let Ok(mut q) = queue.lock() {
                    q.push_back(ev);
                }
            }) {
                Ok(backend) => {
                    let idx = state.midi_in_ports.iter().position(|p| p == &port_name);
                    state.selected_in_port_idx = idx;
                    state.midi_in_backend = Some(backend);
                    state.midi_in_clock_count = 0;
                    state.status_msg = format!("MIDI clock in: {port_name}");
                }
                Err(e) => {
                    state.status_msg = format!("MIDI in error: {e}");
                }
            }
        }
        Message::MidiInDisconnect => {
            state.midi_in_backend = None;
            state.selected_in_port_idx = None;
            state.midi_in_clock_count = 0;
            state.status_msg = "MIDI clock in: internal".to_string();
        }

        Message::AsidConnect => match AsidBackend::try_connect(state.pattern.kit) {
            Ok(backend) => {
                state.asid_backend = Some(backend);
                state.status_msg = "USBSID-Pico connected".to_string();
            }
            Err(e) => {
                state.status_msg = format!("ASID: {e}");
            }
        },

        Message::SavePressed => {
            return Task::future(async move {
                if let Some(f) = rfd::AsyncFileDialog::new()
                    .add_filter("DrumBox64 Pattern", &["db64"])
                    .set_file_name("pattern.db64")
                    .save_file()
                    .await
                {
                    Message::SaveConfirmed(f.path().to_path_buf())
                } else {
                    Message::Noop
                }
            });
        }
        Message::LoadPressed => {
            return Task::future(async move {
                if let Some(f) = rfd::AsyncFileDialog::new()
                    .add_filter("DrumBox64 Pattern", &["db64"])
                    .pick_file()
                    .await
                {
                    Message::LoadConfirmed(f.path().to_path_buf())
                } else {
                    Message::Noop
                }
            });
        }
        Message::SaveConfirmed(path) => match state.pattern.save(&path) {
            Ok(()) => state.status_msg = format!("Saved: {}", path.display()),
            Err(e) => state.status_msg = format!("Save error: {e}"),
        },
        Message::LoadConfirmed(path) => match Pattern::load(&path) {
            Ok(pat) => {
                state.transport.bpm = pat.tempo;
                state.transport.swing = pat.swing;
                state.bpm_text = pat.tempo.to_string();
                state.status_msg = format!("Loaded: {}", path.display());
                if let Some(ref mut backend) = state.asid_backend {
                    backend.set_kit(pat.kit);
                }
                state.pattern = pat;
                if state.transport.playing {
                    state.transport.playing = false;
                    send_midi(state, MidiCommand::Stop);
                }
            }
            Err(e) => state.status_msg = format!("Load error: {e}"),
        },

        Message::ClockTick => {
            // ASID voice sweep updates (regardless of playing state).
            if let Some(ref mut backend) = state.asid_backend {
                backend.tick();
            }

            // Drain incoming MIDI clock events first — they may start/stop us
            // and drive the step advance when active.
            drain_midi_in(state);

            // MIDI clock OUT: emit 24 PPQN ticks while playing.
            if state.transport.playing && state.midi_sender.is_some() {
                state.midi_clock_acc_us += CLOCK_MS * 1_000;
                let bpm = state.transport.bpm.max(1) as u64;
                let period_us = 60_000_000 / (bpm * 24);
                while state.midi_clock_acc_us >= period_us {
                    state.midi_clock_acc_us -= period_us;
                    send_midi(state, MidiCommand::Clock);
                }
            }

            // Internal step timer — only when external clock is NOT driving us.
            if state.transport.playing && state.midi_in_backend.is_none() {
                state.tick_acc_ms += CLOCK_MS;
                let next = (state.transport.current_step + 1) % 16;
                let threshold = step_duration_ms(next, state.transport.bpm, state.transport.swing);
                if state.tick_acc_ms >= threshold {
                    state.tick_acc_ms -= threshold;
                    state.transport.current_step = next;
                    fire_step(state, next);
                }
            }
        }

        Message::Noop => {}
    }
    Task::none()
}

/// Drain any pending incoming MIDI clock / transport events.
fn drain_midi_in(state: &mut DrumBox64) {
    if state.midi_in_backend.is_none() {
        return;
    }
    let events: Vec<MidiInEvent> = {
        let mut q = state.midi_in_queue.lock().unwrap();
        q.drain(..).collect()
    };
    for ev in events {
        match ev {
            MidiInEvent::Start => {
                state.transport.playing = true;
                state.transport.current_step = 0;
                state.midi_in_clock_count = 0;
                fire_step(state, 0);
            }
            MidiInEvent::Continue => {
                state.transport.playing = true;
            }
            MidiInEvent::Stop => {
                state.transport.playing = false;
                if let Some(ref mut backend) = state.asid_backend {
                    backend.mute();
                }
            }
            MidiInEvent::Clock => {
                if state.transport.playing {
                    state.midi_in_clock_count += 1;
                    if state.midi_in_clock_count % MIDI_CLOCKS_PER_STEP == 0 {
                        let next = (state.transport.current_step + 1) % 16;
                        state.transport.current_step = next;
                        fire_step(state, next);
                    }
                }
            }
        }
    }
}

fn fire_step(state: &mut DrumBox64, step: u8) {
    let events = state.pattern.events_at_step(step);
    if events.is_empty() {
        return;
    }
    match state.output_mode {
        OutputMode::Midi => {
            let notes: Vec<(u8, u8, i8)> = events
                .iter()
                .map(|ev| {
                    let t = ev.track as usize;
                    let scaled = (ev.velocity.midi_velocity() as u16
                        * state.track_volume[t] as u16
                        / 127) as u8;
                    (ev.track.midi_note(), scaled, state.track_pan[t])
                })
                .collect();
            send_midi(state, MidiCommand::Notes(notes));
        }
        OutputMode::Asid => {
            if let Some(ref mut backend) = state.asid_backend {
                for ev in &events {
                    let vol_127 = state.track_volume[ev.track as usize];
                    backend.trigger(ev, vol_127);
                }
            }
        }
    }
}

fn send_midi(state: &DrumBox64, cmd: MidiCommand) {
    if let Some(ref tx) = state.midi_sender {
        let _ = tx.try_send(cmd);
    }
}

fn subscription(state: &DrumBox64) -> Subscription<Message> {
    // Tick when playing, when ASID needs sweep updates, or when MIDI in is
    // connected (we need to drain transport events even before the user hits play).
    let needs_tick = state.transport.playing
        || (state.output_mode == OutputMode::Asid && state.asid_backend.is_some())
        || state.midi_in_backend.is_some();
    if needs_tick {
        iced::time::every(std::time::Duration::from_millis(CLOCK_MS)).map(|_| Message::ClockTick)
    } else {
        Subscription::none()
    }
}

fn view(state: &DrumBox64) -> Element<'_, Message> {
    let preset_items: Vec<PresetItem> = state
        .presets
        .iter()
        .enumerate()
        .map(|(i, p)| PresetItem {
            index: i,
            label: format!("{:02}: {}", i, p.name),
        })
        .collect();
    let selected_preset = preset_items.get(state.preset_index).cloned();

    let header = row![
        text("DrumBox64").size(24).color(theme::ACCENT),
        Space::new().width(Length::Fill),
        text("BPM").color(theme::DIM),
        slider(
            40.0f32..=280.0,
            state.transport.bpm as f32,
            Message::TempoSlider
        )
        .step(1.0f32)
        .width(Length::Fixed(140.0)),
        text(&state.bpm_text)
            .size(16)
            .color(theme::TEXT)
            .width(Length::Fixed(36.0)),
        Space::new().width(Length::Fixed(16.0)),
        text("Swing").color(theme::DIM),
        slider(
            0.0f32..=99.0,
            state.transport.swing as f32,
            Message::SwingChanged
        )
        .step(1.0f32)
        .width(Length::Fixed(100.0)),
        text(format!("{}%", state.transport.swing))
            .size(16)
            .color(theme::TEXT)
            .width(Length::Fixed(36.0)),
        Space::new().width(Length::Fixed(16.0)),
        text("Kit").color(theme::DIM),
        pick_list(&Kit::ALL[..], Some(state.pattern.kit), Message::KitSelected),
    ]
    .spacing(8)
    .align_y(iced::Alignment::Center);

    let play_btn = button(text(" ▶  PLAY ").color(theme::TEXT))
        .style(theme::control_style(state.transport.playing))
        .on_press(Message::PlayPressed);
    let stop_btn = button(text(" ■  STOP ").color(theme::TEXT))
        .style(theme::control_style(false))
        .on_press(Message::StopPressed);

    const MODES: [OutputMode; 2] = [OutputMode::Midi, OutputMode::Asid];
    let mode_list = pick_list(
        &MODES[..],
        Some(state.output_mode),
        Message::OutputModeChanged,
    );

    let backend_widget: Element<Message> = match state.output_mode {
        OutputMode::Midi => {
            let sel_name: Option<String> = state
                .selected_port_idx
                .and_then(|i| state.midi_ports.get(i))
                .cloned();
            let ports = state.midi_ports.clone();
            if ports.is_empty() {
                text("(no MIDI ports)").color(theme::DIM).into()
            } else {
                pick_list(ports, sel_name, Message::MidiPortSelected).into()
            }
        }
        OutputMode::Asid => {
            let label = if state.asid_backend.is_some() {
                " ● Connected "
            } else {
                " Connect "
            };
            button(text(label).color(theme::TEXT))
                .style(theme::control_style(state.asid_backend.is_some()))
                .on_press(Message::AsidConnect)
                .into()
        }
    };

    // MIDI clock IN picker
    let in_sel: Option<String> = state
        .selected_in_port_idx
        .and_then(|i| state.midi_in_ports.get(i))
        .cloned();
    let clock_in_widget: Element<Message> = if state.midi_in_ports.is_empty() {
        text("(no MIDI inputs)").color(theme::DIM).into()
    } else {
        pick_list(
            state.midi_in_ports.clone(),
            in_sel,
            Message::MidiInPortSelected,
        )
        .into()
    };
    let clock_in_disconnect: Element<Message> = if state.midi_in_backend.is_some() {
        button(text(" × ").color(theme::TEXT))
            .style(theme::control_style(false))
            .on_press(Message::MidiInDisconnect)
            .into()
    } else {
        Space::new().width(Length::Fixed(0.0)).into()
    };

    let preset_picker = pick_list(preset_items, selected_preset, Message::PresetSelected);
    let save_btn = button(text(" Save ").color(theme::TEXT))
        .style(theme::control_style(false))
        .on_press(Message::SavePressed);
    let load_btn = button(text(" Load ").color(theme::TEXT))
        .style(theme::control_style(false))
        .on_press(Message::LoadPressed);

    let controls_top = row![
        play_btn,
        stop_btn,
        Space::new().width(Length::Fixed(12.0)),
        text("Out:").color(theme::DIM),
        mode_list,
        backend_widget,
        Space::new().width(Length::Fill),
        text("Preset:").color(theme::DIM),
        preset_picker,
        save_btn,
        load_btn,
    ]
    .spacing(8)
    .align_y(iced::Alignment::Center);

    let controls_bottom = row![
        text("Clock In:").color(theme::DIM),
        clock_in_widget,
        clock_in_disconnect,
        Space::new().width(Length::Fill),
        text(if state.midi_in_backend.is_some() {
            "external clock active — internal timer paused"
        } else {
            "internal clock"
        })
        .size(12)
        .color(theme::DIM),
    ]
    .spacing(8)
    .align_y(iced::Alignment::Center);

    let current_step = state.transport.current_step as usize;
    let playing = state.transport.playing;

    // Mixer header
    let mixer_header = row![
        Space::new().width(Length::Fixed(50.0)),
        text("VOL").size(11).color(theme::DIM).width(Length::Fixed(60.0)),
        text("PAN").size(11).color(theme::DIM).width(Length::Fixed(60.0)),
    ]
    .spacing(4);

    let grid = column(
        Track::ALL
            .iter()
            .enumerate()
            .map(|(t, track)| {
                let label = container(text(track.label()).size(13).color(theme::TEXT))
                    .width(Length::Fixed(50.0))
                    .align_x(iced::alignment::Horizontal::Right)
                    .padding(iced::Padding {
                        top: 0.0,
                        right: 6.0,
                        bottom: 0.0,
                        left: 0.0,
                    });

                let vol_slider = slider(
                    0.0f32..=127.0,
                    state.track_volume[t] as f32,
                    move |v| Message::TrackVolumeChanged(t, v),
                )
                .step(1.0f32)
                .width(Length::Fixed(60.0));

                let pan_slider = slider(
                    -64.0f32..=63.0,
                    state.track_pan[t] as f32,
                    move |v| Message::TrackPanChanged(t, v),
                )
                .step(1.0f32)
                .width(Length::Fixed(60.0));

                let steps = row((0..NUM_STEPS)
                    .map(|s| {
                        let vel = state.pattern.steps[t][s];
                        let is_active = playing && s == current_step;
                        let is_beat = s % 4 == 0;

                        button(Space::new().width(Length::Fill).height(Length::Fill))
                            .width(Length::Fixed(54.0))
                            .height(Length::Fixed(36.0))
                            .style(theme::step_style(vel, is_active, is_beat))
                            .on_press(Message::StepToggled { track: t, step: s })
                            .into()
                    })
                    .collect::<Vec<_>>())
                .spacing(2);

                row![label, vol_slider, pan_slider, steps]
                    .spacing(4)
                    .align_y(iced::Alignment::Center)
                    .into()
            })
            .collect::<Vec<_>>(),
    )
    .spacing(4);

    let beat_no = if playing { (current_step / 4) + 1 } else { 0 };
    let step_label = if playing {
        format!("Step {:02}  Beat {}  ▶ PLAYING", current_step + 1, beat_no)
    } else {
        "■ STOPPED".to_string()
    };
    let status_row = row![
        text(step_label).size(13).color(if playing {
            theme::ACTIVE_STEP
        } else {
            theme::DIM
        }),
        Space::new().width(Length::Fill),
        text(&state.status_msg).size(13).color(theme::DIM),
    ]
    .spacing(8);

    let content = column![
        header,
        horizontal_rule(),
        controls_top,
        controls_bottom,
        horizontal_rule(),
        mixer_header,
        grid,
        horizontal_rule(),
        status_row,
    ]
    .spacing(8)
    .padding(16);

    container(content)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(|_| container::Style {
            background: Some(Background::Color(theme::BACKGROUND)),
            ..Default::default()
        })
        .into()
}

fn horizontal_rule() -> Element<'static, Message> {
    container(Space::new().width(Length::Fill).height(Length::Fixed(1.0)))
        .style(|_| container::Style {
            background: Some(Background::Color(theme::BORDER_COLOR)),
            ..Default::default()
        })
        .width(Length::Fill)
        .into()
}
