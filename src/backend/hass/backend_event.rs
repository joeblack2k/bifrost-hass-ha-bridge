use std::collections::HashSet;
use std::sync::Arc;
use std::time::Instant;

use chrono::Utc;
use serde_json::{Map, Value, json};

use bifrost_api::backend::BackendRequest;
use hue::api::{
    GroupedLight, GroupedLightUpdate, LightUpdate, Motion, RType, Resource, ResourceLink, Room,
    Scene, SceneActive, SceneStatus, SceneStatusEnum, SceneUpdate,
};
use hue::colortemp::mirek_to_kelvin;

use crate::backend::hass::{HassBackend, HassEntityBinding, HassEntityKind, HassServiceKind};
use crate::error::ApiResult;
use crate::model::hass::{HassSwitchMode, HassUiConfig};

use super::{events, light_projection, scene_import};

fn light_service_calls(
    requested_off: bool,
    requested_on: bool,
    data: Map<String, Value>,
) -> Vec<(&'static str, Map<String, Value>)> {
    if requested_off {
        let mut calls = vec![("turn_off", Map::new())];
        if !data.is_empty() {
            calls.push(("turn_on", data));
            // Home Assistant's turn_on service also powers on a light when it only carries
            // brightness, effect or identify data. Restore Hue's requested final off state.
            calls.push(("turn_off", Map::new()));
        }
        return calls;
    }

    if requested_on || !data.is_empty() {
        vec![("turn_on", data)]
    } else {
        Vec::new()
    }
}

impl HassBackend {
    fn room_id_for_link(&self, link: &ResourceLink) -> Option<String> {
        self.room_map
            .values()
            .find(|binding| binding.room_link == *link)
            .map(|binding| binding.room_id.clone())
    }

    fn is_hass_room_link(&self, link: &ResourceLink) -> bool {
        self.room_id_for_link(link).is_some()
    }

    fn scene_id(link: &ResourceLink) -> String {
        let short = link.rid.simple().to_string();
        format!("bifrost_{}", &short[..short.len().min(12)])
    }

    fn scene_entity_id(link: &ResourceLink) -> String {
        format!("scene.{}", Self::scene_id(link))
    }

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
                        if let Some(kelvin) = mirek_to_kelvin(ct) {
                            data.insert("color_temp_kelvin".to_string(), json!(kelvin));
                        }
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

                light_projection::append_update_data(&mut data, upd, &binding.capabilities);

                let requested_off = upd.on.is_some_and(|on| !on.on);
                let requested_on = upd.on.is_some_and(|on| on.on);
                for (service, service_data) in
                    light_service_calls(requested_off, requested_on, data)
                {
                    self.client
                        .call_service("light", service, &binding.entity_id, service_data)
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
            HassEntityKind::BinarySensor | HassEntityKind::Sensor | HassEntityKind::Event => {}
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
            HassServiceKind::Light
            | HassServiceKind::Switch
            | HassServiceKind::Temperature
            | HassServiceKind::LightLevel
            | HassServiceKind::Button => {}
        }
        drop(lock);

