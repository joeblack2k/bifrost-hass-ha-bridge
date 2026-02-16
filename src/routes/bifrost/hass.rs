use std::collections::HashSet;
use std::path::Path;
use std::time::Duration;

use axum::Router;
use axum::extract::{Request, State};
use axum::http::header;
use axum::middleware::{self, Next};
use axum::response::Response;
use axum::routing::{get, post, put};
use bifrost_api::backend::BackendRequest;
use hue::api::{Device, RType};
use tower_http::services::{ServeDir, ServeFile};

use crate::model::hass::{
    HassApplyResponse, HassBridgeInfo, HassConnectResponse, HassEntitiesResponse,
    HassEntityPatchRequest, HassLinkButtonResponse, HassLogsResponse, HassPatinaEventRequest,
    HassPatinaPublic, HassResetBridgeResponse, HassRoomCreateRequest, HassRoomDeleteRequest,
    HassRoomRenameRequest, HassRoomsResponse, HassRuntimeConfigPublic, HassRuntimeConfigUpdate,
    HassSensorKind, HassSwitchMode, HassSyncResponse, HassTokenRequest, HassUiConfig,
    HassUiPayload,
};
use crate::routes::bifrost::BifrostApiResult;
use crate::routes::extractor::Json;
use crate::server::appstate::AppState;

const LINKBUTTON_DURATION_SECS: u64 = 30;

fn resolve_ui_dir() -> String {
    if let Ok(path) = std::env::var("BIFROST_UI_DIR") {
        if Path::new(&path).is_dir() {
            return path;
        }
    }

    let docker_path = "/app/ui";
    if Path::new(docker_path).is_dir() {
        return docker_path.to_string();
    }

    let dev_path = "ui/dist";
    if Path::new(dev_path).is_dir() {
        return dev_path.to_string();
    }

    docker_path.to_string()
}

fn looks_like_asset(path: &str) -> bool {
    path.rsplit('/')
        .next()
        .is_some_and(|part| part.contains('.'))
}

async fn ui_cache_control(req: Request, next: Next) -> Response {
    let path = req.uri().path().to_string();
    let mut res = next.run(req).await;

    let cache = if looks_like_asset(&path) {
        "public, max-age=31536000, immutable"
    } else {
        "no-store"
    };

    if let Ok(value) = header::HeaderValue::from_str(cache) {
        res.headers_mut().insert(header::CACHE_CONTROL, value);
    }

    res
}

fn ui_router() -> Router<AppState> {
    let ui_dir = resolve_ui_dir();
    let index_file = format!("{ui_dir}/index.html");
    let ui_assets = ServeDir::new(ui_dir).fallback(ServeFile::new(index_file.clone()));

    Router::new()
        .nest_service("/ui", ui_assets)
        .layer(middleware::from_fn(ui_cache_control))
}

async fn get_ui_payload(State(state): State<AppState>) -> BifrostApiResult<Json<HassUiPayload>> {
    let ui = state.hass_ui();
    let payload = ui.lock().await.payload();
    Ok(Json(payload))
}

async fn get_ui_config(State(state): State<AppState>) -> BifrostApiResult<Json<HassUiConfig>> {
    let ui = state.hass_ui();
    let config = ui.lock().await.config_normalized();
    Ok(Json(config))
}

async fn put_ui_config(
    State(state): State<AppState>,
    Json(config): Json<HassUiConfig>,
) -> BifrostApiResult<Json<HassUiConfig>> {
    let ui = state.hass_ui();
    let mut lock = ui.lock().await;
    lock.set_config(config);
    lock.persist_and_log("Saved web UI configuration")?;
    let normalized = lock.config_normalized();
    drop(lock);

    // Keep room metadata in Hue resources aligned with UI config updates.
    {
        let res = state.res.lock().await;
        res.backend_request(BackendRequest::HassUpdateRooms)?;
    }

    Ok(Json(normalized))
}

async fn get_entities(
    State(state): State<AppState>,
) -> BifrostApiResult<Json<HassEntitiesResponse>> {
    let ui = state.hass_ui();
    let entities = ui.lock().await.entities.clone();
    Ok(Json(HassEntitiesResponse { entities }))
}

