use serde_json::{Map, Value};

use hue::api::{LightEffect, LightEffectsUpdate, LightEffectsV2Update, LightGradient, LightUpdate};

use crate::backend::hass::HassLightCapabilities;

fn normalized(value: &str) -> String {
    value
        .trim()
        .to_ascii_lowercase()
        .replace(['-', ' ', '_'], "")
}

pub(super) fn parse_effect(value: &str) -> Option<LightEffect> {
    match normalized(value).as_str() {
        "none" | "noeffect" | "off" => Some(LightEffect::NoEffect),
        "prism" => Some(LightEffect::Prism),
        "opal" => Some(LightEffect::Opal),
        "glisten" => Some(LightEffect::Glisten),
        "sparkle" => Some(LightEffect::Sparkle),
        "fire" => Some(LightEffect::Fire),
        "candle" => Some(LightEffect::Candle),
        "underwater" => Some(LightEffect::Underwater),
        "cosmos" => Some(LightEffect::Cosmos),
        "sunbeam" => Some(LightEffect::Sunbeam),
        "enchant" => Some(LightEffect::Enchant),
        _ => None,
    }
}

pub(super) fn supported_effects(attributes: &Map<String, Value>) -> Vec<LightEffect> {
    let mut effects = Vec::new();
    let Some(values) = attributes.get("effect_list").and_then(Value::as_array) else {
        return effects;
    };
    for effect in values
        .iter()
        .filter_map(Value::as_str)
        .filter_map(parse_effect)
    {
        if !effects.contains(&effect) {
            effects.push(effect);
        }
    }
    effects
}

pub(super) fn gradient_from_attributes(attributes: &Map<String, Value>) -> Option<LightGradient> {
    attributes
        .get("gradient")
        .cloned()
        .and_then(|value| serde_json::from_value(value).ok())
}

pub(super) fn supports_gradient(attributes: &Map<String, Value>) -> bool {
    attributes.get("gradient").is_some_and(Value::is_object)
        || attributes
            .get("gradient_points_capable")
            .and_then(Value::as_u64)
            .is_some_and(|points| points > 0)
}

fn effect_name(effect: LightEffect) -> &'static str {
    match effect {
        LightEffect::NoEffect => "none",
        LightEffect::Prism => "prism",
        LightEffect::Opal => "opal",
        LightEffect::Glisten => "glisten",
        LightEffect::Sparkle => "sparkle",
        LightEffect::Fire => "fire",
        LightEffect::Candle => "candle",
        LightEffect::Underwater => "underwater",
        LightEffect::Cosmos => "cosmos",
        LightEffect::Sunbeam => "sunbeam",
        LightEffect::Enchant => "enchant",
    }
}

fn effect_from_v2(update: &LightEffectsV2Update) -> Option<LightEffect> {
    update
        .action
        .as_ref()
        .and_then(|action| action.effect)
        .or_else(|| {
            update.status.as_ref().and_then(|status| {
                status.as_str().and_then(parse_effect).or_else(|| {
                    status
                        .get("effect")
                        .and_then(Value::as_str)
                        .and_then(parse_effect)
                })
            })
        })
}

fn effect_from_v1(update: &LightEffectsUpdate) -> Option<LightEffect> {
    update
        .action
        .as_ref()
        .and_then(|action| action.effect)
        .or(update.status)
}

pub(super) fn append_update_data(
    data: &mut Map<String, Value>,
    update: &LightUpdate,
    capabilities: &HassLightCapabilities,
) {
    if update.identify.is_some() {
        data.insert("flash".to_string(), Value::String("short".to_string()));
    }

    if capabilities.supports_effects {
        if let Some(effect) = update.effects.as_ref().and_then(effect_from_v1) {
            if capabilities.effect_values.contains(&effect) {
                data.insert(
                    "effect".to_string(),
                    Value::String(effect_name(effect).to_string()),
                );
            }
        }
        if let Some(effect) = update.effects_v2.as_ref().and_then(effect_from_v2) {
            if capabilities.effect_values.contains(&effect) {
                data.insert(
                    "effect".to_string(),
                    Value::String(effect_name(effect).to_string()),
                );
            }
        }
    }

    if capabilities.supports_gradient {
        if let Some(gradient) = &update.gradient {
            if let Ok(value) = serde_json::to_value(gradient) {
                data.insert("gradient".to_string(), value);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{append_update_data, parse_effect, supported_effects};
    use crate::backend::hass::HassLightCapabilities;
    use hue::api::{DeviceIdentify, DeviceIdentifyUpdate, LightEffect, LightEffectsUpdate};
    use serde_json::{Map, Value, json};

    #[test]
    fn parses_hue_effect_names_without_case_or_separator_fragility() {
        assert_eq!(parse_effect("No Effect"), Some(LightEffect::NoEffect));
        assert_eq!(parse_effect("under-water"), Some(LightEffect::Underwater));
        assert!(parse_effect("rainbow").is_none());
    }

    #[test]
    fn maps_identify_and_supported_effects_to_safe_ha_data() {
        let mut attributes = Map::new();
        attributes.insert("effect_list".to_string(), json!(["prism", "none"]));
        assert_eq!(
            supported_effects(&attributes),
            vec![LightEffect::Prism, LightEffect::NoEffect]
        );

        let mut data = Map::new();
        let update = hue::api::LightUpdate {
            identify: Some(DeviceIdentifyUpdate {
                action: DeviceIdentify::Identify,
            }),
            effects: Some(LightEffectsUpdate {
                action: None,
                status: Some(LightEffect::Prism),
            }),
            ..Default::default()
        };
        append_update_data(
            &mut data,
            &update,
            &HassLightCapabilities {
                supports_effects: true,
                effect_values: vec![LightEffect::Prism, LightEffect::NoEffect],
                ..Default::default()
            },
        );
        assert_eq!(data.get("flash"), Some(&Value::String("short".to_string())));
        assert_eq!(
            data.get("effect"),
            Some(&Value::String("prism".to_string()))
        );
    }
}
