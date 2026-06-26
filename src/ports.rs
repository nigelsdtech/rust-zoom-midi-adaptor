use anyhow::{bail, Result};

pub fn select_port_index(
    available_port_names: &[String],
    preferred_prefixes: &[String],
    port_type: &str,
) -> Result<usize> {
    if available_port_names.is_empty() {
        bail!("no MIDI {port_type} ports available");
    }

    for prefix in preferred_prefixes {
        for (index, port_name) in available_port_names.iter().enumerate() {
            if port_name.starts_with(prefix) {
                return Ok(index);
            }
        }
    }

    bail!(
        "could not find configured MIDI {port_type} device. configured prefixes: {:?}",
        preferred_prefixes
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chooses_first_prefix_match_in_priority_order() {
        let available = vec![
            "Other Device".to_string(),
            "SINCO Foot Controller".to_string(),
            "HX Stomp MIDI".to_string(),
        ];
        let preferred = vec!["HX Stomp".to_string(), "SINCO".to_string()];

        let index = select_port_index(&available, &preferred, "input").expect("should find match");
        assert_eq!(index, 2);
    }

    #[test]
    fn errors_when_no_ports_exist() {
        let err = select_port_index(&[], &["HX".to_string()], "output")
            .expect_err("should fail for empty ports");
        assert!(err.to_string().contains("no MIDI output ports available"));
    }
}
