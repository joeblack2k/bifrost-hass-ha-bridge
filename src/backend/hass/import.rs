use std::collections::{BTreeSet, HashMap, HashSet};
use std::sync::Arc;

use chrono::Utc;
use maplit::btreeset;
use serde_json::{Value, json};

use hue::api::{
    Button, ButtonData, ButtonMetadata, ButtonReport, ColorTemperature, Device, DeviceArchetype,
    DeviceProductData, Dimming, DimmingUpdate, GroupedLight, Light, LightColor, LightDynamics,
    LightDynamicsStatus, LightEffects, LightEffectsV2, LightGradient, LightLevel, LightMetadata,
    Metadata, MirekSchema, Motion, On, RType, Resource, ResourceLink, Room, RoomArchetype,
    RoomMetadata, Scene, Temperature, ZigbeeConnectivity, ZigbeeConnectivityStatus,
};
use hue::colortemp::kelvin_to_mirek;
use hue::xy::XY;
use uuid::Uuid;

use crate::backend::hass::client::HassState;
use crate::backend::hass::{
    HassBackend, HassEntityBinding, HassEntityKind, HassLightCapabilities, HassServiceKind,
};
use crate::error::ApiResult;
use crate::model::hass::{
    HassEntitySummary, HassLightArchetype, HassSensorKind, HassSwitchMode, HassUiConfig,
};
use crate::resource::Resources;

use super::projections;
use super::{events, light_projection, scene_import};

#[derive(Clone, Debug)]
struct ImportedEntity {
    entity_id: String,
    name: String,
    kind: HassEntityKind,
    service_kind: HassServiceKind,
    state: String,
    available: bool,
    on: bool,
    brightness: Option<f64>,
    xy_color: Option<XY>,
    color_temp: Option<u16>,
    area_name: Option<String>,
    capabilities: HassLightCapabilities,
    detected_sensor_kind: Option<HassSensorKind>,
    sensor_enabled: bool,
    switch_mode: Option<HassSwitchMode>,
    light_archetype: Option<HassLightArchetype>,
    sensor_value: Option<f64>,
    gradient: Option<LightGradient>,
    event_name: Option<String>,
    event_values: Option<Value>,
    effect: Option<hue::api::LightEffect>,
}

impl ImportedEntity {
    fn domain(&self) -> &'static str {
        match self.kind {
            HassEntityKind::Light => "light",
            HassEntityKind::Switch => "switch",
            HassEntityKind::BinarySensor => "binary_sensor",
            HassEntityKind::Sensor => "sensor",
            HassEntityKind::Event => "event",
        }
    }

    fn mapped_type(&self) -> String {
        match self.service_kind {
            HassServiceKind::Light => "light".to_string(),
            HassServiceKind::Switch => {
                if self.switch_mode == Some(HassSwitchMode::Light) {
                    "light".to_string()
                } else {
                    "switch".to_string()
                }
            }
            HassServiceKind::Motion => "motion".to_string(),
            HassServiceKind::Contact => "contact".to_string(),
            HassServiceKind::Temperature => "temperature".to_string(),
            HassServiceKind::LightLevel => "light_level".to_string(),
            HassServiceKind::Button => "button".to_string(),
        }
    }
}

fn value_to_f64(value: &Value) -> Option<f64> {
    value
        .as_f64()
        .or_else(|| {
            value
                .as_u64()
                .and_then(|x| x.to_string().parse::<f64>().ok())
        })
        .or_else(|| {
            value
                .as_i64()
                .and_then(|x| x.to_string().parse::<f64>().ok())
        })
}

fn value_to_u16(value: &Value) -> Option<u16> {
    value
        .as_u64()
        .and_then(|x| u16::try_from(x).ok())
        .or_else(|| value.as_i64().and_then(|x| u16::try_from(x).ok()))
}

fn parse_xy_color(value: &Value) -> Option<XY> {
    let arr = value.as_array()?;
    let [x, y] = arr.as_slice() else {
        return None;
    };
    let x = value_to_f64(x)?;
    let y = value_to_f64(y)?;
    Some(XY {
        x: x.clamp(0.0, 1.0),
        y: y.clamp(0.0, 1.0),
    })
}

fn parse_supported_color_modes(state: &HassState) -> BTreeSet<String> {
    state
        .attributes
        .get("supported_color_modes")
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(Value::as_str)
                .map(|x| x.to_ascii_lowercase())
                .collect::<BTreeSet<_>>()
        })
        .unwrap_or_default()
}

fn parse_light_capabilities(state: &HassState) -> HassLightCapabilities {
    let modes = parse_supported_color_modes(state);
    let has_brightness_attr = state.attributes.contains_key("brightness");
    let has_color_temp_attr = state.attributes.contains_key("color_temp");
    let has_color_temp_kelvin_attr = state.attributes.contains_key("color_temp_kelvin");
    let has_xy_attr = state.attributes.contains_key("xy_color");

    let supports_color = modes
        .iter()
        .any(|m| matches!(m.as_str(), "xy" | "hs" | "rgb" | "rgbw" | "rgbww"));
    let supports_color_temp =
        modes.contains("color_temp") || has_color_temp_attr || has_color_temp_kelvin_attr;
    let effect_values = light_projection::supported_effects(&state.attributes);
    let supports_brightness = has_brightness_attr
        || modes.iter().any(|m| {
            matches!(
                m.as_str(),
                "brightness" | "xy" | "hs" | "rgb" | "rgbw" | "rgbww" | "color_temp"
            )
        });

    HassLightCapabilities {
        supports_brightness,
        supports_color: supports_color || has_xy_attr,
        supports_color_temp,
        supports_effects: !effect_values.is_empty(),
        supports_gradient: light_projection::supports_gradient(&state.attributes),
        effect_values,
    }
}

fn detected_sensor_kind(state: &HassState) -> HassSensorKind {
    match state
        .attributes
        .get("device_class")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_ascii_lowercase()
        .as_str()
    {
        "motion" | "occupancy" | "presence" => HassSensorKind::Motion,
        "door" | "opening" | "window" | "garage_door" => HassSensorKind::Contact,
        _ => HassSensorKind::Ignore,
    }
}

