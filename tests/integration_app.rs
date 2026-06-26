use std::io::Write;

use tempfile::NamedTempFile;
use zoom_midi_adaptor::{
    config::Settings,
    ports::select_port_index,
    translator::{TranslatedMessage, Translator},
};

fn write_temp_config(contents: &str) -> NamedTempFile {
    let mut file = NamedTempFile::new().expect("should create temporary config file");
    file.write_all(contents.as_bytes())
        .expect("should write temporary config file");
    file.flush().expect("should flush temporary config file");
    file
}

#[test]
fn loads_config_file_and_selects_expected_ports() {
    let config_json = r#"
    {
      "targetInputDevices": ["HX Stomp", "SINCO"],
      "targetOutputDevice": ["ZOOM MS Series"],
      "targetOutputDeviceChannel": 3,
      "changeControlMaps": [
        { "ccNum": 11, "action": "changeEffectParam", "effectPosition": 0, "paramNum": 2 }
      ]
    }
    "#;

    let path = write_temp_config(config_json);
    let settings = Settings::from_path(path.path()).expect("settings should load from file");

    let input_ports = vec![
        "USB MIDI Keyboard".to_string(),
        "HX Stomp MIDI 1".to_string(),
        "SINCO Controller".to_string(),
    ];
    let output_ports = vec![
        "Scarlett MIDI".to_string(),
        "ZOOM MS Series Port 1".to_string(),
    ];

    let input_index = select_port_index(&input_ports, settings.input_device_prefixes(), "input")
        .expect("input port should be selected");
    let output_index =
        select_port_index(&output_ports, settings.output_device_prefixes(), "output")
            .expect("output port should be selected");

    assert_eq!(input_index, 1);
    assert_eq!(output_index, 1);
}

#[test]
fn processes_messages_end_to_end_from_config_to_translator_output() {
    let config_json = r#"
    {
      "targetInputDevices": ["HX Stomp"],
      "targetOutputDevice": ["ZOOM MS Series"],
      "targetOutputDeviceChannel": 3,
      "changeControlMaps": [
        { "ccNum": 10, "action": "effectOnOff", "effectPosition": 1 },
        { "ccNum": 11, "action": "changeEffectParam", "effectPosition": 0, "paramNum": 2 }
      ]
    }
    "#;

    let path = write_temp_config(config_json);
    let settings = Settings::from_path(path.path()).expect("settings should load from file");
    let translator = Translator::new(&settings);

    let startup = Translator::build_startup_message();
    assert_eq!(startup, [0xF0, 0x52, 0x00, 0x58, 0x50, 0xF7]);

    let forwarded_pc = translator
        .translate(&[0xC2, 0x05])
        .expect("translation should succeed")
        .expect("pc on listen channel should pass through");
    match forwarded_pc {
        TranslatedMessage::Forward(bytes) => assert_eq!(bytes, &[0xC2, 0x05]),
        _ => panic!("expected forwarded program change"),
    }

    let mapped_cc = translator
        .translate(&[0xB2, 11, 99])
        .expect("translation should succeed")
        .expect("mapped cc should produce sysex");
    match mapped_cc {
        TranslatedMessage::Generated { bytes, len } => {
            assert_eq!(
                &bytes[..len],
                &[0xF0, 0x52, 0x00, 0x58, 0x31, 0, 2, 99, 0, 0xF7]
            )
        }
        _ => panic!("expected generated sysex message"),
    }

    let unmapped_cc = translator
        .translate(&[0xB2, 99, 100])
        .expect("translation should succeed");
    assert!(unmapped_cc.is_none());

    let wrong_channel_pc = translator
        .translate(&[0xC1, 0x05])
        .expect("translation should succeed");
    assert!(wrong_channel_pc.is_none());
}

#[test]
fn fails_when_port_prefixes_do_not_match() {
    let available = vec!["Controller A".to_string(), "Controller B".to_string()];
    let preferred = vec!["HX Stomp".to_string(), "SINCO".to_string()];

    let err = select_port_index(&available, &preferred, "input")
        .expect_err("selection should fail without matching prefix");

    assert!(err
        .to_string()
        .contains("could not find configured MIDI input device"));
}

#[test]
fn fails_loading_invalid_config_file() {
    let invalid_config = r#"
    {
      "targetInputDevices": ["HX Stomp"],
      "targetOutputDevice": ["ZOOM MS Series"],
      "targetOutputDeviceChannel": 3,
      "changeControlMaps": [
        { "ccNum": 11, "action": "changeEffectParam", "paramNum": 2 }
      ]
    }
    "#;

    let path = write_temp_config(invalid_config);
    let err = Settings::from_path(path.path()).expect_err("invalid config should fail");
    assert!(format!("{err:#}").contains("effectPosition"));
}