async fn patch_entity(
    State(state): State<AppState>,
    Json(req): Json<HassEntityPatchRequest>,
) -> BifrostApiResult<Json<HassUiConfig>> {
    let ui = state.hass_ui();
    let mut lock = ui.lock().await;
    let mut trigger_upsert = false;
    let mut trigger_remove = false;

    if let Some(hidden) = req.hidden {
        lock.set_entity_visibility(&req.entity_id, hidden);
        if hidden {
            trigger_remove = true;
        } else {
            trigger_upsert = true;
        }
    }
    if let Some(room_id) = req.room_id {
        let room_id = room_id.trim();
        let room_id = if room_id.is_empty() {
            None
        } else {
            Some(room_id.to_string())
        };
        lock.set_entity_room(&req.entity_id, room_id);
        trigger_upsert = true;
    }
    if let Some(alias) = req.alias {
        let alias = alias.trim();
        let alias = if alias.is_empty() {
            None
        } else {
            Some(alias.to_string())
        };
        lock.set_entity_alias(&req.entity_id, alias);
        trigger_upsert = true;
    }
    if let Some(kind) = req.sensor_kind {
        lock.set_entity_sensor_kind(&req.entity_id, Some(kind));
        trigger_upsert = true;
    }
    if let Some(enabled) = req.enabled {
        lock.set_entity_sensor_enabled(&req.entity_id, enabled);
        trigger_upsert = true;
    }
    if let Some(mode) = req.switch_mode {
        lock.set_entity_switch_mode(&req.entity_id, Some(mode));
        trigger_upsert = true;
    }
    if let Some(archetype) = req.light_archetype {
        lock.set_entity_light_archetype(&req.entity_id, Some(archetype));
        trigger_upsert = true;
    }

    lock.persist_and_log(&format!("Updated entity {}", req.entity_id))?;
    let cfg = lock.config_normalized();

    if let Some(summary) = lock
        .entities
        .iter_mut()
        .find(|ent| ent.entity_id == req.entity_id)
    {
        if let Some(alias) = cfg.entity_alias(&summary.entity_id) {
            summary.name = alias;
        }
        if let Some(room_id) = cfg
            .entity_preferences
            .get(&summary.entity_id)
            .and_then(|pref| pref.room_id.clone())
            .filter(|room_id| cfg.rooms.iter().any(|room| room.id == *room_id))
        {
            summary.room_name = cfg.room_name(&room_id);
            summary.room_id = room_id;
        }
        summary.hidden = cfg.is_manually_hidden(&summary.entity_id);
        let mut included = cfg.should_include(&summary.entity_id, &summary.name, summary.available);
        if summary.domain == "binary_sensor" {
            let detected = summary.sensor_kind.unwrap_or(HassSensorKind::Ignore);
            let selected = cfg.sensor_kind(&summary.entity_id, detected);
            summary.sensor_kind = Some(selected);
            summary.enabled = cfg.sensor_enabled(&summary.entity_id);
            if matches!(selected, HassSensorKind::Ignore) {
                included = false;
            }
            summary.light_archetype = None;
        } else if summary.domain == "light" {
            summary.light_archetype = Some(cfg.light_archetype(&summary.entity_id));
        } else if summary.domain == "switch" {
            let mode = cfg.switch_mode(&summary.entity_id);
            summary.switch_mode = Some(mode);
            summary.mapped_type = if mode == HassSwitchMode::Light {
                "light".to_string()
            } else {
                "switch".to_string()
            };
            summary.light_archetype = if mode == HassSwitchMode::Light {
                Some(cfg.light_archetype(&summary.entity_id))
            } else {
                None
            };
        }
        summary.included = included;
    }
    drop(lock);

    // Apply immediately so the Hue app updates without requiring manual save/sync.
    if trigger_remove {
        let res = state.res.lock().await;
        res.backend_request(BackendRequest::HassRemoveEntity(req.entity_id.clone()))?;
    } else if trigger_upsert {
        let res = state.res.lock().await;
        res.backend_request(BackendRequest::HassUpsertEntity(req.entity_id.clone()))?;
    }

    Ok(Json(cfg))
}