fn parse_imported_entity(state: &HassState, area_name: Option<String>) -> Option<ImportedEntity> {
    let (domain, _) = state.entity_id.split_once('.')?;
    let (kind, service_kind, capabilities, detected_kind) = match domain {
        "light" => (
            HassEntityKind::Light,
            HassServiceKind::Light,
            parse_light_capabilities(state),
            None,
        ),
        "switch" => (
            HassEntityKind::Switch,
            HassServiceKind::Switch,
            HassLightCapabilities::default(),
            None,
        ),
        "binary_sensor" => {
            let detected = detected_sensor_kind(state);
            let sk = match detected {
                HassSensorKind::Motion => HassServiceKind::Motion,
                HassSensorKind::Contact => HassServiceKind::Contact,
                HassSensorKind::Ignore => HassServiceKind::Motion,
            };
            (
                HassEntityKind::BinarySensor,
                sk,
                HassLightCapabilities::default(),
                Some(detected),
            )
        }
        "sensor" => {
            let service_kind = projections::classify_sensor(state)?;
            (
                HassEntityKind::Sensor,
                service_kind,
                HassLightCapabilities::default(),
                None,
            )
        }
        "event" => (
            HassEntityKind::Event,
            HassServiceKind::Button,
            HassLightCapabilities::default(),
            None,
        ),
        _ => return None,
    };

    let available = !matches!(state.state.as_str(), "unavailable" | "unknown");
    let on = available && state.state == "on";

    let name = state
        .attributes
        .get("friendly_name")
        .and_then(Value::as_str)
        .unwrap_or(&state.entity_id)
        .to_string();

    let brightness = if matches!(kind, HassEntityKind::Light) && capabilities.supports_brightness {
        state
            .attributes
            .get("brightness")
            .and_then(value_to_f64)
            .map(|x| x.clamp(0.0, 255.0))
    } else {
        None
    };
    let xy_color = if matches!(kind, HassEntityKind::Light) && capabilities.supports_color {
        state.attributes.get("xy_color").and_then(parse_xy_color)
    } else {
        None
    };
    let color_temp = if matches!(kind, HassEntityKind::Light) && capabilities.supports_color_temp {
        state
            .attributes
            .get("color_temp_kelvin")
            .and_then(value_to_u16)
            .and_then(|x| kelvin_to_mirek(u32::from(x)))
            .or_else(|| {
                state
                    .attributes
                    .get("color_temp")
                    .and_then(value_to_u16)
                    .map(|x| x.clamp(153, 500))
            })
    } else {
        None
    };

    let event_name = matches!(kind, HassEntityKind::Event)
        .then(|| {
            state
                .attributes
                .get("event_type")
                .and_then(Value::as_str)
                .map(events::normalize_event_value)
                .or_else(|| {
                    let state_value = state.state.trim();
                    if state_value.is_empty()
                        || matches!(state_value, "unknown" | "unavailable")
                        || chrono::DateTime::parse_from_rfc3339(state_value).is_ok()
                    {
                        None
                    } else {
                        Some(events::normalize_event_value(state_value))
                    }
                })
        })
        .flatten();
    let event_values = matches!(kind, HassEntityKind::Event)
        .then(|| {
            state
                .attributes
                .get("event_types")
                .or_else(|| state.attributes.get("event_values"))
                .and_then(events::normalize_event_values)
        })
        .flatten();

    Some(ImportedEntity {
        entity_id: state.entity_id.clone(),
        name,
        kind,
        service_kind,
        state: state.state.clone(),
        available,
        on,
        brightness,
        xy_color,
        color_temp,
        area_name,
        capabilities,
        detected_sensor_kind: detected_kind,
        sensor_enabled: true,
        switch_mode: if matches!(kind, HassEntityKind::Switch) {
            Some(HassSwitchMode::Plug)
        } else {
            None
        },
        light_archetype: None,
        sensor_value: matches!(kind, HassEntityKind::Sensor)
            .then(|| projections::numeric_value(state))
            .flatten(),
        gradient: matches!(kind, HassEntityKind::Light)
            .then(|| light_projection::gradient_from_attributes(&state.attributes))
            .flatten(),
        event_name,
        event_values,
        effect: matches!(kind, HassEntityKind::Light)
            .then(|| {
                state
                    .attributes
                    .get("effect")
                    .and_then(Value::as_str)
                    .and_then(light_projection::parse_effect)
            })
            .flatten(),
    })
}

fn device_archetype(archetype: HassLightArchetype) -> DeviceArchetype {
    match archetype {
        HassLightArchetype::ClassicBulb => DeviceArchetype::ClassicBulb,
        HassLightArchetype::SultanBulb => DeviceArchetype::SultanBulb,
        HassLightArchetype::CandleBulb => DeviceArchetype::CandleBulb,
        HassLightArchetype::SpotBulb => DeviceArchetype::SpotBulb,
        HassLightArchetype::VintageBulb => DeviceArchetype::VintageBulb,
        HassLightArchetype::FloodBulb => DeviceArchetype::FloodBulb,
        HassLightArchetype::CeilingRound => DeviceArchetype::CeilingRound,
        HassLightArchetype::CeilingSquare => DeviceArchetype::CeilingSquare,
        HassLightArchetype::PendantRound => DeviceArchetype::PendantRound,
        HassLightArchetype::PendantLong => DeviceArchetype::PendantLong,
        HassLightArchetype::FloorShade => DeviceArchetype::FloorShade,
        HassLightArchetype::FloorLantern => DeviceArchetype::FloorLantern,
        HassLightArchetype::TableShade => DeviceArchetype::TableShade,
        HassLightArchetype::WallSpot => DeviceArchetype::WallSpot,
        HassLightArchetype::WallLantern => DeviceArchetype::WallLantern,
        HassLightArchetype::RecessedCeiling => DeviceArchetype::RecessedCeiling,
        HassLightArchetype::HueLightstrip => DeviceArchetype::HueLightstrip,
        HassLightArchetype::HuePlay => DeviceArchetype::HuePlay,
        HassLightArchetype::HueGo => DeviceArchetype::HueGo,
        HassLightArchetype::HueBloom => DeviceArchetype::HueBloom,
        HassLightArchetype::HueIris => DeviceArchetype::HueIris,
        HassLightArchetype::HueSigne => DeviceArchetype::HueSigne,
        HassLightArchetype::HueTube => DeviceArchetype::HueTube,
    }
}

fn light_archetype(imported: &ImportedEntity) -> DeviceArchetype {
    match imported.kind {
        HassEntityKind::Light => device_archetype(
            imported
                .light_archetype
                .unwrap_or(HassLightArchetype::ClassicBulb),
        ),
        HassEntityKind::Switch => {
            if imported.switch_mode == Some(HassSwitchMode::Light) {
                device_archetype(
                    imported
                        .light_archetype
                        .unwrap_or(HassLightArchetype::ClassicBulb),
                )
            } else {
                DeviceArchetype::Plug
            }
        }
        HassEntityKind::BinarySensor | HassEntityKind::Sensor | HassEntityKind::Event => {
            DeviceArchetype::UnknownArchetype
        }
    }
}

fn make_device(service_link: ResourceLink, imported: &ImportedEntity) -> Device {
    let archetype = light_archetype(imported);
    let domain = imported.domain();

    Device {
        product_data: DeviceProductData {
            model_id: format!("hass-{domain}"),
            manufacturer_name: "Home Assistant".to_string(),
            product_name: imported.name.clone(),
            product_archetype: archetype.clone(),
            certified: false,
            software_version: "1.0.0".to_string(),
            hardware_platform_type: None,
        },
        metadata: Metadata::new(archetype, &imported.name),
        services: btreeset![service_link],
        usertest: None,
        identify: None,
    }
}

fn ieee_like_from_uuid(id: &Uuid) -> String {
    let b = id.as_bytes();
    // Hue expects an EUI-64 style string for zigbee_connectivity.
    format!(
        "{:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
        b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7]
    )
}