        if matches!(
            binding.service_kind,
            HassServiceKind::Motion | HassServiceKind::Contact
        ) {
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
                let grouped_as_light = match binding.kind {
                    HassEntityKind::Light => true,
                    HassEntityKind::Switch => {
                        binding.switch_mode.unwrap_or(HassSwitchMode::Plug) == HassSwitchMode::Light
                    }
                    HassEntityKind::BinarySensor
                    | HassEntityKind::Sensor
                    | HassEntityKind::Event => false,
                };
                if grouped_as_light {
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
        lock.aux_set(
            link_scene,
            crate::model::state::AuxData::new().with_index(sid),
        );
        lock.add(link_scene, Resource::Scene(scene.clone()))?;

        let snapshot_entities = lock
            .get::<Room>(&scene.group)
            .map(|room| {
                room.children
                    .iter()
                    .filter_map(|device| self.lookup_binding_by_device(device))
                    .filter(|binding| match binding.kind {
                        HassEntityKind::Light => true,
                        HassEntityKind::Switch => {
                            binding.switch_mode.unwrap_or(HassSwitchMode::Plug)
                                == HassSwitchMode::Light
                        }
                        HassEntityKind::BinarySensor
                        | HassEntityKind::Sensor
                        | HassEntityKind::Event => false,
                    })
                    .map(|binding| binding.entity_id)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        drop(lock);

        let scene_id = Self::scene_id(link_scene);
        let ha_entity_id = Self::scene_entity_id(link_scene);

        if snapshot_entities.is_empty() {
            self.ui_log(format!(
                "Skipped scene writeback for {} (empty room snapshot)",
                scene.metadata.name
            ))
            .await;
        } else if let Err(err) = self
            .client
            .create_scene_snapshot(&scene_id, snapshot_entities)
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

    async fn backend_room_update(
        &mut self,
        link: &ResourceLink,
        upd: &hue::api::RoomUpdate,
    ) -> ApiResult<()> {
        let Some(room_id) = self.room_id_for_link(link) else {
            return Ok(());
        };

        let (old_children, room) = {
            let mut lock = self.state.lock().await;
            let old_children = lock.get::<Room>(link)?.children.clone();
            lock.update::<Room>(&link.rid, |room| {
                *room += upd;
            })?;
            let room = lock.get::<Room>(link)?.clone();
            drop(lock);
            (old_children, room)
        };

        {
            let mut ui = self.ui_state.lock().await;
            ui.rename_room(&room_id, &room.metadata.name);

            if let Some(children) = &upd.children {
                for binding in self.entity_map.values() {
                    let was_member = old_children.contains(&binding.device_link);
                    let is_member = children.contains(&binding.device_link);
                    if was_member != is_member {
                        ui.set_entity_room(&binding.entity_id, is_member.then(|| room_id.clone()));
                    }
                }
            }

            ui.persist_and_log(&format!("Updated Hue room {}", room.metadata.name))?;
            drop(ui);
        }

        self.refresh_rooms_from_ui_config().await
    }

    async fn backend_scene_delete(&mut self, link: &ResourceLink) -> ApiResult<()> {
        let scene = {
            let lock = self.state.lock().await;
            lock.get::<Scene>(link).ok().cloned()
        };
        let imported = self.imported_scene_map.remove(&link.rid).is_some()
            || scene.as_ref().is_some_and(scene_import::is_imported);
        if imported {
            if let Some(entity_id) = scene.as_ref().and_then(scene_import::imported_entity_id) {
                let mut ui = self.ui_state.lock().await;
                ui.set_entity_visibility(entity_id, true);
                ui.persist_and_log(&format!("Hidden imported Home Assistant scene {entity_id}"))?;
            }
            self.scene_map.remove(&link.rid);
            let mut lock = self.state.lock().await;
            return lock.delete(link);
        }

        let Some(scene) = scene else {
            self.scene_map.remove(&link.rid);
            return Ok(());
        };

        if !self.is_hass_room_link(&scene.group) {
            return Ok(());
        }

        // Hue scenes persist across a Bifrost restart while scene_map is in-memory only. The
        // deterministic HA entity id is therefore the deletion authority.
        if let Err(err) = self
            .client
            .delete_scene_snapshot(&Self::scene_entity_id(link))
            .await
        {
            self.ui_log(format!(
                "HA scene snapshot delete failed for {}: {}",
                scene.metadata.name, err
            ))
            .await;
        }

        self.scene_map.remove(&link.rid);
        let mut lock = self.state.lock().await;
        lock.delete(link)
    }

    async fn backend_room_delete(&mut self, link: &ResourceLink) -> ApiResult<()> {
        let Some(room_id) = self.room_id_for_link(link) else {
            return Ok(());
        };
        if room_id == HassUiConfig::DEFAULT_ROOM_ID {
            self.ui_log("Ignoring delete of the default Home Assistant room")
                .await;
            return Ok(());
        }

        let scene_links = {
            let lock = self.state.lock().await;
            lock.get_scenes_for_room(&link.rid)
                .into_iter()
                .map(|rid| RType::Scene.link_to(rid))
                .collect::<Vec<_>>()
        };
        let imported_scene_rids = {
            let lock = self.state.lock().await;
            scene_links
                .iter()
                .filter(|scene_link| {
                    self.imported_scene_map.contains_key(&scene_link.rid)
                        || lock
                            .get::<Scene>(scene_link)
                            .ok()
                            .is_some_and(scene_import::is_imported)
                })
                .map(|scene_link| scene_link.rid)
                .collect::<HashSet<_>>()
        };
        {
            let mut ui = self.ui_state.lock().await;
            ui.remove_room(&room_id);
            ui.persist_and_log(&format!("Removed Hue room {room_id}"))?;
            drop(ui);
        }

        for scene_link in &scene_links {
            if imported_scene_rids.contains(&scene_link.rid) {
                continue;
            }
            if let Err(err) = self
                .client
                .delete_scene_snapshot(&Self::scene_entity_id(scene_link))
                .await
            {
                self.ui_log(format!(
                    "HA scene snapshot delete failed during room removal: {err}"
                ))
                .await;
            }
        }

        let imported_scene_entities = {
            let lock = self.state.lock().await;
            scene_links
                .iter()
                .filter(|scene_link| imported_scene_rids.contains(&scene_link.rid))
                .filter_map(|scene_link| {
                    lock.get::<Scene>(scene_link)
                        .ok()
                        .and_then(scene_import::imported_entity_id)
                        .map(ToOwned::to_owned)
                })
                .collect::<Vec<_>>()
        };
        if !imported_scene_entities.is_empty() {
            let mut ui = self.ui_state.lock().await;
            for entity_id in imported_scene_entities {
                ui.set_entity_visibility(&entity_id, true);
                ui.persist_and_log(&format!("Hidden imported Home Assistant scene {entity_id}"))?;
            }
        }
        {
            let mut lock = self.state.lock().await;
            for scene_link in &scene_links {
                let _ = lock.delete(scene_link);
            }
        }
        for scene_link in scene_links {
            self.imported_scene_map.remove(&scene_link.rid);
            self.scene_map.remove(&scene_link.rid);
        }

        self.refresh_rooms_from_ui_config().await
    }

    async fn backend_device_delete(&mut self, link: &ResourceLink) -> ApiResult<()> {
        let Some(entity_id) = self.device_map.get(&link.rid).cloned() else {
            return Ok(());
        };

        {
            let mut ui = self.ui_state.lock().await;
            ui.set_entity_visibility(&entity_id, true);
            ui.persist_and_log(&format!("Hidden Home Assistant entity {entity_id}"))?;
            drop(ui);
        }

        self.remove_entity_by_id(&entity_id).await
    }

    async fn backend_delete(&mut self, link: &ResourceLink) -> ApiResult<()> {
        match link.rtype {
            RType::Room => self.backend_room_delete(link).await,
            RType::Scene => self.backend_scene_delete(link).await,
            RType::Device => self.backend_device_delete(link).await,
            _ => Ok(()),
        }
    }

    async fn backend_scene_recall(&self, link: &ResourceLink) -> ApiResult<()> {
        if let Some(ha_scene) = self.scene_map.get(&link.rid) {
            self.client.turn_on_scene(ha_scene).await?;
            return Ok(());
        }

        let persisted_ha_scene = {
            let lock = self.state.lock().await;
            lock.get::<Scene>(link)
                .ok()
                .and_then(scene_import::imported_entity_id)
                .map(ToOwned::to_owned)
        };
        if let Some(ha_scene) = persisted_ha_scene {
            self.client.turn_on_scene(&ha_scene).await?;
            return Ok(());
        }

        let scene_actions = {
            let lock = self.state.lock().await;
            lock.get::<Scene>(link)?.actions.clone()
        };

        for action in scene_actions {
            if let Some(binding) = self.lookup_binding_by_light(&action.target) {
                if matches!(binding.kind, HassEntityKind::Switch)
                    && binding.switch_mode.unwrap_or(HassSwitchMode::Plug) != HassSwitchMode::Light
                {
                    continue;
                }
                let upd = LightUpdate {
                    on: action.action.on,
                    dimming: action.action.dimming,
                    color: action.action.color,
                    color_temperature: action.action.color_temperature,
                    gradient: action.action.gradient,
                    dynamics: None,
                    ..LightUpdate::default()
                };
                self.backend_light_update(&binding, &upd).await?;
            }
        }

        Ok(())
    }

    async fn backend_scene_update(&self, link: &ResourceLink, upd: &SceneUpdate) -> ApiResult<()> {
        {
            let mut lock = self.state.lock().await;
            lock.update::<Scene>(&link.rid, |scene| {
                *scene += upd;
                if let Some(recall) = &upd.recall {
                    if matches!(
                        recall.action,
                        Some(SceneStatusEnum::Active | SceneStatusEnum::Static)
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
                Some(SceneStatusEnum::Active | SceneStatusEnum::Static)
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
                    self.backend_sensor_enabled_update(&binding, *enabled)
                        .await?;
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
                self.ws_needs_sync = self.run_sync("connect").await.is_err();
                self.ws_backoff_secs = 1;
                self.ws_retry_at = Instant::now();
            }
            BackendRequest::HassDisconnect => {
                {
                    let mut rt = self.runtime_state.lock().await;
                    rt.config.enabled = false;
                    let _ = rt.save();
                }
                self.ws = None;
                self.ws_needs_sync = false;
                self.ws_backoff_secs = 1;
                self.ws_retry_at = Instant::now();
                self.ui_log("Home Assistant backend disconnected by user")
                    .await;
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

            BackendRequest::RoomUpdate(link, upd) => {
                self.backend_room_update(link, upd).await?;
            }
            BackendRequest::Delete(link) => {
                self.backend_delete(link).await?;
            }
            BackendRequest::EntertainmentStart(_)
            | BackendRequest::EntertainmentFrame(_)
            | BackendRequest::EntertainmentStop()
            | BackendRequest::ZigbeeDeviceDiscovery(_, _) => {}
        }

        Ok(())
    }

    pub(super) async fn handle_generic_event(
        &mut self,
        event_type: &str,
        data: &Value,
    ) -> ApiResult<()> {
        let kind = events::classify(event_type, data);
        if matches!(
            kind,
            events::HassEventKind::Unknown | events::HassEventKind::StateChanged
        ) {
            return Ok(());
        }

        if let Some(entity_id) = events::entity_id(data) {
            if entity_id.starts_with("event.") {
                let _ = self.sync_entity_by_id(entity_id).await;
            }
        }

        if let Some(name) = events::normalized_event_name(data) {
            self.ui_log(format!("Accessory event {event_type}: {name}"))
                .await;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::light_service_calls;
    use serde_json::json;

    #[test]
    fn combined_off_update_restores_final_off_state() {
        let calls = light_service_calls(
            true,
            false,
            [
                ("brightness".to_string(), json!(128)),
                ("flash".to_string(), json!("short")),
            ]
            .into_iter()
            .collect(),
        );
        assert_eq!(calls.len(), 3);
        assert_eq!(calls[0].0, "turn_off");
        assert_eq!(calls[1].0, "turn_on");
        assert_eq!(calls[2].0, "turn_off");
        assert_eq!(calls[1].1.get("brightness"), Some(&json!(128)));
    }

    #[test]
    fn plain_off_does_not_emit_a_follow_up_turn_on() {
        let calls = light_service_calls(true, false, Default::default());
        assert_eq!(
            calls
                .iter()
                .map(|(service, _)| *service)
                .collect::<Vec<_>>(),
            ["turn_off"]
        );
    }
}
