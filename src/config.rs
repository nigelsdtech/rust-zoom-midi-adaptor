use std::{fs, path::Path};

use anyhow::{anyhow, bail, Context, Result};
use serde::Deserialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MappedCcAction {
    ChangeEffectParam { effect_position: u8, param_num: u8 },
    EffectOnOff { effect_position: u8 },
    TunerOnOff,
}

#[derive(Debug)]
pub struct Settings {
    input_device_prefixes: Vec<String>,
    output_device_prefixes: Vec<String>,
    listen_channel_zero_based: u8,
    cc_actions: [Option<MappedCcAction>; 128],
}

impl Settings {
    pub fn from_path(path: &Path) -> Result<Self> {
        let contents = fs::read_to_string(path)
            .with_context(|| format!("failed to read config file at {}", path.display()))?;
        Self::from_json_str(&contents)
            .with_context(|| format!("failed to parse/validate config at {}", path.display()))
    }

    pub(crate) fn from_json_str(contents: &str) -> Result<Self> {
        let raw: RawConfig = serde_json::from_str(contents)?;
        raw.validate()
    }

    pub fn input_device_prefixes(&self) -> &[String] {
        &self.input_device_prefixes
    }

    pub fn output_device_prefixes(&self) -> &[String] {
        &self.output_device_prefixes
    }

    pub fn listen_channel_zero_based(&self) -> u8 {
        self.listen_channel_zero_based
    }

    pub fn cc_actions(&self) -> [Option<MappedCcAction>; 128] {
        self.cc_actions
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RawConfig {
    target_input_devices: Vec<String>,
    target_output_device: RawDeviceNames,
    target_output_device_channel: u8,
    #[serde(default)]
    change_control_maps: Vec<RawChangeControlMap>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum RawDeviceNames {
    Single(String),
    Multiple(Vec<String>),
}

impl RawDeviceNames {
    fn into_vec(self) -> Vec<String> {
        match self {
            Self::Single(name) => vec![name],
            Self::Multiple(names) => names,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawChangeControlMap {
    cc_num: u8,
    action: RawAction,
    effect_position: Option<u8>,
    param_num: Option<u8>,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
enum RawAction {
    ChangeEffectParam,
    EffectOnOff,
    TunerOnOff,
}

impl RawConfig {
    pub(crate) fn validate(self) -> Result<Settings> {
        if !(1..=16).contains(&self.target_output_device_channel) {
            bail!(
                "targetOutputDeviceChannel must be between 1 and 16, got {}",
                self.target_output_device_channel
            );
        }

        let input_device_prefixes =
            validate_device_prefixes(self.target_input_devices, "targetInputDevices")?;
        let output_device_prefixes =
            validate_device_prefixes(self.target_output_device.into_vec(), "targetOutputDevice")?;

        let mut cc_actions = [None; 128];
        for map in self.change_control_maps {
            let mapped_action = match map.action {
                RawAction::ChangeEffectParam => {
                    let effect_position = map.effect_position.ok_or_else(|| {
                        anyhow!(
                            "changeEffectParam mapping for ccNum {} must include effectPosition",
                            map.cc_num
                        )
                    })?;
                    if effect_position > 5 {
                        bail!(
                            "effectPosition must be between 0 and 5 for ccNum {}, got {}",
                            map.cc_num,
                            effect_position
                        );
                    }
                    let param_num = map.param_num.ok_or_else(|| {
                        anyhow!(
                            "changeEffectParam mapping for ccNum {} must include paramNum",
                            map.cc_num
                        )
                    })?;
                    MappedCcAction::ChangeEffectParam {
                        effect_position,
                        param_num,
                    }
                }
                RawAction::EffectOnOff => {
                    let effect_position = map.effect_position.ok_or_else(|| {
                        anyhow!(
                            "effectOnOff mapping for ccNum {} must include effectPosition",
                            map.cc_num
                        )
                    })?;
                    if effect_position > 5 {
                        bail!(
                            "effectPosition must be between 0 and 5 for ccNum {}, got {}",
                            map.cc_num,
                            effect_position
                        );
                    }
                    MappedCcAction::EffectOnOff { effect_position }
                }
                RawAction::TunerOnOff => MappedCcAction::TunerOnOff,
            };

            let slot = &mut cc_actions[map.cc_num as usize];
            if slot.is_some() {
                bail!("duplicate changeControlMaps entry for ccNum {}", map.cc_num);
            }
            *slot = Some(mapped_action);
        }

        Ok(Settings {
            input_device_prefixes,
            output_device_prefixes,
            listen_channel_zero_based: self.target_output_device_channel - 1,
            cc_actions,
        })
    }
}

fn validate_device_prefixes(mut names: Vec<String>, key_name: &str) -> Result<Vec<String>> {
    if names.is_empty() {
        bail!("{key_name} must contain at least one MIDI device prefix");
    }
    names.retain(|name| !name.trim().is_empty());
    if names.is_empty() {
        bail!("{key_name} cannot be empty");
    }
    Ok(names)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_pedalboard_style_config() {
        let raw = r#"
        {
          "targetInputDevices": ["HX Stomp"],
          "targetOutputDevice": ["ZOOM MS Series"],
          "targetOutputDeviceChannel": 3,
          "changeControlMaps": [
            { "ccNum": 0, "action": "tunerOnOff" },
            { "ccNum": 10, "action": "effectOnOff", "effectPosition": 1 },
            { "ccNum": 11, "action": "changeEffectParam", "effectPosition": 1, "paramNum": 2 }
          ]
        }"#;

        let raw_config: RawConfig = serde_json::from_str(raw).expect("valid raw config");
        let settings = raw_config.validate().expect("validated settings");
        let actions = settings.cc_actions();

        assert_eq!(settings.listen_channel_zero_based(), 2);
        assert!(matches!(actions[0], Some(MappedCcAction::TunerOnOff)));
        assert!(matches!(
            actions[10],
            Some(MappedCcAction::EffectOnOff { effect_position: 1 })
        ));
        assert!(matches!(
            actions[11],
            Some(MappedCcAction::ChangeEffectParam {
                effect_position: 1,
                param_num: 2
            })
        ));
    }

    #[test]
    fn rejects_duplicate_cc_mappings() {
        let raw = r#"
        {
          "targetInputDevices": ["HX Stomp"],
          "targetOutputDevice": "ZOOM",
          "targetOutputDeviceChannel": 1,
          "changeControlMaps": [
            { "ccNum": 10, "action": "tunerOnOff" },
            { "ccNum": 10, "action": "tunerOnOff" }
          ]
        }"#;
        let raw_config: RawConfig = serde_json::from_str(raw).expect("valid raw config");
        let err = raw_config.validate().expect_err("duplicate should fail");
        assert!(err.to_string().contains("duplicate"));
    }

    #[test]
    fn rejects_missing_effect_position() {
        let raw = r#"
        {
          "targetInputDevices": ["HX Stomp"],
          "targetOutputDevice": "ZOOM",
          "targetOutputDeviceChannel": 1,
          "changeControlMaps": [
            { "ccNum": 11, "action": "changeEffectParam", "paramNum": 2 }
          ]
        }"#;
        let raw_config: RawConfig = serde_json::from_str(raw).expect("valid raw config");
        let err = raw_config
            .validate()
            .expect_err("missing field should fail");
        assert!(err.to_string().contains("effectPosition"));
    }
}
