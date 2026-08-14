use chrono::Utc;
use serde_json::{Value, json};

use crate::backend::hass::HassServiceKind;
use crate::backend::hass::client::HassState;

/// Return the small, explicitly supported subset of HA numeric sensors.
/// Unknown numeric sensors stay out of the Hue model until their protocol mapping is known.
pub(super) fn classify_sensor(state: &HassState) -> Option<HassServiceKind> {
    let (domain, _) = state.entity_id.split_once('.')?;
    if domain != "sensor" {
        return None;
    }

    let device_class = state
        .attributes
        .get("device_class")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_ascii_lowercase();
    let unit = state
        .attributes
        .get("unit_of_measurement")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase();

    match device_class.as_str() {
        "temperature" => Some(HassServiceKind::Temperature),
        "illuminance" | "light_level" => Some(HassServiceKind::LightLevel),
        _ if matches!(unit.as_str(), "°c" | "°f" | "c" | "f") => {
            Some(HassServiceKind::Temperature)
        }
        _ if unit == "lx" => Some(HassServiceKind::LightLevel),
        _ => None,
    }
}

pub(super) fn numeric_value(state: &HassState) -> Option<f64> {
    if matches!(state.state.as_str(), "unknown" | "unavailable") {
        return None;
    }

    let value = state.state.parse::<f64>().ok()?;
    value.is_finite().then_some(value)
}

pub(super) fn resource_payload(
    kind: HassServiceKind,
    value: Option<f64>,
    available: bool,
) -> Value {
    let valid = available && value.is_some();
    let last_updated = Utc::now().to_rfc3339();

    match kind {
        HassServiceKind::Temperature => json!({
            "temperature": value,
            "temperature_valid": valid,
            "last_updated": last_updated,
        }),
        HassServiceKind::LightLevel => json!({
            "light_level": value.map(|x| x.max(0.0).round()),
            "light_level_valid": valid,
            "last_updated": last_updated,
        }),
        _ => Value::Null,
    }
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use super::{classify_sensor, numeric_value, resource_payload};
    use crate::backend::hass::HassServiceKind;
    use crate::backend::hass::client::HassState;

    fn state(entity_id: &str, value: &str, attrs: Value) -> HassState {
        HassState {
            entity_id: entity_id.to_string(),
            state: value.to_string(),
            attributes: attrs.as_object().cloned().unwrap_or_default(),
        }
    }

    #[test]
    fn classifies_temperature_and_light_level() {
        let temperature = state(
            "sensor.room_temperature",
            "21.5",
            json!({"device_class": "temperature"}),
        );
        let light_level = state(
            "sensor.room_lux",
            "400",
            json!({"unit_of_measurement": "lx"}),
        );

        assert_eq!(
            classify_sensor(&temperature),
            Some(HassServiceKind::Temperature)
        );
        assert_eq!(
            classify_sensor(&light_level),
            Some(HassServiceKind::LightLevel)
        );
        assert_eq!(numeric_value(&temperature), Some(21.5));
    }

    #[test]
    fn ignores_unknown_or_invalid_numeric_values() {
        let unknown = state("sensor.power", "3", json!({"unit_of_measurement": "W"}));
        let unavailable = state(
            "sensor.room_temperature",
            "unavailable",
            json!({"device_class": "temperature"}),
        );
        let invalid = state(
            "sensor.room_temperature",
            "not-a-number",
            json!({"device_class": "temperature"}),
        );

        assert_eq!(classify_sensor(&unknown), None);
        assert_eq!(numeric_value(&unavailable), None);
        assert_eq!(numeric_value(&invalid), None);
    }

    #[test]
    fn builds_valid_and_invalid_hue_payloads() {
        let valid = resource_payload(HassServiceKind::Temperature, Some(20.25), true);
        let invalid = resource_payload(HassServiceKind::LightLevel, None, false);

        assert_eq!(valid["temperature"], 20.25);
        assert_eq!(valid["temperature_valid"], true);
        assert_eq!(invalid["light_level"], Value::Null);
        assert_eq!(invalid["light_level_valid"], false);
    }
}
