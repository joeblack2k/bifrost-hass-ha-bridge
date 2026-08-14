use serde_json::Value;

use hue::api::{
    ColorTemperatureUpdate, ColorUpdate, DimmingUpdate, On, ResourceLink, Scene, SceneAction,
    SceneActionElement, SceneActive, SceneMetadata, SceneRecall, SceneStatus,
};
use hue::xy::XY;

use super::client::HassState;

#[derive(Clone, Debug)]
pub(super) struct ImportedScene {
    pub entity_id: String,
    pub name: String,
    pub state: String,
    pub available: bool,
    pub area_name: Option<String>,
    pub targets: Vec<String>,
}

fn target_ids(attributes: &serde_json::Map<String, Value>) -> Vec<String> {
    let mut targets = Vec::new();
    for key in ["entity_id", "entities"] {
        let Some(value) = attributes.get(key) else {
            continue;
        };
        match value {
            Value::String(entity_id) => targets.push(entity_id.clone()),
            Value::Array(values) => targets.extend(
                values
                    .iter()
                    .filter_map(Value::as_str)
                    .map(ToOwned::to_owned),
            ),
            Value::Object(values) => targets.extend(values.keys().cloned()),
            Value::Null | Value::Bool(_) | Value::Number(_) => {}
        }
    }
    targets.sort();
    targets.dedup();
    targets
}

pub(super) fn parse(state: &HassState, area_name: Option<String>) -> Option<ImportedScene> {
    let (domain, _) = state.entity_id.split_once('.')?;
    if domain != "scene" || state.entity_id.starts_with("scene.bifrost_") {
        return None;
    }

    let name = state
        .attributes
        .get("friendly_name")
        .and_then(Value::as_str)
        .unwrap_or(&state.entity_id)
        .to_string();

    Some(ImportedScene {
        entity_id: state.entity_id.clone(),
        name,
        state: state.state.clone(),
        available: state.state != "unavailable",
        area_name,
        targets: target_ids(&state.attributes),
    })
}

pub(super) fn link(backend_name: &str, entity_id: &str) -> ResourceLink {
    hue::api::RType::Scene.deterministic(format!("hass:{backend_name}:{entity_id}:scene"))
}

pub(super) fn is_imported(scene: &Scene) -> bool {
    imported_entity_id(scene).is_some()
}

pub(super) fn imported_entity_id(scene: &Scene) -> Option<&str> {
    scene
        .metadata
        .appdata
        .as_deref()
        .and_then(|appdata| appdata.strip_prefix("hass:scene:"))
        .filter(|entity_id| !entity_id.trim().is_empty())
}

fn value_to_f64(value: &Value) -> Option<f64> {
    value
        .as_f64()
        .or_else(|| {
            value
                .as_i64()
                .and_then(|x| x.to_string().parse::<f64>().ok())
        })
        .or_else(|| {
            value
                .as_u64()
                .and_then(|x| x.to_string().parse::<f64>().ok())
        })
}

fn xy_color(state: &HassState) -> Option<ColorUpdate> {
    let values = state.attributes.get("xy_color")?.as_array()?;
    let [x, y] = values.as_slice() else {
        return None;
    };
    Some(ColorUpdate {
        xy: XY {
            x: value_to_f64(x)?.clamp(0.0, 1.0),
            y: value_to_f64(y)?.clamp(0.0, 1.0),
        },
    })
}

fn color_temperature(state: &HassState) -> Option<ColorTemperatureUpdate> {
    state
        .attributes
        .get("color_temp")
        .and_then(Value::as_u64)
        .and_then(|value| u16::try_from(value).ok())
        .map(|mirek| ColorTemperatureUpdate::new(mirek.clamp(153, 500)))
}

pub(super) fn scene_action(target: ResourceLink, state: Option<&HassState>) -> SceneActionElement {
    let action = state.map_or(
        SceneAction {
            color: None,
            color_temperature: None,
            dimming: None,
            on: None,
            gradient: None,
            effects: Value::Null,
        },
        |state| SceneAction {
            color: xy_color(state),
            color_temperature: color_temperature(state),
            dimming: state
                .attributes
                .get("brightness")
                .and_then(value_to_f64)
                .map(|brightness| {
                    DimmingUpdate::new((brightness / 255.0 * 100.0).clamp(0.0, 100.0))
                }),
            on: matches!(state.state.as_str(), "on" | "off").then(|| On::new(state.state == "on")),
            gradient: None,
            effects: Value::Null,
        },
    );
    SceneActionElement { action, target }
}

pub(super) fn build(
    imported: &ImportedScene,
    group: ResourceLink,
    actions: Vec<SceneActionElement>,
) -> Scene {
    Scene {
        actions,
        auto_dynamic: false,
        group,
        metadata: SceneMetadata {
            appdata: Some(format!("hass:scene:{}", imported.entity_id)),
            image: None,
            name: imported.name.clone(),
        },
        palette: Value::Null,
        speed: 0.0,
        status: Some(SceneStatus {
            active: SceneActive::Inactive,
            last_recall: None,
        }),
        recall: SceneRecall::default(),
    }
}

#[cfg(test)]
mod tests {
    use super::{build, link, parse, scene_action};
    use crate::backend::hass::client::HassState;
    use hue::api::RType;
    use serde_json::json;

    #[test]
    fn parses_scene_targets_without_trusting_optional_shape() {
        let state = HassState {
            entity_id: "scene.movie".to_string(),
            state: "scening".to_string(),
            attributes: json!({
                "friendly_name": "Movie",
                "entity_id": ["light.tv", "light.tv", "switch.receiver"]
            })
            .as_object()
            .cloned()
            .expect("object"),
        };
        let scene = parse(&state, Some("Living room".to_string())).expect("scene");
        assert_eq!(scene.targets, ["light.tv", "switch.receiver"]);
        assert_eq!(scene.area_name.as_deref(), Some("Living room"));
        assert!(
            parse(
                &HassState {
                    entity_id: "scene.bifrost_owned".to_string(),
                    state: "scening".to_string(),
                    attributes: Default::default(),
                },
                None
            )
            .is_none()
        );
    }

    #[test]
    fn keeps_scene_recall_resource_stable() {
        let imported = parse(
            &HassState {
                entity_id: "scene.movie".to_string(),
                state: "scening".to_string(),
                attributes: json!({"friendly_name": "Movie"})
                    .as_object()
                    .cloned()
                    .expect("object"),
            },
            None,
        )
        .expect("scene");
        let group = RType::Room.link_to(uuid::Uuid::nil());
        let target = RType::Light.link_to(uuid::Uuid::new_v4());
        let scene = build(&imported, group, vec![scene_action(target, None)]);
        assert_eq!(link("ha", "scene.movie").rtype, RType::Scene);
        assert_eq!(scene.group, group);
        assert_eq!(scene.actions.len(), 1);
        assert_eq!(super::imported_entity_id(&scene), Some("scene.movie"));
    }

    #[test]
    fn unknown_scene_state_remains_recallable() {
        let scene = parse(
            &HassState {
                entity_id: "scene.sleep".to_string(),
                state: "unknown".to_string(),
                attributes: json!({"friendly_name": "Sleep"})
                    .as_object()
                    .cloned()
                    .expect("object"),
            },
            None,
        )
        .expect("scene");
        assert!(scene.available);
    }
}