fn apply_light_state(light: &mut Light, imported: &ImportedEntity) {
    light.metadata.name.clone_from(&imported.name);
    light.metadata.archetype = light_archetype(imported);
    light.on = On { on: imported.on };

    match imported.kind {
        HassEntityKind::Light => {
            // Hue clients infer supported controls from field presence, not just capability flags.
            // Home Assistant often omits brightness/color/ct values when the light is off.
            // Keep the last known values (or set a sane default) so the Hue app still shows controls.
            if imported.capabilities.supports_brightness {
                if let Some(b) = imported.brightness {
                    light.dimming = Some(Dimming {
                        brightness: (b / 255.0 * 100.0).clamp(0.0, 100.0),
                        min_dim_level: None,
                    });
                } else if light.dimming.is_none() {
                    light.dimming = Some(Dimming {
                        brightness: 100.0,
                        min_dim_level: None,
                    });
                }
            } else {
                light.dimming = None;
            }

            if imported.capabilities.supports_color {
                if let Some(xy) = imported.xy_color {
                    light.color = Some(LightColor::new(xy));
                } else if light.color.is_none() {
                    // Default to D65 white point.
                    light.color = Some(LightColor::new(XY {
                        x: 0.3127,
                        y: 0.3290,
                    }));
                }
            } else {
                light.color = None;
            }

            if imported.capabilities.supports_color_temp {
                if let Some(mirek) = imported.color_temp {
                    light.color_temperature = Some(ColorTemperature {
                        mirek: Some(mirek),
                        mirek_schema: MirekSchema::DEFAULT,
                        mirek_valid: true,
                    });
                } else if light.color_temperature.is_none() {
                    light.color_temperature = Some(ColorTemperature {
                        mirek: Some(366),
                        mirek_schema: MirekSchema::DEFAULT,
                        mirek_valid: true,
                    });
                }
            } else {
                light.color_temperature = None;
            }
            if !imported.capabilities.supports_color_temp {
                light.color_temperature_delta = None;
            }
        }
        HassEntityKind::Switch
        | HassEntityKind::BinarySensor
        | HassEntityKind::Sensor
        | HassEntityKind::Event => {
            light.dimming = None;
            light.color = None;
            light.color_temperature = None;
            light.color_temperature_delta = None;
        }
    }

    // Light::new contains the complete Hue capability set. HA-backed resources must only expose
    // controls handled by backend_light_update; this is deliberately scoped to this importer so
    // real Zigbee/Z2M lights keep their native capabilities.
    light.alert = None;
    light.color_temperature_delta = None;
    light.dimming_delta = None;
    let effect_values = imported.capabilities.effect_values.clone();
    let mut status_values = vec![hue::api::LightEffect::NoEffect];
    let additional_status_values = effect_values
        .iter()
        .copied()
        .filter(|effect| !status_values.contains(effect))
        .collect::<Vec<_>>();
    status_values.extend(additional_status_values);
    let current_effect = imported.effect.unwrap_or(hue::api::LightEffect::NoEffect);
    light.effects =
        (imported.kind == HassEntityKind::Light && !effect_values.is_empty()).then(|| {
            LightEffects {
                status_values: status_values.clone(),
                status: current_effect,
                effect_values: effect_values.clone(),
            }
        });
    light.effects_v2 =
        (imported.kind == HassEntityKind::Light && !effect_values.is_empty()).then(|| {
            LightEffectsV2 {
                action: hue::api::LightEffectValues {
                    effect_values: effect_values.clone(),
                },
                status: hue::api::LightEffectStatus {
                    effect: current_effect,
                    effect_values: status_values,
                    parameters: None,
                },
            }
        });
    light.gradient = (imported.kind == HassEntityKind::Light)
        .then(|| imported.gradient.clone())
        .flatten();
    light.service_id = None;
    light.timed_effects = None;
    light.signaling = None;
    light.dynamics = match imported.kind {
        HassEntityKind::Light => Some(LightDynamics {
            status: LightDynamicsStatus::None,
            status_values: vec![LightDynamicsStatus::None],
            speed: 0.0,
            speed_valid: false,
        }),
        HassEntityKind::Switch
        | HassEntityKind::BinarySensor
        | HassEntityKind::Sensor
        | HassEntityKind::Event => None,
    };
}

fn make_contact_resource(imported: &ImportedEntity, device_link: ResourceLink) -> Value {
    json!({
        "owner": device_link,
        "enabled": imported.sensor_enabled,
        "contact": {
            "contact": imported.on,
            "contact_valid": imported.available,
            "last_updated": Utc::now().to_rfc3339(),
        }
    })
}

fn make_button_resource(imported: &ImportedEntity, device_link: ResourceLink) -> Button {
    let event = imported.event_name.clone();
    Button {
        owner: device_link,
        metadata: ButtonMetadata { control_id: 0 },
        button: ButtonData {
            button_report: event.clone().map(|event| ButtonReport {
                updated: Utc::now(),
                event,
            }),
            last_event: event.map(Value::String),
            repeat_interval: Some(100),
            event_values: imported.event_values.clone().or_else(|| {
                Some(json!([
                    "initial_press",
                    "press",
                    "double_press",
                    "triple_press",
                    "hold",
                    "release",
                    "repeat"
                ]))
            }),
        },
    }
}

impl HassBackend {
    fn links_for_entity(
        &self,
        entity_id: &str,
        service_kind: HassServiceKind,
    ) -> (ResourceLink, ResourceLink) {
        let key = format!("hass:{}:{}", self.name, entity_id);
        let service = match service_kind {
            HassServiceKind::Light | HassServiceKind::Switch => {
                RType::Light.deterministic(format!("{key}:light"))
            }
            HassServiceKind::Motion => RType::Motion.deterministic(format!("{key}:motion")),
            HassServiceKind::Contact => RType::Contact.deterministic(format!("{key}:contact")),
            HassServiceKind::Temperature => {
                RType::Temperature.deterministic(format!("{key}:temperature"))
            }
            HassServiceKind::LightLevel => {
                RType::LightLevel.deterministic(format!("{key}:light_level"))
            }
            HassServiceKind::Button => RType::Button.deterministic(format!("{key}:button")),
        };
        (
            RType::Device.deterministic(format!("{key}:device")),
            service,
        )
    }

    pub(super) fn ensure_rooms(
        &mut self,
        res: &mut Resources,
        config: &HassUiConfig,
    ) -> ApiResult<()> {
        let wanted = config
            .rooms
            .iter()
            .map(|room| {
                let binding = self.room_binding(room);
                (room.id.clone(), binding)
            })
            .collect::<HashMap<_, _>>();

        for room in &config.rooms {
            let binding = wanted
                .get(&room.id)
                .expect("wanted map must contain configured room");

            if res.get::<Room>(&binding.room_link).is_err() {
                let room = Room {
                    children: BTreeSet::new(),
                    metadata: RoomMetadata::new(RoomArchetype::Home, &binding.room_name),
                    services: btreeset![binding.grouped_light_link],
                };
                res.add(&binding.room_link, Resource::Room(room))?;
            } else {
                res.update::<Room>(&binding.room_link.rid, |room| {
                    room.metadata.name.clone_from(&binding.room_name);
                    room.services = btreeset![binding.grouped_light_link];
                })?;
            }

            if res
                .get::<GroupedLight>(&binding.grouped_light_link)
                .is_err()
            {
                res.add(
                    &binding.grouped_light_link,
                    Resource::GroupedLight(GroupedLight::new(binding.room_link)),
                )?;
            }
        }

        for id in res.get_resource_ids_by_type(RType::BridgeHome) {
            res.update(&id, |bh: &mut hue::api::BridgeHome| {
                bh.children.extend(wanted.values().map(|x| x.room_link));
            })?;
        }

        let stale_rooms = self
            .room_map
            .iter()
            .filter(|(room_id, _)| !wanted.contains_key(*room_id))
            .map(|(_, binding)| binding.clone())
            .collect::<Vec<_>>();
        for stale in stale_rooms {
            if let Err(err) = res.delete(&stale.room_link) {
                log::warn!(
                    "[{}] Failed to delete stale room {}: {}",
                    self.name,
                    stale.room_id,
                    err
                );
            }
        }

        self.room_map = wanted;
        Ok(())
    }

