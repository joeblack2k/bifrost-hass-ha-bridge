use std::sync::Arc;

use chrono::Utc;
use serde_json::{Map, Value, json};

use bifrost_api::backend::BackendRequest;
use hue::api::{
    GroupedLight, GroupedLightUpdate, LightUpdate, Motion, Resource, ResourceLink, Room,
    Scene, SceneActive, SceneStatus, SceneStatusEnum, SceneUpdate,
};

use crate::backend::hass::{
    HassBackend, HassEntityBinding, HassEntityKind, HassServiceKind,
};
use crate::error::ApiResult;

impl HassBackend {
    fn lookup_binding_by_light(&self, link: &ResourceLink) -> Option<HassEntityBinding> {
        let entity_id = self.light_map.get(&link.rid)?;
        self.entity_map.get(entity_id).cloned()
    }

    fn lookup_binding_by_sensor(&self, link: &ResourceLink) -> Option<HassEntityBinding> {
        let entity_id = self.sensor_map.get(&link.rid)?;
        self.entity_map.get(entity_id).cloned()
    }

    fn lookup_binding_by_device(&self, link: &ResourceLink) -> Option<HassEntityBinding> {
        let entity_id = self.device_map.get(&link.rid)?;
        self.entity_map.get(entity_id).cloned()
    }

    async fn backend_light_update(
        &self,
        binding: &HassEntityBinding,
        upd: &LightUpdate,
    ) -> ApiResult<()> {
        match binding.kind {
            HassEntityKind::Light => {
                if let Some(on) = upd.on {
                    if !on.on {
                        self.client
                            .call_service("light", "turn_off", &binding.entity_id, Map::new())
                            .await?;
                        return Ok(());
                    }
                }

                let mut data = Map::new();

                if binding.capabilities.supports_brightness {
                    if let Some(dim) = upd.dimming {
                        let bri_value = (dim.brightness * 255.0 / 100.0).round().clamp(0.0, 255.0);
                        let bri = format!("{bri_value:.0}")
                            .parse::<u16>()
                            .ok()
                            .map_or(0, |x| x.min(255));
                        data.insert("brightness".to_string(), json!(bri));
                    }
                }

                if binding.capabilities.supports_color_temp {
                    if let Some(ct) = upd.color_temperature.and_then(|ct| ct.mirek) {
                        data.insert("color_temp".to_string(), json!(ct));
                    }
                }

                if binding.capabilities.supports_color {
                    if let Some(color) = upd.color {
                        data.insert("xy_color".to_string(), json!([color.xy.x, color.xy.y]));
                    }
                }

                if let Some(duration_ms) = upd.dynamics.as_ref().and_then(|d| d.duration) {
                    data.insert(
                        "transition".to_string(),
                        Value::from(f64::from(duration_ms) / 1000.0),
                    );
                }

                if upd.on.is_some_and(|on| on.on) || !data.is_empty() {
                    self.client
                        .call_service("light", "turn_on", &binding.entity_id, data)
                        .await?;
                }
            }
            HassEntityKind::Switch => {
                if let Some(on) = upd.on {
                    let service = if on.on { "turn_on" } else { "turn_off" };
                    self.client
                        .call_service("switch", service, &binding.entity_id, Map::new())
                        .await?;
                }
            }
            HassEntityKind::BinarySensor => {}
        }

        Ok(())
    }

    async fn backend_sensor_enabled_update(
        &self,
        binding: &HassEntityBinding,
        enabled: bool,
    ) -> ApiResult<()> {
        {
            let mut lock = self.ui_state.lock().await;
            lock.set_entity_sensor_enabled(&binding.entity_id, enabled);
            let _ = lock.persist_and_log(&format!(
                "Sensor {} {}",
                binding.entity_id,
                if enabled { "enabled" } else { "disabled" }
            ));
        }

        let mut lock = self.state.lock().await;
        match binding.service_kind {
            HassServiceKind::Motion => {
                if lock.get::<Motion>(&binding.service_link).is_ok() {
                    lock.update::<Motion>(&binding.service_link.rid, |m| {
                        m.enabled = enabled;
                    })?;
                }
            }
            HassServiceKind::Contact => {
                if let Ok(contact_obj) = lock.get_resource(&binding.service_link) {
                    if let Resource::Contact(mut raw) = contact_obj.obj {
                        if let Some(map) = raw.as_object_mut() {
                            map.insert("enabled".to_string(), Value::Bool(enabled));
                        }
                        let _ = lock.delete(&binding.service_link);
                        lock.add(&binding.service_link, Resource::Contact(raw))?;
                    }
                }
            }
            HassServiceKind::Light | HassServiceKind::Switch => {}
        }
        drop(lock);

        if let Err(err) = self
            .client
            .set_entity_registry_disabled(&binding.entity_id, !enabled)
            .await
        {
            self.ui_log(format!(
                "HA entity registry update failed for {}: {}",
                binding.entity_id, err
            ))
            .await;
        }

        Ok(())
    }

    async fn backend_grouped_light_update(
        &self,
        link: &ResourceLink,
        upd: &GroupedLightUpdate,
    ) -> ApiResult<()> {
        let room = self.state.lock().await.get::<GroupedLight>(link)?.owner;
        let children = self
            .state
            .lock()
            .await
            .get::<Room>(&room)?
            .children
            .iter()
            .copied()
            .collect::<Vec<_>>();

        let light_upd = LightUpdate {
            on: upd.on,
            dimming: upd.dimming,
            color: upd.color,
            color_temperature: upd.color_temperature,
            dynamics: None,
            ..LightUpdate::default()
        };

        for child in children {
            if let Some(binding) = self.lookup_binding_by_device(&child) {
                if matches!(binding.kind, HassEntityKind::Light | HassEntityKind::Switch) {
                    self.backend_light_update(&binding, &light_upd).await?;
                }
            }
        }

        Ok(())
    }

