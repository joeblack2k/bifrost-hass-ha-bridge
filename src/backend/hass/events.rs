use serde_json::Value;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum HassEventKind {
    StateChanged,
    InitialPress,
    ShortRelease,
    LongPress,
    LongRelease,
    DoubleShortRelease,
    Repeat,
    Unknown,
}

impl HassEventKind {
    pub(super) const fn name(self) -> Option<&'static str> {
        match self {
            Self::StateChanged | Self::Unknown => None,
            Self::InitialPress => Some("initial_press"),
            Self::ShortRelease => Some("short_release"),
            Self::LongPress => Some("long_press"),
            Self::LongRelease => Some("long_release"),
            Self::DoubleShortRelease => Some("double_short_release"),
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
    match value.as_str() {
        "initial_press" | "initial" => HassEventKind::InitialPress,
        "repeat" => HassEventKind::Repeat,
        "short_release" | "button_short_press" | "remote_button_short_press" => {
            HassEventKind::ShortRelease
        }
        "long_press" | "button_long_press" | "remote_button_long_press" => HassEventKind::LongPress,
        "long_release" => HassEventKind::LongRelease,
        "double_short_release" | "double_press" => HassEventKind::DoubleShortRelease,
        _ => HassEventKind::Unknown,
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

pub(super) fn normalize_event_value(value: &str) -> Option<String> {
    classify_value(value).name().map(ToOwned::to_owned)
}

pub(super) fn normalized_event_name(data: &Value) -> Option<String> {
    let raw = event_name(data)?;
    normalize_event_value(&raw)
}

pub(super) fn normalize_event_values(value: &Value) -> Option<Value> {
    let values = match value {
        Value::Array(values) => values.iter().filter_map(Value::as_str).collect::<Vec<_>>(),
        Value::String(value) => vec![value.as_str()],
        _ => return None,
    };

    let mut normalized_values = Vec::new();
    for value in values {
        let Some(normalized) = normalize_event_value(value) else {
            continue;
        };
        if !normalized_values.contains(&normalized) {
            normalized_values.push(normalized);
        }
    }
    (!normalized_values.is_empty())
        .then(|| Value::Array(normalized_values.into_iter().map(Value::String).collect()))
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
            HassEventKind::ShortRelease
        );
        assert_eq!(
            classify("deconz_event", &json!({"event_type": "double_press"})),
            HassEventKind::DoubleShortRelease
        );
        assert_eq!(
            classify("mqtt_event", &json!({"action": "long_release"})),
            HassEventKind::LongRelease
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
        assert_eq!(
            normalized_event_name(&data).as_deref(),
            Some("short_release")
        );
        assert_eq!(
            normalize_event_values(data.get("event_types").expect("event types")),
            Some(json!([
                "initial_press",
                "repeat",
                "short_release",
                "long_press",
                "long_release"
            ]))
        );
    }

    #[test]
    fn drops_unknown_event_values_instead_of_inventing_taxonomy() {
        let data = json!({
            "event_type": "unknown_event",
            "event_types": ["short_release", "swipe", "long_release"]
        });
        assert_eq!(normalized_event_name(&data), None);
        assert_eq!(
            normalize_event_values(data.get("event_types").expect("event types")),
            Some(json!(["short_release", "long_release"]))
        );
    }
}