    pub(super) async fn refresh_rooms_from_ui_config(&mut self) -> ApiResult<()> {
        let ui_config = {
            let ui = self.ui_state.lock().await;
            ui.config_normalized()
        };

        let mut entity_room = HashMap::new();
        for binding in self.entity_map.values() {
            let room_id = ui_config
                .entity_preferences
                .get(&binding.entity_id)
                .and_then(|pref| pref.room_id.clone())
                .filter(|room_id| ui_config.rooms.iter().any(|room| room.id == *room_id))
                .unwrap_or_else(|| HassUiConfig::DEFAULT_ROOM_ID.to_string());
            entity_room.insert(binding.entity_id.clone(), room_id);
        }

        let state = Arc::clone(&self.state);
        let mut res = state.lock().await;
        self.ensure_rooms(&mut res, &ui_config)?;

        let mut children_by_room = self
            .room_map
            .keys()
            .map(|room_id| (room_id.clone(), BTreeSet::<ResourceLink>::new()))
            .collect::<HashMap<_, _>>();

        for binding in self.entity_map.values() {
            let room_id = entity_room
                .get(&binding.entity_id)
                .cloned()
                .unwrap_or_else(|| HassUiConfig::DEFAULT_ROOM_ID.to_string());
            children_by_room
                .entry(room_id)
                .or_default()
                .insert(binding.device_link);
        }

        for room in self.room_map.values() {
            let children = children_by_room
                .get(&room.room_id)
                .cloned()
                .unwrap_or_default();
            res.update::<Room>(&room.room_link.rid, |hue_room| {
                hue_room.metadata.name.clone_from(&room.room_name);
                hue_room.children = children;
            })?;
        }
        drop(res);

        {
            let mut ui = self.ui_state.lock().await;
            for summary in &mut ui.entities {
                if let Some(room_id) = entity_room.get(&summary.entity_id) {
                    summary.room_id.clone_from(room_id);
                    summary.room_name = ui_config.room_name(room_id);
                }
            }
        }

        self.ui_log("Updated room metadata and assignments from UI config")
            .await;
        Ok(())
    }

    fn sync_single_entity(
        &mut self,
        imported: &ImportedEntity,
        res: &mut Resources,
    ) -> ApiResult<()> {
        let (device_link, service_link) =
            self.links_for_entity(&imported.entity_id, imported.service_kind);
        let link_zbc = RType::ZigbeeConnectivity
            .deterministic(format!("hass:{}:{}:zbc", self.name, imported.entity_id));
        let binding = self
            .entity_map
            .entry(imported.entity_id.clone())
            .or_insert_with(|| HassEntityBinding {
                entity_id: imported.entity_id.clone(),
                name: imported.name.clone(),
                kind: imported.kind,
                service_kind: imported.service_kind,
                service_link,
                device_link,
                capabilities: imported.capabilities.clone(),
                switch_mode: imported.switch_mode,
            });

        let previous_service_link = binding.service_link;
        binding.name.clone_from(&imported.name);
        binding.kind = imported.kind;
        binding.service_kind = imported.service_kind;
        binding.service_link = service_link;
        binding.device_link = device_link;
        binding.capabilities = imported.capabilities.clone();
        binding.switch_mode = imported.switch_mode;

        if previous_service_link != binding.service_link {
            self.light_map.remove(&previous_service_link.rid);
            self.sensor_map.remove(&previous_service_link.rid);
            self.button_map.remove(&previous_service_link.rid);
            if res.get_resource(&previous_service_link).is_ok() {
                let _ = res.delete(&previous_service_link);
            }
        }

        self.device_map
            .insert(binding.device_link.rid, imported.entity_id.clone());
        match imported.service_kind {
            HassServiceKind::Light | HassServiceKind::Switch => {
                self.light_map
                    .insert(binding.service_link.rid, imported.entity_id.clone());
                self.sensor_map.remove(&binding.service_link.rid);
            }
            HassServiceKind::Motion
            | HassServiceKind::Contact
            | HassServiceKind::Temperature
            | HassServiceKind::LightLevel => {
                self.sensor_map
                    .insert(binding.service_link.rid, imported.entity_id.clone());
                self.light_map.remove(&binding.service_link.rid);
            }
            HassServiceKind::Button => {
                self.button_map
                    .insert(binding.service_link.rid, imported.entity_id.clone());
                self.light_map.remove(&binding.service_link.rid);
                self.sensor_map.remove(&binding.service_link.rid);
            }
        }

        if res.get::<Device>(&binding.device_link).is_err() {
            let mut dev = make_device(binding.service_link, imported);
            dev.services.insert(link_zbc);
            res.add(&binding.device_link, Resource::Device(dev))?;
        } else {
            res.update::<Device>(&binding.device_link.rid, |dev| {
                dev.metadata.name.clone_from(&imported.name);
                dev.metadata.archetype = light_archetype(imported);
                dev.product_data.product_name.clone_from(&imported.name);
                dev.product_data.product_archetype = light_archetype(imported);
                dev.services = btreeset![binding.service_link, link_zbc];
            })?;
        }

        if res.get::<ZigbeeConnectivity>(&link_zbc).is_err() {
            // Hue app expects zigbee_connectivity for "real" devices. For HA entities we emulate it.
            let zbc = ZigbeeConnectivity {
                owner: binding.device_link,
                mac_address: ieee_like_from_uuid(&binding.device_link.rid),
                status: ZigbeeConnectivityStatus::Connected,
                channel: Some(json!({
                    "status": "set",
                    "value": "channel_25",
                })),
                extended_pan_id: None,
            };
            res.add(&link_zbc, Resource::ZigbeeConnectivity(zbc))?;
        }

        match imported.service_kind {
            HassServiceKind::Light | HassServiceKind::Switch => {
                if res.get::<Light>(&binding.service_link).is_err() {
                    let mut light = Light::new(
                        binding.device_link,
                        LightMetadata::new(light_archetype(imported), &imported.name),
                    );
                    // HA has no portable startup hook; expose no default power-up, while
                    // apply_light_state deliberately preserves a user-configured one.
                    light.powerup = None;
                    apply_light_state(&mut light, imported);
                    res.add(&binding.service_link, Resource::Light(light))?;
                } else {
                    res.update::<Light>(&binding.service_link.rid, |light| {
                        apply_light_state(light, imported);
                    })?;
                }
            }
            HassServiceKind::Motion => {
                if res.get::<Motion>(&binding.service_link).is_err() {
                    res.add(
                        &binding.service_link,
                        Resource::Motion(Motion {
                            enabled: imported.sensor_enabled,
                            owner: binding.device_link,
                            motion: json!({
                                "motion": imported.on,
                                "motion_valid": imported.available,
                                "last_updated": Utc::now().to_rfc3339(),
                            }),
                            sensitivity: json!({}),
                        }),
                    )?;
                } else {
                    res.update::<Motion>(&binding.service_link.rid, |motion| {
                        motion.enabled = imported.sensor_enabled;
                        motion.motion = json!({
                            "motion": imported.on,
                            "motion_valid": imported.available,
                            "last_updated": Utc::now().to_rfc3339(),
                        });
                    })?;
                }
            }
            HassServiceKind::Contact => {
                let value = make_contact_resource(imported, binding.device_link);
                if res.get_resource(&binding.service_link).is_ok() {
                    let _ = res.delete(&binding.service_link);
                }
                res.add(&binding.service_link, Resource::Contact(value))?;
            }
            HassServiceKind::Temperature => {
                let value = projections::resource_payload(
                    imported.service_kind,
                    imported.sensor_value,
                    imported.available,
                );
                if res.get::<Temperature>(&binding.service_link).is_err() {
                    res.add(
                        &binding.service_link,
                        Resource::Temperature(Temperature {
                            enabled: imported.sensor_enabled,
                            owner: binding.device_link,
                            temperature: value,
                        }),
                    )?;
                } else {
                    res.update::<Temperature>(&binding.service_link.rid, |temperature| {
                        temperature.enabled = imported.sensor_enabled;
                        temperature.temperature = value.clone();
                    })?;
                }
            }
            HassServiceKind::LightLevel => {
                let value = projections::resource_payload(
                    imported.service_kind,
                    imported.sensor_value,
                    imported.available,
                );
                if res.get::<LightLevel>(&binding.service_link).is_err() {
                    res.add(
                        &binding.service_link,
                        Resource::LightLevel(LightLevel {
                            enabled: imported.sensor_enabled,
                            owner: binding.device_link,
                            light: value,
                        }),
                    )?;
                } else {
                    res.update::<LightLevel>(&binding.service_link.rid, |light_level| {
                        light_level.enabled = imported.sensor_enabled;
                        light_level.light = value.clone();
                    })?;
                }
            }
            HassServiceKind::Button => {
                let button = make_button_resource(imported, binding.device_link);
                if res.get::<Button>(&binding.service_link).is_err() {
                    res.add(&binding.service_link, Resource::Button(button))?;
                } else {
                    res.update::<Button>(&binding.service_link.rid, |current| {
                        *current = button.clone();
                    })?;
                }
            }
        }

        Ok(())
    }

