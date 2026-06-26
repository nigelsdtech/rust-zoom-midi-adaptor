use std::{env, fmt::Write as _, path::PathBuf, thread, time::Duration};

use anyhow::{anyhow, Context, Result};
use log::{debug, error, info};
use midir::{Ignore, MidiInput, MidiInputPort, MidiOutput, MidiOutputConnection, MidiOutputPort};
use zoom_midi_adaptor::config::Settings;
use zoom_midi_adaptor::ports::select_port_index;
use zoom_midi_adaptor::translator::{TranslatedMessage, Translator};

const DEFAULT_CONFIG_PATH: &str = "config/pedalboard.json";

fn main() -> Result<()> {
    init_logger();

    let config_path = env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(DEFAULT_CONFIG_PATH));
    let settings = Settings::from_path(&config_path)?;

    info!("loaded config from {}", config_path.display());

    let mut midi_input =
        MidiInput::new("zoom-midi-adaptor-input").context("failed to create MIDI input client")?;
    midi_input.ignore(Ignore::None);
    let midi_output = MidiOutput::new("zoom-midi-adaptor-output")
        .context("failed to create MIDI output client")?;

    let input_port = select_input_port(&midi_input, settings.input_device_prefixes())?;
    let output_port = select_output_port(&midi_output, settings.output_device_prefixes())?;

    let input_port_name = midi_input
        .port_name(&input_port)
        .context("failed to read MIDI input port name")?;
    let output_port_name = midi_output
        .port_name(&output_port)
        .context("failed to read MIDI output port name")?;

    info!("listening to input: {input_port_name}");
    info!("sending to output: {output_port_name}");

    let output_connection = midi_output
        .connect(&output_port, "zoom-midi-adaptor-output-connection")
        .map_err(|err| anyhow!("failed to open MIDI output port: {err}"))?;

    let mut callback_state = CallbackState::new(settings, output_connection);
    callback_state.send_startup_message()?;

    let _input_connection = midi_input
        .connect(
            &input_port,
            "zoom-midi-adaptor-input-connection",
            |_timestamp, message, state| {
                state.handle_message(message);
            },
            callback_state,
        )
        .map_err(|err| anyhow!("failed to open MIDI input port: {err}"))?;

    info!("zoom-midi-adaptor is running");

    loop {
        thread::sleep(Duration::from_secs(1));
    }
}

fn init_logger() {
    let mut builder =
        env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"));
    builder.format_timestamp_millis();
    builder.init();
}

struct CallbackState {
    translator: Translator,
    output_connection: MidiOutputConnection,
}

impl CallbackState {
    fn new(settings: Settings, output_connection: MidiOutputConnection) -> Self {
        Self {
            translator: Translator::new(&settings),
            output_connection,
        }
    }

    fn send_startup_message(&mut self) -> Result<()> {
        let startup = Translator::build_startup_message();
        self.output_connection
            .send(&startup)
            .context("failed to send startup edit-mode message")
    }

    fn handle_message(&mut self, message: &[u8]) {
        let translated = match self.translator.translate(message) {
            Ok(value) => value,
            Err(err) => {
                error!("dropping malformed MIDI message: {err}");
                return;
            }
        };

        let Some(translated) = translated else {
            debug!("ignoring non-matching MIDI message: {:?}", message);
            return;
        };

        match translated {
            TranslatedMessage::Forward(bytes) => self.log_and_send(message, bytes),
            TranslatedMessage::Generated { bytes, len } => {
                self.log_and_send(message, &bytes[..len])
            }
        }
    }

    fn log_and_send(&mut self, input: &[u8], output: &[u8]) {
        if log::log_enabled!(log::Level::Info) {
            info!(
                "midi in={} hex_in=[{}] hex_out=[{}]",
                format_input_message(input),
                format_midi_bytes(input),
                format_midi_bytes(output)
            );
        }

        if let Err(err) = self.output_connection.send(output) {
            error!("failed to send MIDI output message: {err}");
        }
    }
}

fn format_input_message(bytes: &[u8]) -> String {
    if bytes.is_empty() {
        return "UNKNOWN".to_string();
    }

    match bytes[0] & 0xF0 {
        0xC0 => {
            if bytes.len() < 2 {
                return "UNKNOWN".to_string();
            }
            format!("PC {}", bytes[1])
        }
        0xB0 => {
            if bytes.len() < 3 {
                return "UNKNOWN".to_string();
            }
            format!("CC {} {}", bytes[1], bytes[2])
        }
        _ => "UNKNOWN".to_string(),
    }
}

fn format_midi_bytes(bytes: &[u8]) -> String {
    let mut rendered = String::with_capacity(bytes.len().saturating_mul(3));
    for (index, byte) in bytes.iter().enumerate() {
        if index > 0 {
            rendered.push(' ');
        }
        let _ = write!(&mut rendered, "{byte:02X}");
    }
    rendered
}

fn select_input_port(
    midi_input: &MidiInput,
    preferred_prefixes: &[String],
) -> Result<MidiInputPort> {
    let ports = midi_input.ports();
    let mut port_names = Vec::with_capacity(ports.len());

    for (index, port) in ports.iter().enumerate() {
        let port_name = midi_input
            .port_name(port)
            .with_context(|| format!("failed to read MIDI input port name for index {index}"))?;
        info!("input port {index}: {port_name}");
        port_names.push(port_name);
    }

    let selected_index = select_port_index(&port_names, preferred_prefixes, "input")?;
    Ok(ports[selected_index].clone())
}

fn select_output_port(
    midi_output: &MidiOutput,
    preferred_prefixes: &[String],
) -> Result<MidiOutputPort> {
    let ports = midi_output.ports();
    let mut port_names = Vec::with_capacity(ports.len());

    for (index, port) in ports.iter().enumerate() {
        let port_name = midi_output
            .port_name(port)
            .with_context(|| format!("failed to read MIDI output port name for index {index}"))?;
        info!("output port {index}: {port_name}");
        port_names.push(port_name);
    }

    let selected_index = select_port_index(&port_names, preferred_prefixes, "output")?;
    Ok(ports[selected_index].clone())
}