async fn get_rooms(State(state): State<AppState>) -> BifrostApiResult<Json<HassRoomsResponse>> {
    let ui = state.hass_ui();
    let rooms = ui.lock().await.config_normalized().rooms;
    Ok(Json(HassRoomsResponse { rooms }))
}

async fn post_room(
    State(state): State<AppState>,
    Json(req): Json<HassRoomCreateRequest>,
) -> BifrostApiResult<Json<HassRoomsResponse>> {
    let ui = state.hass_ui();
    let mut lock = ui.lock().await;
    let _created = lock.add_room(&req.name);
    lock.persist_and_log(&format!("Added room {}", req.name))?;
    let response = HassRoomsResponse {
        rooms: lock.config_normalized().rooms,
    };
    drop(lock);

    {
        let res = state.res.lock().await;
        res.backend_request(BackendRequest::HassUpdateRooms)?;
    }

    Ok(Json(response))
}

async fn put_room(
    State(state): State<AppState>,
    Json(req): Json<HassRoomRenameRequest>,
) -> BifrostApiResult<Json<HassRoomsResponse>> {
    let ui = state.hass_ui();
    let mut lock = ui.lock().await;
    lock.rename_room(&req.room_id, &req.name);
    lock.persist_and_log(&format!(
        "Renamed room {} to {}",
        req.room_id,
        req.name.trim()
    ))?;
    let response = HassRoomsResponse {
        rooms: lock.config_normalized().rooms,
    };
    drop(lock);

    {
        let res = state.res.lock().await;
        res.backend_request(BackendRequest::HassUpdateRooms)?;
    }

    Ok(Json(response))
}

async fn delete_room(
    State(state): State<AppState>,
    Json(req): Json<HassRoomDeleteRequest>,
) -> BifrostApiResult<Json<HassRoomsResponse>> {
    let ui = state.hass_ui();
    let mut lock = ui.lock().await;
    lock.remove_room(&req.room_id);
    lock.persist_and_log(&format!("Removed room {}", req.room_id))?;
    let response = HassRoomsResponse {
        rooms: lock.config_normalized().rooms,
    };
    drop(lock);

    {
        let res = state.res.lock().await;
        res.backend_request(BackendRequest::HassUpdateRooms)?;
    }

    Ok(Json(response))
}

async fn get_logs(State(state): State<AppState>) -> BifrostApiResult<Json<HassLogsResponse>> {
    let ui = state.hass_ui();
    let logs = ui.lock().await.visible_logs();
    Ok(Json(HassLogsResponse { logs }))
}

async fn get_bridge_info(State(state): State<AppState>) -> BifrostApiResult<Json<HassBridgeInfo>> {
    let conf = state.config();
    let bridge_id = hue::bridge_id(conf.bridge.mac);
    let software_version = state
        .updater()
        .lock()
        .await
        .get()
        .await
        .get_software_version();
    let linkbutton_active = state.linkbutton_active().await;

    let (
        total_entities,
        included_entities,
        hidden_entities,
        room_count,
        defaults,
        sync_areas,
        sync_status,
        ui_timezone,
        hass_lat,
        hass_long,
    ) = {
        let ui = state.hass_ui();
        let lock = ui.lock().await;
        let cfg = lock.config_normalized();
        let total = lock.entities.len();
        let included = lock
            .entities
            .iter()
            .filter(|ent| {
                let mut include = cfg.should_include(&ent.entity_id, &ent.name, ent.available);
                if ent.domain == "binary_sensor" {
                    let detected = ent.sensor_kind.unwrap_or(HassSensorKind::Ignore);
                    if matches!(
                        cfg.sensor_kind(&ent.entity_id, detected),
                        HassSensorKind::Ignore
                    ) {
                        include = false;
                    }
                }
                include
            })
            .count();
        let hidden = total.saturating_sub(included);
        let room_count = cfg.rooms.len();
        let defaults = cfg.default_add_new_devices_to_hue;
        let sync_areas = cfg.sync_hass_areas_to_rooms;
        (
            total,
            included,
            hidden,
            room_count,
            defaults,
            sync_areas,
            lock.sync.clone(),
            cfg.hass_timezone,
            cfg.hass_lat,
            cfg.hass_long,
        )
    };

    Ok(Json(HassBridgeInfo {
        bridge_name: conf.bridge.name.clone(),
        bridge_id,
        software_version,
        mac: conf.bridge.mac.to_string(),
        ipaddress: conf.bridge.ipaddress.to_string(),
        netmask: conf.bridge.netmask.to_string(),
        gateway: conf.bridge.gateway.to_string(),
        timezone: ui_timezone.unwrap_or_else(|| conf.bridge.timezone.clone()),
        hass_lat,
        hass_long,
        total_entities,
        included_entities,
        hidden_entities,
        room_count,
        linkbutton_active,
        default_add_new_devices_to_hue: defaults,
        sync_hass_areas_to_rooms: sync_areas,
        sync_status,
    }))
}