    fn prune_homeassistant_devices(
        &mut self,
        res: &mut Resources,
        keep_device_rids: &HashSet<Uuid>,
    ) -> ApiResult<usize> {
        let mut removed = 0_usize;

        // Delete only devices that Bifrost created for the Home Assistant backend.
        // This keeps "real" Hue devices (and other backends) safe.
        let device_ids = res.get_resource_ids_by_type(RType::Device);
        for rid in device_ids {
            if keep_device_rids.contains(&rid) {
                continue;
            }

            let Ok(dev) = res.get_id::<Device>(rid) else {
                continue;
            };

            if dev.product_data.manufacturer_name != "Home Assistant" {
                continue;
            }
            if !dev.product_data.model_id.starts_with("hass-") {
                continue;
            }

            let link = RType::Device.link_to(rid);
            if res.delete(&link).is_ok() {
                removed += 1;
                if let Some(entity_id) = self.device_map.remove(&rid) {
                    if let Some(binding) = self.entity_map.remove(&entity_id) {
                        self.light_map.remove(&binding.service_link.rid);
                        self.sensor_map.remove(&binding.service_link.rid);
                        self.button_map.remove(&binding.service_link.rid);
                    }
                }
            }
        }

        Ok(removed)
    }

    fn sync_grouped_light_states(
        &self,
        imported_map: &HashMap<String, ImportedEntity>,
        entity_room: &HashMap<String, String>,
        res: &mut Resources,
    ) -> ApiResult<()> {
        for room in self.room_map.values() {
            let mut any_on = false;
            let mut values = Vec::new();

            for binding in self.entity_map.values() {
                if entity_room.get(&binding.entity_id) != Some(&room.room_id) {
                    continue;
                }
                let grouped_as_light = match binding.kind {
                    HassEntityKind::Light => true,
                    HassEntityKind::Switch => {
                        binding.switch_mode.unwrap_or(HassSwitchMode::Plug) == HassSwitchMode::Light
                    }
                    HassEntityKind::BinarySensor
                    | HassEntityKind::Sensor
                    | HassEntityKind::Event => false,
                };
                if !grouped_as_light {
                    continue;
                }
                if let Some(imported) = imported_map.get(&binding.entity_id) {
                    any_on |= imported.on;
                    if let Some(br) = imported.brightness {
                        values.push((br / 255.0 * 100.0).clamp(0.0, 100.0));
                    }
                }
            }

            let dimming = if values.is_empty() {
                None
            } else {
                let sum: f64 = values.iter().sum();
                let count = u32::try_from(values.len()).map_or(1.0, f64::from);
                Some(DimmingUpdate::new(sum / count))
            };

            res.update::<GroupedLight>(&room.grouped_light_link.rid, |grouped| {
                grouped.on = Some(On { on: any_on });
                grouped.dimming = dimming;
            })?;
        }

        Ok(())
    }

    fn assigned_room_id(config: &HassUiConfig, imported: &ImportedEntity) -> String {
        if let Some(room_id) = config
            .entity_preferences
            .get(&imported.entity_id)
            .and_then(|x| x.room_id.as_ref())
            .filter(|room_id| config.rooms.iter().any(|r| &r.id == *room_id))
        {
            return room_id.clone();
        }

        if config.sync_hass_areas_to_rooms {
            if let Some(area_name) = imported.area_name.as_deref() {
                if let Some(room_id) = config.room_for_area(area_name) {
                    return room_id;
                }
            }
        }

        HassUiConfig::DEFAULT_ROOM_ID.to_string()
    }

    fn assigned_scene_room_id(
        config: &HassUiConfig,
        imported: &scene_import::ImportedScene,
        area_map: &HashMap<String, String>,
    ) -> String {
        if let Some(room_id) = config
            .entity_preferences
            .get(&imported.entity_id)
            .and_then(|preference| preference.room_id.as_ref())
            .filter(|room_id| config.rooms.iter().any(|room| &room.id == *room_id))
        {
            return room_id.clone();
        }

        if config.sync_hass_areas_to_rooms {
            if let Some(area_name) = imported.area_name.as_deref() {
                if let Some(room_id) = config.room_for_area(area_name) {
                    return room_id;
                }
            }
            for target in &imported.targets {
                if let Some(area_name) = area_map.get(target) {
                    if let Some(room_id) = config.room_for_area(area_name) {
                        return room_id;
                    }
                }
            }
        }

        HassUiConfig::DEFAULT_ROOM_ID.to_string()
    }

    fn sync_imported_scene(
        &mut self,
        imported: &scene_import::ImportedScene,
        states: &HashMap<String, HassState>,
        config: &HassUiConfig,
        area_map: &HashMap<String, String>,
        res: &mut Resources,
    ) -> ApiResult<Option<ResourceLink>> {
        if !config.should_include(&imported.entity_id, &imported.name, imported.available) {
            return Ok(None);
        }

        let room_id = Self::assigned_scene_room_id(config, imported, area_map);
        let Some(room) = self.room_map.get(&room_id) else {
            return Ok(None);
        };
        let actions = imported
            .targets
            .iter()
            .filter_map(|target| {
                let binding = self.entity_map.get(target)?;
                let include = match binding.kind {
                    HassEntityKind::Light => true,
                    HassEntityKind::Switch => {
                        binding.switch_mode.unwrap_or(HassSwitchMode::Plug) == HassSwitchMode::Light
                    }
                    HassEntityKind::BinarySensor
                    | HassEntityKind::Sensor
                    | HassEntityKind::Event => false,
                };
                include
                    .then(|| scene_import::scene_action(binding.service_link, states.get(target)))
            })
            .collect::<Vec<_>>();

        let link = scene_import::link(&self.name, &imported.entity_id);
        let scene = scene_import::build(imported, room.room_link, actions);
        if res.get::<Scene>(&link).is_err() {
            res.add(&link, Resource::Scene(scene))?;
        } else {
            res.update::<Scene>(&link.rid, |current| *current = scene.clone())?;
        }
        self.scene_map.insert(link.rid, imported.entity_id.clone());
        Ok(Some(link))
    }

    fn prune_persisted_imported_scenes(
        &mut self,
        res: &mut Resources,
        keep_scene_rids: &HashSet<Uuid>,
    ) -> usize {
        let mut removed = 0;
        for rid in res.get_resource_ids_by_type(RType::Scene) {
            let imported = res
                .get_id::<Scene>(rid)
                .ok()
                .is_some_and(scene_import::is_imported);
            if !imported || keep_scene_rids.contains(&rid) {
                continue;
            }

            if res.delete(&RType::Scene.link_to(rid)).is_ok() {
                self.scene_map.remove(&rid);
                self.imported_scene_map.remove(&rid);
                removed += 1;
            }
        }
        removed
    }