    async fn backend_scene_create(
        &mut self,
        link_scene: &ResourceLink,
        sid: u32,
        scene: &Scene,
    ) -> ApiResult<()> {
        let mut lock = self.state.lock().await;
        lock.aux_set(link_scene, crate::model::state::AuxData::new().with_index(sid));
        lock.add(link_scene, Resource::Scene(scene.clone()))?;

        let snapshot_entities = lock
            .get::<Room>(&scene.group)
            .map(|room| {
                room.children
                    .iter()
                    .filter_map(|device| self.lookup_binding_by_device(device))
                    .filter(|binding| {
                        matches!(binding.kind, HassEntityKind::Light | HassEntityKind::Switch)
                    })
                    .map(|binding| binding.entity_id)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        drop(lock);

        let short = link_scene.rid.simple().to_string();
        let scene_id = format!("bifrost_{}", &short[..short.len().min(12)]);
        let ha_entity_id = format!("scene.{scene_id}");

        if snapshot_entities.is_empty() {
            self.ui_log(format!(
                "Skipped scene writeback for {} (empty room snapshot)",
                scene.metadata.name
            ))
            .await;
        } else if let Err(err) = self
            .client
            .create_scene_snapshot(&scene_id, &scene.metadata.name, snapshot_entities)
            .await
        {
            self.ui_log(format!(
                "Scene writeback failed for {}: {}",
                scene.metadata.name, err
            ))
            .await;
        } else {
            self.scene_map.insert(link_scene.rid, ha_entity_id);
        }

        Ok(())
    }

    async fn backend_scene_recall(&mut self, link: &ResourceLink) -> ApiResult<()> {
        if let Some(ha_scene) = self.scene_map.get(&link.rid) {
            self.client.turn_on_scene(ha_scene).await?;
            return Ok(());
        }

        let scene_actions = {
            let lock = self.state.lock().await;
            lock.get::<Scene>(link)?.actions.clone()
        };

        for action in scene_actions {
            if let Some(binding) = self.lookup_binding_by_light(&action.target) {
                let upd = LightUpdate {
                    on: action.action.on,
                    dimming: action.action.dimming,
                    color: action.action.color,
                    color_temperature: action.action.color_temperature,
                    dynamics: None,
                    ..LightUpdate::default()
                };
                self.backend_light_update(&binding, &upd).await?;
            }
        }

        Ok(())
    }

    async fn backend_scene_update(
        &mut self,
        link: &ResourceLink,
        upd: &SceneUpdate,
    ) -> ApiResult<()> {
        {
            let mut lock = self.state.lock().await;
            lock.update::<Scene>(&link.rid, |scene| {
                *scene += upd;
                if let Some(recall) = &upd.recall {
                    if matches!(
                        recall.action,
                        Some(SceneStatusEnum::Active) | Some(SceneStatusEnum::Static)
                    ) {
                        scene.status = Some(SceneStatus {
                            active: SceneActive::Static,
                            last_recall: Some(Utc::now()),
                        });
                    }
                }
            })?;
        }

        if let Some(recall) = &upd.recall {
            if matches!(
                recall.action,
                Some(SceneStatusEnum::Active) | Some(SceneStatusEnum::Static)
            ) {
                self.backend_scene_recall(link).await?;
                return Ok(());
            }
        }

        Ok(())
    }

    pub(super) async fn handle_backend_event(&mut self, req: Arc<BackendRequest>) -> ApiResult<()> {
        match &*req {
            BackendRequest::LightUpdate(link, upd) => {
                if let Some(binding) = self.lookup_binding_by_light(link) {
                    self.backend_light_update(&binding, upd).await?;
                }
            }
            BackendRequest::SensorEnabledUpdate(link, enabled) => {
                if let Some(binding) = self.lookup_binding_by_sensor(link) {
                    self.backend_sensor_enabled_update(&binding, *enabled).await?;
                }
            }
            BackendRequest::HassSync => {
                let _ = self.run_sync("manual").await;
            }
            BackendRequest::HassUpsertEntity(entity_id) => {
                let _ = self.sync_entity_by_id(entity_id).await;
            }
            BackendRequest::HassRemoveEntity(entity_id) => {
                let _ = self.remove_entity_by_id(entity_id).await;
            }
            BackendRequest::HassUpdateRooms => {
                let _ = self.refresh_rooms_from_ui_config().await;
            }
            BackendRequest::HassConnect => {
                {
                    let mut rt = self.runtime_state.lock().await;
                    rt.config.enabled = true;
                    let _ = rt.save();
                }
                self.ws = None;
                let _ = self.run_sync("connect").await;
            }
            BackendRequest::HassDisconnect => {
                {
                    let mut rt = self.runtime_state.lock().await;
                    rt.config.enabled = false;
                    let _ = rt.save();
                }
                self.ws = None;
                self.ui_log("Home Assistant backend disconnected by user").await;
            }
            BackendRequest::GroupedLightUpdate(link, upd) => {
                self.backend_grouped_light_update(link, upd).await?;
            }
            BackendRequest::SceneCreate(link, sid, scene) => {
                self.backend_scene_create(link, *sid, scene).await?;
            }
            BackendRequest::SceneUpdate(link, upd) => {
                self.backend_scene_update(link, upd).await?;
            }

            BackendRequest::RoomUpdate(_, _)
            | BackendRequest::Delete(_)
            | BackendRequest::EntertainmentStart(_)
            | BackendRequest::EntertainmentFrame(_)
            | BackendRequest::EntertainmentStop()
            | BackendRequest::ZigbeeDeviceDiscovery(_, _) => {}
        }

        Ok(())
    }
}
