use anyhow::{anyhow, Result};

use crate::config::{MappedCcAction, Settings};

pub enum TranslatedMessage<'a> {
    Forward(&'a [u8]),
    Generated { bytes: [u8; 10], len: usize },
}

pub struct Translator {
    listen_channel_zero_based: u8,
    cc_actions: [Option<MappedCcAction>; 128],
}

impl Translator {
    pub fn new(settings: &Settings) -> Self {
        Self {
            listen_channel_zero_based: settings.listen_channel_zero_based(),
            cc_actions: settings.cc_actions(),
        }
    }

    pub fn translate<'a>(
        &self,
        incoming_message: &'a [u8],
    ) -> Result<Option<TranslatedMessage<'a>>> {
        if incoming_message.is_empty() {
            return Err(anyhow!("received empty MIDI message"));
        }

        let status = incoming_message[0];
        let message_type = status & 0xF0;
        let channel = status & 0x0F;

        if channel != self.listen_channel_zero_based {
            return Ok(None);
        }

        match message_type {
            0xC0 => {
                if incoming_message.len() < 2 {
                    return Err(anyhow!(
                        "program change message too short: {:?}",
                        incoming_message
                    ));
                }
                Ok(Some(TranslatedMessage::Forward(&incoming_message[..2])))
            }
            0xB0 => {
                if incoming_message.len() < 3 {
                    return Err(anyhow!(
                        "control change message too short: {:?}",
                        incoming_message
                    ));
                }
                let cc_num = incoming_message[1];
                let cc_value = incoming_message[2];
                let Some(action) = self.cc_actions[cc_num as usize] else {
                    return Ok(None);
                };
                Ok(Some(self.translate_cc_action(action, cc_value, channel)))
            }
            _ => Ok(None),
        }
    }

    pub fn build_startup_message() -> [u8; 6] {
        [0xF0, 0x52, 0x00, 0x58, 0x50, 0xF7]
    }

    fn translate_cc_action(
        &self,
        action: MappedCcAction,
        cc_value: u8,
        channel: u8,
    ) -> TranslatedMessage<'static> {
        match action {
            MappedCcAction::ChangeEffectParam {
                effect_position,
                param_num,
            } => self.create_change_effect_param_message(effect_position, param_num, cc_value),
            MappedCcAction::EffectOnOff { effect_position } => self
                .create_change_effect_param_message(
                    effect_position,
                    0x00,
                    if cc_value <= 64 { 0x00 } else { 0x01 },
                ),
            MappedCcAction::TunerOnOff => TranslatedMessage::Generated {
                bytes: [
                    0xB0 | channel,
                    0x4A,
                    cc_value,
                    0x00,
                    0x00,
                    0x00,
                    0x00,
                    0x00,
                    0x00,
                    0x00,
                ],
                len: 3,
            },
        }
    }

    fn create_change_effect_param_message(
        &self,
        effect_position: u8,
        param_num: u8,
        param_value: u8,
    ) -> TranslatedMessage<'static> {
        TranslatedMessage::Generated {
            bytes: [
                0xF0,
                0x52,
                0x00,
                0x58,
                0x31,
                effect_position,
                param_num,
                param_value,
                0x00,
                0xF7,
            ],
            len: 10,
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use crate::config::Settings;

    use super::*;

    fn settings_with_maps() -> Settings {
        let raw = json!({
            "targetInputDevices": ["HX Stomp"],
            "targetOutputDevice": ["ZOOM MS Series"],
            "targetOutputDeviceChannel": 3,
            "changeControlMaps": [
                { "ccNum": 0, "action": "tunerOnOff" },
                { "ccNum": 10, "action": "effectOnOff", "effectPosition": 1 },
                { "ccNum": 11, "action": "changeEffectParam", "effectPosition": 2, "paramNum": 7 }
            ]
        });
        Settings::from_json_str(&raw.to_string()).expect("validated settings")
    }

    #[test]
    fn forwards_program_change_on_listen_channel() {
        let settings = settings_with_maps();
        let translator = Translator::new(&settings);

        let translated = translator
            .translate(&[0xC2, 0x2A])
            .expect("translation should work")
            .expect("message should be forwarded");
        match translated {
            TranslatedMessage::Forward(bytes) => assert_eq!(bytes, &[0xC2, 0x2A]),
            _ => panic!("expected forwarded message"),
        }
    }

    #[test]
    fn ignores_program_change_on_other_channel() {
        let settings = settings_with_maps();
        let translator = Translator::new(&settings);

        let translated = translator
            .translate(&[0xC1, 0x2A])
            .expect("translation should work");
        assert!(translated.is_none());
    }

    #[test]
    fn maps_change_effect_param_cc_to_sysex() {
        let settings = settings_with_maps();
        let translator = Translator::new(&settings);

        let translated = translator
            .translate(&[0xB2, 11, 99])
            .expect("translation should work")
            .expect("message should be transformed");
        match translated {
            TranslatedMessage::Generated { bytes, len } => {
                assert_eq!(len, 10);
                assert_eq!(
                    &bytes[..len],
                    &[0xF0, 0x52, 0x00, 0x58, 0x31, 2, 7, 99, 0, 0xF7]
                );
            }
            _ => panic!("expected generated message"),
        }
    }

    #[test]
    fn maps_effect_on_off_threshold() {
        let settings = settings_with_maps();
        let translator = Translator::new(&settings);

        let off = translator
            .translate(&[0xB2, 10, 64])
            .expect("translation should work")
            .expect("message should be transformed");
        let on = translator
            .translate(&[0xB2, 10, 65])
            .expect("translation should work")
            .expect("message should be transformed");

        match off {
            TranslatedMessage::Generated { bytes, len } => {
                assert_eq!(
                    &bytes[..len],
                    &[0xF0, 0x52, 0x00, 0x58, 0x31, 1, 0, 0, 0, 0xF7]
                );
            }
            _ => panic!("expected generated message"),
        }
        match on {
            TranslatedMessage::Generated { bytes, len } => {
                assert_eq!(
                    &bytes[..len],
                    &[0xF0, 0x52, 0x00, 0x58, 0x31, 1, 0, 1, 0, 0xF7]
                );
            }
            _ => panic!("expected generated message"),
        }
    }

    #[test]
    fn maps_tuner_cc_to_channel_specific_cc74() {
        let settings = settings_with_maps();
        let translator = Translator::new(&settings);

        let translated = translator
            .translate(&[0xB2, 0, 127])
            .expect("translation should work")
            .expect("message should be transformed");
        match translated {
            TranslatedMessage::Generated { bytes, len } => {
                assert_eq!(len, 3);
                assert_eq!(&bytes[..len], &[0xB2, 0x4A, 127]);
            }
            _ => panic!("expected generated message"),
        }
    }
}