    pub(super) async fn sync_entities(&mut self) -> ApiResult<()> {
        self.apply_runtime_connection().await?;

        let states = self.client.get_states().await?;
        let core_config = self.client.get_core_config().await.ok();
        let area_map = match self.client.get_entity_areas().await {
            Ok(map) => map,
            Err(err) => {
                log::warn!(
                    "[{}] Failed to query Home Assistant areas. Continuing without area mapping: {}",
                    self.name,
                    err
                );
                self.ui_log(format!("Area sync fallback (no areas): {err}"))
                    .await;
                HashMap::new()
            }
        };

        let mut parsed = states
            .iter()
            .filter_map(|state| {
                parse_imported_entity(state, area_map.get(&state.entity_id).cloned())
            })
            .collect::<Vec<_>>();
        parsed.sort_by(|a, b| a.entity_id.cmp(&b.entity_id));
        let parsed_scenes = states
            .iter()
            .filter_map(|state| scene_import::parse(state, area_map.get(&state.entity_id).cloned()))
            .collect::<Vec<_>>();

        let mut ui_state = self.ui_state.lock().await;
        let mut ui_config = ui_state.config_normalized();
        let mut changed = false;
        if let Some(core) = core_config {
            let timezone = core
                .timezone
                .map(|x| x.trim().to_string())
                .filter(|x| !x.is_empty());
            let lat = core.latitude.map(|x| format!("{x:.4}"));
            let long = core.longitude.map(|x| format!("{x:.4}"));
            if ui_config.hass_timezone != timezone
                || ui_config.hass_lat != lat
                || ui_config.hass_long != long
            {
                ui_config.set_hass_location(timezone, lat, long);
                changed = true;
            }
        }
        if ui_config.sync_hass_areas_to_rooms {
            for imported in &parsed {
                if let Some(area_name) = imported.area_name.as_deref() {
                    if ui_config.room_for_area(area_name).is_none() {
                        let _ = ui_config.ensure_room_for_area(area_name);
                        changed = true;
                    }
                }
            }
        }
        if changed {
            ui_state.set_config(ui_config.clone());
            ui_state.persist_and_log("Synced Home Assistant metadata into Bifrost state")?;
        } else {
            ui_state.set_config(ui_config.clone());
        }
        drop(ui_state);

        let mut imported_included = HashMap::new();
        let mut summaries = Vec::with_capacity(parsed.len());
        let mut entity_room = HashMap::new();

        for imported in &parsed {
            let mut imported = imported.clone();
            if let Some(alias) = ui_config.entity_alias(&imported.entity_id) {
                imported.name = alias;
            }
            if matches!(imported.kind, HassEntityKind::Switch) {
                imported.switch_mode = Some(ui_config.switch_mode(&imported.entity_id));
            }
            if matches!(imported.kind, HassEntityKind::Light)
                || (matches!(imported.kind, HassEntityKind::Switch)
                    && imported.switch_mode == Some(HassSwitchMode::Light))
            {
                imported.light_archetype = Some(ui_config.light_archetype(&imported.entity_id));
            }

            let detected_sensor_kind = imported
                .detected_sensor_kind
                .unwrap_or(HassSensorKind::Ignore);
            if matches!(imported.kind, HassEntityKind::BinarySensor) {
                imported.service_kind =
                    match ui_config.sensor_kind(&imported.entity_id, detected_sensor_kind) {
                        HassSensorKind::Motion => HassServiceKind::Motion,
                        HassSensorKind::Contact => HassServiceKind::Contact,
                        HassSensorKind::Ignore => imported.service_kind,
                    };
            }
            if matches!(
                imported.kind,
                HassEntityKind::BinarySensor | HassEntityKind::Sensor
            ) {
                imported.sensor_enabled = ui_config.sensor_enabled(&imported.entity_id);
            }

            let hidden = ui_config.is_manually_hidden(&imported.entity_id);
            let room_id = Self::assigned_room_id(&ui_config, &imported);
            let room_name = ui_config.room_name(&room_id);
            let selected_sensor_kind = match imported.service_kind {
                HassServiceKind::Motion => Some(HassSensorKind::Motion),
                HassServiceKind::Contact => Some(HassSensorKind::Contact),
                HassServiceKind::Light
                | HassServiceKind::Switch
                | HassServiceKind::Temperature
                | HassServiceKind::LightLevel
                | HassServiceKind::Button => None,
            };

            let mut included =
                ui_config.should_include(&imported.entity_id, &imported.name, imported.available);
            if matches!(imported.kind, HassEntityKind::BinarySensor)
                && matches!(
                    ui_config.sensor_kind(&imported.entity_id, detected_sensor_kind),
                    HassSensorKind::Ignore
                )
            {
                included = false;
            }

            if included {
                imported_included.insert(imported.entity_id.clone(), imported.clone());
                entity_room.insert(imported.entity_id.clone(), room_id.clone());
            }

            summaries.push(HassEntitySummary {
                entity_id: imported.entity_id.clone(),
                domain: imported.domain().to_string(),
                name: imported.name.clone(),
                state: imported.state.clone(),
                available: imported.available,
                included,
                hidden,
                area_name: imported.area_name.clone(),
                room_id,
                room_name,
                mapped_type: imported.mapped_type(),
                supports_brightness: imported.capabilities.supports_brightness,
                supports_color: imported.capabilities.supports_color,
                supports_color_temp: imported.capabilities.supports_color_temp,
                switch_mode: imported.switch_mode,
                sensor_kind: selected_sensor_kind,
                light_archetype: imported.light_archetype,
                enabled: imported.sensor_enabled,
            });
        }

        {
            let mut ui_state = self.ui_state.lock().await;
            ui_state.entities = summaries;
        }

        let state = self.state.clone();
        let mut res = state.lock().await;
        self.ensure_rooms(&mut res, &ui_config)?;

        for imported in imported_included.values() {
            self.sync_single_entity(imported, &mut res)?;
        }

        // If the user previously exposed many entities, they may still exist in the persisted
        // Hue resource DB after a restart (since `entity_map` is in-memory only). Always prune
        // any Home Assistant-generated devices that are no longer included. Keep only unavailable
        // lights that carry bridge-local power-up state; ordinary unavailable resources retain the
        // existing include_unavailable behavior.
        let unavailable_powerup_lights = parsed
            .iter()
            .filter_map(|imported| {
                if imported.kind != HassEntityKind::Light || imported.available {
                    return None;
                }
                let (device_link, service_link) =
                    self.links_for_entity(&imported.entity_id, imported.service_kind);
                res.get::<Light>(&service_link)
                    .ok()
                    .filter(|light| light.powerup.is_some())
                    .map(|_| (imported.entity_id.clone(), device_link.rid))
            })
            .collect::<HashMap<_, _>>();
        let mut keep_device_rids = imported_included
            .values()
            .map(|imported| {
                let (device_link, _service_link) =
                    self.links_for_entity(&imported.entity_id, imported.service_kind);
                device_link.rid
            })
            .collect::<HashSet<_>>();
        keep_device_rids.extend(unavailable_powerup_lights.values().copied());
        let pruned = self.prune_homeassistant_devices(&mut res, &keep_device_rids)?;
        if pruned > 0 {
            self.ui_log(format!(
                "Pruned {pruned} stale Home Assistant devices from Hue bridge"
            ))
            .await;
        }

        let stale = self
            .entity_map
            .keys()
            .filter(|entity_id| !imported_included.contains_key(*entity_id))
            .cloned()
            .collect::<Vec<_>>();
        for entity_id in stale {
            if let Some(binding) = self.entity_map.remove(&entity_id) {
                self.light_map.remove(&binding.service_link.rid);
                self.sensor_map.remove(&binding.service_link.rid);
                self.button_map.remove(&binding.service_link.rid);
                self.device_map.remove(&binding.device_link.rid);
                if !unavailable_powerup_lights.contains_key(&entity_id) {
                    if let Err(err) = res.delete(&binding.device_link) {
                        log::warn!(
                            "[{}] Failed to delete stale entity {}: {}",
                            self.name,
                            entity_id,
                            err
                        );
                    }
                }
            }
        }

        let mut children_by_room = self
            .room_map
            .keys()
            .map(|room_id| (room_id.clone(), BTreeSet::<ResourceLink>::new()))
            .collect::<HashMap<_, _>>();

        for binding in self.entity_map.values() {
            let room_id = entity_room
                .get(&binding.entity_id)
                .cloned()
                .unwrap_or_else(|| HassUiConfig::DEFAULT_ROOM_ID.to_string());
            children_by_room
                .entry(room_id)
                .or_default()
                .insert(binding.device_link);
        }

        for room in self.room_map.values() {
            let children = children_by_room
                .get(&room.room_id)
                .cloned()
                .unwrap_or_default();
            res.update::<Room>(&room.room_link.rid, |hue_room| {
                hue_room.children = children;
            })?;
        }

        self.sync_grouped_light_states(&imported_included, &entity_room, &mut res)?;

        let scene_states = states
            .iter()
            .map(|state| (state.entity_id.clone(), state.clone()))
            .collect::<HashMap<_, _>>();
        let mut imported_scenes = HashMap::new();
        for imported in &parsed_scenes {
            if let Some(link) =
                self.sync_imported_scene(imported, &scene_states, &ui_config, &area_map, &mut res)?
            {
                imported_scenes.insert(link.rid, imported.entity_id.clone());
            }
        }
        let kept_scene_rids = imported_scenes.keys().copied().collect::<HashSet<_>>();
        let pruned_scenes = self.prune_persisted_imported_scenes(&mut res, &kept_scene_rids);
        if pruned_scenes > 0 {
            self.ui_log(format!(
                "Pruned {pruned_scenes} stale Home Assistant scenes from Hue bridge"
            ))
            .await;
        }
        self.imported_scene_map = imported_scenes;

        self.ui_log(format!(
            "Synced {} entities ({} exposed, {} hidden) across {} rooms",
            parsed.len(),
            imported_included.len(),
            parsed.len().saturating_sub(imported_included.len()),
            self.room_map.len()
        ))
        .await;

        Ok(())
    }

