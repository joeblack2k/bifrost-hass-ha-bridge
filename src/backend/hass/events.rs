use serde_json::Value;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum HassEventKind {
    StateChanged,
    InitialPress,
    Press,
    DoublePress,
    TriplePress,
    Hold,
    Release,
    Repeat,
    Unknown,
}

impl HassEventKind {
    pub(super) const fn name(self) -> Option<&'static str> {
        match self {
            Self::StateChanged | Self::Unknown => None,
            Self::InitialPress => Some("initial_press"),
            Self::Press => Some("press"),
            Self::DoublePress => Some("double_press"),
            Self::TriplePress => Some("triple_press"),
            Self::Hold => Some("hold"),
            Self::Release => Some("release"),
            Self::Repeat => Some("repeat"),
        }
    }
}

fn normalized(value: &str) -> String {
    value
        .trim()
        .to_ascii_lowercase()
        .replace(['-', ' ', '.'], "_")
}

fn classify_value(value: &str) -> HassEventKind {
    let value = normalized(value);
    if value.contains("double") {
        HassEventKind::DoublePress
    } else if value.contains("triple") {
        HassEventKind::TriplePress
    } else if value.contains("initial_press") || value == "initial" {
        HassEventKind::InitialPress
    } else if value.contains("repeat") {
        HassEventKind::Repeat
    } else if value.contains("release") || value.contains("released") {
        HassEventKind::Release
    } else if value.contains("hold") || value.contains("long") {
        HassEventKind::Hold
    } else if value.contains("press") || value.contains("click") || value == "single" {
        HassEventKind::Press
    } else {
        HassEventKind::Unknown
    }
}

pub(super) fn classify(event_type: &str, data: &Value) -> HassEventKind {
    if event_type == "state_changed" {
        return HassEventKind::StateChanged;
    }

    let candidates = [
        "event_type",
        "command",
        "action",
        "subtype",
        "type",
        "event",
    ];
    candidates
        .iter()
        .filter_map(|key| data.get(*key).and_then(Value::as_str))
        .map(str::trim)
        .find(|value| !value.is_empty())
        .map_or(HassEventKind::Unknown, classify_value)
}

pub(super) fn event_name(data: &Value) -> Option<String> {
    [
        "event_type",
        "command",
        "action",
        "subtype",
        "type",
        "event",
    ]
    .iter()
    .filter_map(|key| data.get(*key).and_then(Value::as_str))
    .map(str::trim)
    .find(|value| !value.is_empty())
    .map(ToOwned::to_owned)
}

pub(super) fn normalize_event_value(value: &str) -> String {
    let kind = classify_value(value);
    let normalized = normalized(value);
    kind.name().unwrap_or(&normalized).to_string()
}

pub(super) fn normalized_event_name(data: &Value) -> Option<String> {
    let raw = event_name(data)?;
    Some(normalize_event_value(&raw))
}

pub(super) fn normalize_event_values(value: &Value) -> Option<Value> {
    let values = match value {
        Value::Array(values) => values.iter().filter_map(Value::as_str).collect::<Vec<_>>(),
        Value::String(value) => vec![value.as_str()],
        _ => return None,
    };

    let mut normalized_values = Vec::new();
    for value in values {
        let normalized = normalize_event_value(value);
        if !normalized.is_empty() && !normalized_values.contains(&normalized) {
            normalized_values.push(normalized);
        }
    }
    Some(Value::Array(
        normalized_values.into_iter().map(Value::String).collect(),
    ))
}

pub(super) fn entity_id(data: &Value) -> Option<&str> {
    data.get("entity_id").and_then(Value::as_str).or_else(|| {
        data.get("new_state")
            .and_then(|state| state.get("entity_id"))
            .and_then(Value::as_str)
    })
}

#[cfg(test)]
mod tests {
    use super::{
        HassEventKind, classify, entity_id, event_name, normalize_event_values,
        normalized_event_name,
    };
    use serde_json::json;

    #[test]
    fn classifies_common_accessory_events() {
        assert_eq!(
            classify("zha_event", &json!({"command": "button_short_press"})),
            HassEventKind::Press
        );
        assert_eq!(
            classify("deconz_event", &json!({"event_type": "double_press"})),
            HassEventKind::DoublePress
        );
        assert_eq!(
            classify("mqtt_event", &json!({"action": "long_release"})),
            HassEventKind::Release
        );
    }

    #[test]
    fn safely_ignores_unknown_event_payloads() {
        let data = json!({"new_state": {"entity_id": "event.remote"}});
        assert_eq!(classify("custom_event", &data), HassEventKind::Unknown);
        assert_eq!(entity_id(&data), Some("event.remote"));
        assert_eq!(event_name(&data), None);
    }

    #[test]
    fn normalizes_home_assistant_event_entity_values() {
        let data = json!({
            "event_type": "short_release",
            "event_types": ["initial_press", "repeat", "short_release", "long_press", "long_release"]
        });
        assert_eq!(normalized_event_name(&data).as_deref(), Some("release"));
        assert_eq!(
            normalize_event_values(data.get("event_types").expect("event types")),
            Some(json!(["initial_press", "repeat", "release", "hold"]))
        );
    }
}