async fn post_linkbutton(
    State(state): State<AppState>,
) -> BifrostApiResult<Json<HassLinkButtonResponse>> {
    state
        .press_linkbutton(Duration::from_secs(LINKBUTTON_DURATION_SECS))
        .await;

    {
        let ui = state.hass_ui();
        let mut lock = ui.lock().await;
        lock.push_log(format!(
            "Virtual bridge button pressed ({}s active)",
            LINKBUTTON_DURATION_SECS
        ));
    }

    Ok(Json(HassLinkButtonResponse {
        active: true,
        active_for_seconds: LINKBUTTON_DURATION_SECS,
    }))
}

async fn post_sync(State(state): State<AppState>) -> BifrostApiResult<Json<HassSyncResponse>> {
    {
        let res = state.res.lock().await;
        res.backend_request(BackendRequest::HassSync)?;
    }
    let sync = state.hass_ui().lock().await.sync.clone();
    Ok(Json(HassSyncResponse { queued: true, sync }))
}

async fn post_apply(State(state): State<AppState>) -> BifrostApiResult<Json<HassApplyResponse>> {
    let (cfg, entities) = {
        let ui = state.hass_ui();
        let lock = ui.lock().await;
        (lock.config_normalized(), lock.entities.clone())
    };

    let backend_name = state
        .config()
        .hass
        .servers
        .keys()
        .next()
        .cloned()
        .unwrap_or_else(|| "homeassistant".to_string());

    let mut keep_device_rids = HashSet::new();
    for ent in &entities {
        let mut include = cfg.should_include(&ent.entity_id, &ent.name, ent.available);
        if ent.domain == "binary_sensor" {
            let detected = ent.sensor_kind.unwrap_or(HassSensorKind::Ignore);
            if matches!(
                cfg.sensor_kind(&ent.entity_id, detected),
                HassSensorKind::Ignore
            ) {
                include = false;
            }
        }

        if include {
            let link = RType::Device
                .deterministic(format!("hass:{}:{}:device", backend_name, ent.entity_id));
            keep_device_rids.insert(link.rid);
        }
    }

    let removed_devices = {
        let mut removed = 0_usize;
        let mut res = state.res.lock().await;
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
            if res.delete(&RType::Device.link_to(rid)).is_ok() {
                removed += 1;
            }
        }
        removed
    };

    {
        let ui = state.hass_ui();
        let mut lock = ui.lock().await;
        lock.push_log(format!(
            "Applied selection to Hue bridge (removed {removed_devices} devices)"
        ));
    }

    Ok(Json(HassApplyResponse {
        applied: true,
        removed_devices,
    }))
}

async fn post_reset_bridge(
    State(state): State<AppState>,
) -> BifrostApiResult<Json<HassResetBridgeResponse>> {
    let conf = state.config();
    let bridge_id = hue::bridge_id(conf.bridge.mac);

    {
        let mut res = state.res.lock().await;
        res.factory_reset(&bridge_id)?;
    }

    {
        let ui = state.hass_ui();
        let mut lock = ui.lock().await;
        lock.push_log("Hue bridge factory reset (resources cleared)");
    }

    Ok(Json(HassResetBridgeResponse { reset: true }))
}

async fn get_runtime_config(
    State(state): State<AppState>,
) -> BifrostApiResult<Json<HassRuntimeConfigPublic>> {
    let runtime = state.hass_runtime();
    let config = runtime.lock().await.public_config();
    Ok(Json(config))
}