    pub(super) async fn sync_entity_by_id(&mut self, entity_id: &str) -> ApiResult<()> {
        self.apply_runtime_connection().await?;

        let state = self.client.get_state(entity_id).await?;
        let area_name = self.client.get_entity_area(entity_id).await.ok().flatten();
        if scene_import::parse(&state, area_name.clone()).is_some() {
            // Scene attributes contain the complete target list only on a full state read. A
            // single-scene update therefore reuses the normal full-sync path instead of creating
            // a partial resource that could silently lose targets.
            return self.sync_entities().await;
        }
        let Some(mut imported) = parse_imported_entity(&state, area_name) else {
            return Err(crate::error::ApiError::service_error(format!(
                "[{}] Unsupported Home Assistant entity {}",
                self.name, entity_id
            )));
        };

        let ui_state = self.ui_state.lock().await;
        let ui_config = ui_state.config_normalized();
        let mut include =
            ui_config.should_include(&imported.entity_id, &imported.name, imported.available);
        if matches!(imported.kind, HassEntityKind::BinarySensor) {
            let detected_sensor_kind = imported
                .detected_sensor_kind
                .unwrap_or(HassSensorKind::Ignore);
            if matches!(
                ui_config.sensor_kind(&imported.entity_id, detected_sensor_kind),
                HassSensorKind::Ignore
            ) {
                include = false;
            }
        }
        drop(ui_state);

        if !include {
            // If user toggled to hidden quickly, do not import.
            self.ui_log(format!(
                "Skipped import of {} (not included by UI config)",
                imported.entity_id
            ))
            .await;
            return Ok(());
        }

        // Apply alias + sensor settings (UI config is source of truth).
        if let Some(alias) = ui_config.entity_alias(&imported.entity_id) {
            imported.name = alias;
        }
        if matches!(imported.kind, HassEntityKind::Switch) {
            imported.switch_mode = Some(ui_config.switch_mode(&imported.entity_id));
        }
        if matches!(imported.kind, HassEntityKind::Light)
            || (matches!(imported.kind, HassEntityKind::Switch)
                && imported.switch_mode == Some(HassSwitchMode::Light))
        {
            imported.light_archetype = Some(ui_config.light_archetype(&imported.entity_id));
        }
        if matches!(imported.kind, HassEntityKind::BinarySensor) {
            let detected = imported
                .detected_sensor_kind
                .unwrap_or(HassSensorKind::Ignore);
            imported.service_kind = match ui_config.sensor_kind(&imported.entity_id, detected) {
                HassSensorKind::Motion => HassServiceKind::Motion,
                HassSensorKind::Contact => HassServiceKind::Contact,
                HassSensorKind::Ignore => imported.service_kind,
            };
        }
        if matches!(
            imported.kind,
            HassEntityKind::BinarySensor | HassEntityKind::Sensor
        ) {
            imported.sensor_enabled = ui_config.sensor_enabled(&imported.entity_id);
        }

        let room_id = Self::assigned_room_id(&ui_config, &imported);

        let state = self.state.clone();
        let mut res = state.lock().await;
        self.ensure_rooms(&mut res, &ui_config)?;

        self.sync_single_entity(&imported, &mut res)?;

        // Move to selected room (remove from others first).
        let (device_link, _svc) = self.links_for_entity(&imported.entity_id, imported.service_kind);
        for room in self.room_map.values() {
            res.try_update::<Room>(&room.room_link.rid, |hue_room| {
                hue_room.children.remove(&device_link);
                Ok(())
            })?;
        }
        if let Some(target) = self.room_map.get(&room_id) {
            res.try_update::<Room>(&target.room_link.rid, |hue_room| {
                hue_room.children.insert(device_link);
                Ok(())
            })?;
        }

        self.ui_log(format!("Upserted {} into Hue bridge", imported.entity_id))
            .await;
        Ok(())
    }

    pub(super) async fn handle_state_update(&mut self, state: HassState) -> ApiResult<()> {
        // Realtime HA -> Hue sync: update only included entities without polling.
        if scene_import::parse(&state, None).is_some() {
            return self.sync_entities().await;
        }

        let ui_state = self.ui_state.lock().await;
        let ui_config = ui_state.config_normalized();
        drop(ui_state);

        let Some(mut imported) = parse_imported_entity(&state, None) else {
            return Ok(());
        };

        // HA websocket state_changed events can omit capability metadata like supported_color_modes.
        // Never downgrade a light to "on/off only" just because the incremental payload is sparse.
        if matches!(imported.kind, HassEntityKind::Light)
            && imported.capabilities == HassLightCapabilities::default()
        {
            if let Some(existing) = self.entity_map.get(&imported.entity_id) {
                imported.capabilities = existing.capabilities.clone();
            }
        }

        // Decide inclusion based on UI config (explicit visible overrides patterns/defaults).
        let mut include =
            ui_config.should_include(&imported.entity_id, &imported.name, imported.available);
        if matches!(imported.kind, HassEntityKind::BinarySensor) {
            let detected = imported
                .detected_sensor_kind
                .unwrap_or(HassSensorKind::Ignore);
            if matches!(
                ui_config.sensor_kind(&imported.entity_id, detected),
                HassSensorKind::Ignore
            ) {
                include = false;
            }
        }
        if !include {
            return Ok(());
        }

        if let Some(alias) = ui_config.entity_alias(&imported.entity_id) {
            imported.name = alias;
        }
        if matches!(imported.kind, HassEntityKind::Switch) {
            imported.switch_mode = Some(ui_config.switch_mode(&imported.entity_id));
        }
        if matches!(imported.kind, HassEntityKind::Light)
            || (matches!(imported.kind, HassEntityKind::Switch)
                && imported.switch_mode == Some(HassSwitchMode::Light))
        {
            imported.light_archetype = Some(ui_config.light_archetype(&imported.entity_id));
        }
        if matches!(imported.kind, HassEntityKind::BinarySensor) {
            let detected = imported
                .detected_sensor_kind
                .unwrap_or(HassSensorKind::Ignore);
            imported.service_kind = match ui_config.sensor_kind(&imported.entity_id, detected) {
                HassSensorKind::Motion => HassServiceKind::Motion,
                HassSensorKind::Contact => HassServiceKind::Contact,
                HassSensorKind::Ignore => imported.service_kind,
            };
        }
        if matches!(
            imported.kind,
            HassEntityKind::BinarySensor | HassEntityKind::Sensor
        ) {
            imported.sensor_enabled = ui_config.sensor_enabled(&imported.entity_id);
        }

        let state = self.state.clone();
        let mut res = state.lock().await;
        self.ensure_rooms(&mut res, &ui_config)?;
        self.sync_single_entity(&imported, &mut res)?;

        Ok(())
    }

