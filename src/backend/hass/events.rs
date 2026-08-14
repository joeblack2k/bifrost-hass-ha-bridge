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

fn normalized(value: &str) -> String {
    value
        .trim()
        .to_ascii_lowercase()
        .replace(['-', ' ', '.'], "_")
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
    let value = candidates
        .iter()
        .filter_map(|key| data.get(*key).and_then(Value::as_str))
        .map(normalized)
        .find(|value| !value.is_empty())
        .unwrap_or_default();

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

pub(super) fn entity_id(data: &Value) -> Option<&str> {
    data.get("entity_id").and_then(Value::as_str).or_else(|| {
        data.get("new_state")
            .and_then(|state| state.get("entity_id"))
            .and_then(Value::as_str)
    })
}

#[cfg(test)]
mod tests {
    use super::{HassEventKind, classify, entity_id, event_name};
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
}