async fn put_runtime_config(
    State(state): State<AppState>,
    Json(update): Json<HassRuntimeConfigUpdate>,
) -> BifrostApiResult<Json<HassRuntimeConfigPublic>> {
    let config = {
        let runtime = state.hass_runtime();
        let mut lock = runtime.lock().await;
        lock.set_config_update(update);
        lock.save()?;
        lock.public_config()
    };

    {
        let res = state.res.lock().await;
        if config.enabled {
            res.backend_request(BackendRequest::HassConnect)?;
        } else {
            res.backend_request(BackendRequest::HassDisconnect)?;
        }
    }

    Ok(Json(config))
}

async fn put_token(
    State(state): State<AppState>,
    Json(req): Json<HassTokenRequest>,
) -> BifrostApiResult<Json<HassRuntimeConfigPublic>> {
    let config = {
        let runtime = state.hass_runtime();
        let mut lock = runtime.lock().await;
        lock.set_token(req.token)?;
        lock.save()?;
        lock.public_config()
    };
    Ok(Json(config))
}

async fn delete_token(
    State(state): State<AppState>,
) -> BifrostApiResult<Json<HassRuntimeConfigPublic>> {
    let config = {
        let runtime = state.hass_runtime();
        let mut lock = runtime.lock().await;
        lock.clear_token();
        lock.save()?;
        lock.public_config()
    };
    Ok(Json(config))
}

async fn post_connect(
    State(state): State<AppState>,
) -> BifrostApiResult<Json<HassConnectResponse>> {
    let runtime_cfg = {
        let runtime = state.hass_runtime();
        let mut lock = runtime.lock().await;
        lock.config.enabled = true;
        lock.save()?;
        lock.public_config()
    };
    {
        let res = state.res.lock().await;
        res.backend_request(BackendRequest::HassConnect)?;
    }

    Ok(Json(HassConnectResponse {
        connected: true,
        runtime: runtime_cfg,
    }))
}

async fn post_disconnect(
    State(state): State<AppState>,
) -> BifrostApiResult<Json<HassConnectResponse>> {
    let runtime_cfg = {
        let runtime = state.hass_runtime();
        let mut lock = runtime.lock().await;
        lock.config.enabled = false;
        lock.save()?;
        lock.public_config()
    };
    {
        let res = state.res.lock().await;
        res.backend_request(BackendRequest::HassDisconnect)?;
    }

    Ok(Json(HassConnectResponse {
        connected: false,
        runtime: runtime_cfg,
    }))
}

async fn get_patina(State(state): State<AppState>) -> BifrostApiResult<Json<HassPatinaPublic>> {
    let ui = state.hass_ui();
    let patina = ui.lock().await.patina_public();
    Ok(Json(patina))
}

async fn post_patina_event(
    State(state): State<AppState>,
    Json(req): Json<HassPatinaEventRequest>,
) -> BifrostApiResult<Json<HassPatinaPublic>> {
    let ui = state.hass_ui();
    let mut lock = ui.lock().await;
    lock.record_patina_event(&req.kind, req.key.as_deref());
    lock.save_config()?;
    Ok(Json(lock.patina_public()))
}

pub fn router() -> Router<AppState> {
    Router::new()
        .merge(ui_router())
        .route("/hass/ui-payload", get(get_ui_payload))
        .route("/hass/ui-config", get(get_ui_config).put(put_ui_config))
        .route("/hass/entities", get(get_entities))
        .route("/hass/entity", put(patch_entity))
        .route(
            "/hass/rooms",
            get(get_rooms).post(post_room).delete(delete_room),
        )
        .route("/hass/room", put(put_room))
        .route("/hass/logs", get(get_logs))
        .route("/hass/bridge-info", get(get_bridge_info))
        .route("/hass/linkbutton", post(post_linkbutton))
        .route("/hass/sync", post(post_sync))
        .route("/hass/apply", post(post_apply))
        .route("/hass/reset-bridge", post(post_reset_bridge))
        .route(
            "/hass/runtime-config",
            get(get_runtime_config).put(put_runtime_config),
        )
        .route("/hass/token", put(put_token).delete(delete_token))
        .route("/hass/connect", post(post_connect))
        .route("/hass/disconnect", post(post_disconnect))
        .route("/hass/patina", get(get_patina))
        .route("/hass/patina/event", post(post_patina_event))
}