    pub(super) async fn remove_entity_by_id(&mut self, entity_id: &str) -> ApiResult<()> {
        let device_link =
            RType::Device.deterministic(format!("hass:{}:{}:device", self.name, entity_id));

        {
            let mut res = self.state.lock().await;
            let _ = res.delete(&device_link);
        }

        if let Some(binding) = self.entity_map.remove(entity_id) {
            self.light_map.remove(&binding.service_link.rid);
            self.sensor_map.remove(&binding.service_link.rid);
            self.button_map.remove(&binding.service_link.rid);
            self.device_map.remove(&binding.device_link.rid);
        }

        if entity_id.starts_with("scene.") {
            let link = scene_import::link(&self.name, entity_id);
            let mut res = self.state.lock().await;
            let _ = res.delete(&link);
            self.imported_scene_map.remove(&link.rid);
            self.scene_map.remove(&link.rid);
        }

        self.ui_log(format!("Removed {} from Hue bridge", entity_id))
            .await;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{apply_light_state, parse_imported_entity};
    use crate::backend::hass::client::HassState;
    use crate::backend::hass::{HassEntityKind, HassServiceKind};
    use hue::api::{DeviceArchetype, Light, LightMetadata, RType};
    use serde_json::json;

    fn light_state(attributes: serde_json::Map<String, serde_json::Value>) -> HassState {
        HassState {
            entity_id: "light.test".to_string(),
            state: "on".to_string(),
            attributes,
        }
    }

    #[test]
    fn imports_modern_kelvin_color_temperature() {
        let attributes = json!({
            "friendly_name": "Test light",
            "supported_color_modes": ["color_temp"],
            "color_temp_kelvin": 2700
        })
        .as_object()
        .cloned()
        .expect("object attributes");

        let imported = parse_imported_entity(&light_state(attributes), None).expect("light");
        assert!(imported.capabilities.supports_color_temp);
        assert_eq!(imported.color_temp, Some(370));
    }

    #[test]
    fn keeps_legacy_mirek_color_temperature_as_fallback() {
        let attributes = json!({
            "friendly_name": "Test light",
            "supported_color_modes": ["color_temp"],
            "color_temp": 400
        })
        .as_object()
        .cloned()
        .expect("object attributes");

        let imported = parse_imported_entity(&light_state(attributes), None).expect("light");
        assert_eq!(imported.color_temp, Some(400));
    }

    #[test]
    fn imports_supported_numeric_sensor_without_turning_it_into_a_light() {
        let state = HassState {
            entity_id: "sensor.room_temperature".to_string(),
            state: "21.5".to_string(),
            attributes: json!({
                "friendly_name": "Room temperature",
                "device_class": "temperature",
                "unit_of_measurement": "°C"
            })
            .as_object()
            .cloned()
            .expect("object attributes"),
        };

        let imported = parse_imported_entity(&state, None).expect("numeric sensor");
        assert_eq!(imported.kind, HassEntityKind::Sensor);
        assert_eq!(imported.service_kind, HassServiceKind::Temperature);
        assert_eq!(imported.mapped_type(), "temperature");
        assert_eq!(imported.sensor_value, Some(21.5));
    }

    #[test]
    fn imports_event_entity_as_normalized_button_taxonomy() {
        let state = HassState {
            entity_id: "event.dimmer_button".to_string(),
            state: "2026-08-14T18:45:26.156+00:00".to_string(),
            attributes: json!({
                "friendly_name": "Dimmer button",
                "event_type": "short_release",
                "event_types": ["initial_press", "repeat", "short_release", "long_press", "long_release"]
            })
            .as_object()
            .cloned()
            .expect("object attributes"),
        };

        let imported = parse_imported_entity(&state, None).expect("event entity");

        assert_eq!(imported.kind, HassEntityKind::Event);
        assert_eq!(imported.event_name.as_deref(), Some("release"));
        assert_eq!(
            imported.event_values,
            Some(json!(["initial_press", "repeat", "release", "hold"]))
        );
    }

    #[test]
    fn ha_lights_do_not_advertise_unhandled_hue_controls() {
        let attributes = json!({
            "friendly_name": "On off light",
            "supported_color_modes": ["onoff"]
        })
        .as_object()
        .cloned()
        .expect("object attributes");
        let imported = parse_imported_entity(&light_state(attributes), None).expect("light");
        let mut light = Light::new(
            RType::Device.deterministic("test-device"),
            LightMetadata::new(DeviceArchetype::ClassicBulb, "On off light"),
        );
        light.powerup = None;

        apply_light_state(&mut light, &imported);

        assert!(light.alert.is_none());
        assert!(light.dimming.is_none());
        assert!(light.color.is_none());
        assert!(light.color_temperature.is_none());
        assert!(light.color_temperature_delta.is_none());
        assert!(light.dimming_delta.is_none());
        assert!(light.effects.is_none());
        assert!(light.effects_v2.is_none());
        assert!(light.gradient.is_none());
        assert!(light.powerup.is_none());
        assert!(light.signaling.is_none());
        assert!(light.timed_effects.is_none());
        assert!(light.service_id.is_none());
        assert_eq!(
            light
                .dynamics
                .expect("transitions are the only dynamics capability")
                .status_values,
            vec![hue::api::LightDynamicsStatus::None]
        );
    }

    #[test]
    fn preserves_local_powerup_during_home_assistant_state_sync() {
        let attributes = json!({
            "friendly_name": "On off light",
            "supported_color_modes": ["onoff"]
        })
        .as_object()
        .cloned()
        .expect("object attributes");
        let imported = parse_imported_entity(&light_state(attributes), None).expect("light");
        let mut light = Light::new(
            RType::Device.deterministic("test-device"),
            LightMetadata::new(DeviceArchetype::ClassicBulb, "On off light"),
        );

        apply_light_state(&mut light, &imported);

        assert!(light.powerup.is_some());
    }

    #[test]
    fn preserves_local_powerup_when_home_assistant_light_is_unavailable() {
        let state = HassState {
            entity_id: "light.test".to_string(),
            state: "unavailable".to_string(),
            attributes: json!({
                "friendly_name": "Test light",
                "supported_color_modes": ["onoff"]
            })
            .as_object()
            .cloned()
            .expect("object attributes"),
        };
        let imported = parse_imported_entity(&state, None).expect("light");
        let mut light = Light::new(
            RType::Device.deterministic("test-device"),
            LightMetadata::new(DeviceArchetype::ClassicBulb, "Test light"),
        );

        apply_light_state(&mut light, &imported);

        assert!(!imported.available);
        assert!(light.powerup.is_some());
    }
}
