use std::collections::{BTreeSet, HashMap, HashSet};
use std::sync::Arc;

use chrono::Utc;
use maplit::btreeset;
use serde_json::{Value, json};

use hue::api::{
    ColorTemperature, Device, DeviceArchetype, DeviceProductData, Dimming, DimmingUpdate,
    GroupedLight, Light, LightColor, LightMetadata, Metadata, MirekSchema, Motion, On, RType,
    Resource, ResourceLink, Room, RoomArchetype, RoomMetadata, ZigbeeConnectivity,
    ZigbeeConnectivityStatus,
};
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
}

impl ImportedEntity {
    fn domain(&self) -> &'static str {
        match self.kind {
            HassEntityKind::Light => "light",
            HassEntityKind::Switch => "switch",
            HassEntityKind::BinarySensor => "binary_sensor",
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
    let has_xy_attr = state.attributes.contains_key("xy_color");

    let supports_color = modes
        .iter()
        .any(|m| matches!(m.as_str(), "xy" | "hs" | "rgb" | "rgbw" | "rgbww"));
    let supports_color_temp = modes.contains("color_temp") || has_color_temp_attr;
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
            .get("color_temp")
            .and_then(value_to_u16)
            .map(|x| x.clamp(153, 500))
    } else {
        None
    };

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
        HassEntityKind::BinarySensor => DeviceArchetype::UnknownArchetype,
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
        HassEntityKind::Switch | HassEntityKind::BinarySensor => {
            light.dimming = None;
            light.color = None;
            light.color_temperature = None;
            light.color_temperature_delta = None;
        }
    }
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
                capabilities: imported.capabilities,
                switch_mode: imported.switch_mode,
            });

        let previous_service_link = binding.service_link;
        binding.name.clone_from(&imported.name);
        binding.kind = imported.kind;
        binding.service_kind = imported.service_kind;
        binding.service_link = service_link;
        binding.device_link = device_link;
        binding.capabilities = imported.capabilities;
        binding.switch_mode = imported.switch_mode;

        if previous_service_link != binding.service_link {
            self.light_map.remove(&previous_service_link.rid);
            self.sensor_map.remove(&previous_service_link.rid);
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
            HassServiceKind::Motion | HassServiceKind::Contact => {
                self.sensor_map
                    .insert(binding.service_link.rid, imported.entity_id.clone());
                self.light_map.remove(&binding.service_link.rid);
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
                    HassEntityKind::BinarySensor => false,
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
                imported.sensor_enabled = ui_config.sensor_enabled(&imported.entity_id);
            }

            let hidden = ui_config.is_manually_hidden(&imported.entity_id);
            let room_id = Self::assigned_room_id(&ui_config, &imported);
            let room_name = ui_config.room_name(&room_id);
            let selected_sensor_kind = match imported.service_kind {
                HassServiceKind::Motion => Some(HassSensorKind::Motion),
                HassServiceKind::Contact => Some(HassSensorKind::Contact),
                HassServiceKind::Light | HassServiceKind::Switch => None,
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
        // any Home Assistant-generated devices that are no longer included.
        let keep_device_rids = imported_included
            .values()
            .map(|imported| {
                let (device_link, _service_link) =
                    self.links_for_entity(&imported.entity_id, imported.service_kind);
                device_link.rid
            })
            .collect::<HashSet<_>>();
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
                self.device_map.remove(&binding.device_link.rid);
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
                imported.capabilities = existing.capabilities;
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
            self.device_map.remove(&binding.device_link.rid);
        }

        self.ui_log(format!("Removed {} from Hue bridge", entity_id))
            .await;
        Ok(())
    }
}
