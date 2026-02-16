mod backend_event;
mod client;
mod import;

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use async_trait::async_trait;
use svc::error::SvcError;
use svc::template::ServiceTemplate;
use svc::traits::{BoxDynService, Service};
use thiserror::Error;
use tokio::sync::{Mutex, broadcast::Receiver};
use tokio::time::{Duration, MissedTickBehavior, interval};
use uuid::Uuid;

use bifrost_api::backend::BackendRequest;
use bifrost_api::config::HassServer;
use hue::api::{RType, ResourceLink};

use crate::error::{ApiError, ApiResult};
use crate::model::hass::{HassRoomConfig, HassRuntimeState, HassSwitchMode, HassUiState};
use crate::resource::Resources;
use crate::server::appstate::AppState;

use self::client::{HassClient, HassWs};

#[derive(Error, Debug)]
pub enum TemplateError {
    #[error("No config found for hass server {0:?}")]
    NotFound(String),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum HassEntityKind {
    Light,
    Switch,
    BinarySensor,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum HassServiceKind {
    Light,
    Switch,
    Motion,
    Contact,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct HassLightCapabilities {
    pub supports_brightness: bool,
    pub supports_color: bool,
    pub supports_color_temp: bool,
}

#[derive(Clone, Debug)]
pub(super) struct HassEntityBinding {
    pub entity_id: String,
    pub name: String,
    pub kind: HassEntityKind,
    pub service_kind: HassServiceKind,
    pub service_link: ResourceLink,
    pub device_link: ResourceLink,
    pub capabilities: HassLightCapabilities,
    pub switch_mode: Option<HassSwitchMode>,
}

#[derive(Clone, Debug)]
pub(super) struct HassRoomBinding {
    pub room_id: String,
    pub room_name: String,
    pub room_link: ResourceLink,
    pub grouped_light_link: ResourceLink,
}

pub struct HassServiceTemplate {
    state: AppState,
}

impl HassServiceTemplate {
    #[must_use]
    pub const fn new(state: AppState) -> Self {
        Self { state }
    }
}

impl ServiceTemplate for HassServiceTemplate {
    fn generate(&self, name: String) -> Result<BoxDynService, SvcError> {
        let config = self.state.config();
        let Some(server) = config.hass.servers.get(&name) else {
            return Err(SvcError::generation(TemplateError::NotFound(name)));
        };

        let svc = HassBackend::new(
            name,
            server.clone(),
            self.state.res.clone(),
            self.state.hass_ui(),
            self.state.hass_runtime(),
        )
        .map_err(SvcError::generation)?;

        Ok(svc.boxed())
    }
}

pub struct HassBackend {
    name: String,
    server: HassServer,
    state: Arc<Mutex<Resources>>,
    ui_state: Arc<Mutex<HassUiState>>,
    runtime_state: Arc<Mutex<HassRuntimeState>>,
    client: HassClient,
    entity_map: HashMap<String, HassEntityBinding>,
    light_map: HashMap<Uuid, String>,
    sensor_map: HashMap<Uuid, String>,
    device_map: HashMap<Uuid, String>,
    room_map: HashMap<String, HassRoomBinding>,
    scene_map: HashMap<Uuid, String>,
    ws: Option<HassWs>,
}

impl HassBackend {
    pub fn new(
        name: String,
        server: HassServer,
        state: Arc<Mutex<Resources>>,
        ui_state: Arc<Mutex<HassUiState>>,
        runtime_state: Arc<Mutex<HassRuntimeState>>,
    ) -> ApiResult<Self> {
        Ok(Self {
            client: HassClient::new(&name, &server)?,
            name,
            server,
            state,
            ui_state,
            runtime_state,
            entity_map: HashMap::new(),
            light_map: HashMap::new(),
            sensor_map: HashMap::new(),
            device_map: HashMap::new(),
            room_map: HashMap::new(),
            scene_map: HashMap::new(),
            ws: None,
        })
    }

    pub(super) fn room_links_for_id(&self, room_id: &str) -> (ResourceLink, ResourceLink) {
        (
            RType::Room.deterministic(format!("hass:{}:room:{}", self.name, room_id)),
            RType::GroupedLight.deterministic(format!("hass:{}:grouped:{}", self.name, room_id)),
        )
    }

    fn room_binding(&self, room: &HassRoomConfig) -> HassRoomBinding {
        let (room_link, grouped_light_link) = self.room_links_for_id(&room.id);
        HassRoomBinding {
            room_id: room.id.clone(),
            room_name: room.name.clone(),
            room_link,
            grouped_light_link,
        }
    }

    pub(super) async fn ui_log(&self, message: impl AsRef<str>) {
        let mut ui = self.ui_state.lock().await;
        ui.push_log(format!("[{}] {}", self.name, message.as_ref()));
    }

    fn token_env_name(&self) -> String {
        self.server
            .token_env
            .clone()
            .unwrap_or_else(|| "HASS_TOKEN".to_string())
    }

    async fn apply_runtime_connection(&mut self) -> ApiResult<()> {
        let (enabled, runtime_url, runtime_token) = {
            let rt = self.runtime_state.lock().await;
            (rt.enabled(), rt.config.url.clone(), rt.token())
        };

        if !enabled {
            return Err(ApiError::service_error(format!(
                "[{}] Home Assistant backend is disconnected in runtime config",
                self.name
            )));
        }

        let url = if runtime_url.trim().is_empty() {
            self.server.url.clone()
        } else {
            url::Url::parse(runtime_url.trim())?
        };

        if let Some(token) = runtime_token {
            self.client.set_runtime(url, Some(token))?;
            return Ok(());
        }

        self.client.set_base_url(url);
        self.client.load_token_from_env(&self.server).map_err(|_| {
            ApiError::service_error(format!(
                "[{}] No Home Assistant token set. Configure token in GUI or env {}",
                self.name,
                self.token_env_name()
            ))
        })
    }

    async fn run_sync(&mut self, reason: &str) -> ApiResult<()> {
        {
            let mut ui = self.ui_state.lock().await;
            ui.mark_sync_started();
            ui.push_log(format!("Sync requested: {reason}"));
        }

        let start = Instant::now();
        let result = self.sync_entities().await;
        let elapsed_u128 = start.elapsed().as_millis();
        let elapsed = u64::try_from(elapsed_u128).unwrap_or(u64::MAX);

        let mut ui = self.ui_state.lock().await;
        match &result {
            Ok(()) => {
                ui.mark_sync_finished(Ok(elapsed));
                ui.push_log(format!("Sync completed in {elapsed}ms"));
            }
            Err(err) => {
                ui.mark_sync_finished(Err(err.to_string()));
                ui.push_log(format!("Sync failed: {err}"));
            }
        }
        drop(ui);

        result
    }

    async fn ensure_ws_connected(&mut self) {
        if self.ws.is_some() {
            return;
        }

        let enabled = {
            let rt = self.runtime_state.lock().await;
            rt.enabled()
        };
        if !enabled {
            return;
        }

        if let Err(err) = self.apply_runtime_connection().await {
            log::debug!("[{}] WS connect skipped: {}", self.name, err);
            return;
        }

        match self.client.subscribe_state_changed().await {
            Ok(ws) => {
                self.ws = Some(ws);
                self.ui_log("Realtime state sync connected (Home Assistant websocket)")
                    .await;
            }
            Err(err) => {
                log::debug!("[{}] WS connect failed: {}", self.name, err);
            }
        }
    }

    async fn event_loop(&mut self, chan: &mut Receiver<Arc<BackendRequest>>) -> ApiResult<()> {
        if let Err(err) = self.run_sync("startup").await {
            log::error!(
                "[{}] Initial Home Assistant sync failed: {}",
                self.name,
                err
            );
        }

        let mut ws_tick = interval(Duration::from_secs(10));
        ws_tick.set_missed_tick_behavior(MissedTickBehavior::Skip);

        loop {
            if let Some(ws) = &mut self.ws {
                tokio::select! {
                    _ = ws_tick.tick() => {
                        self.ensure_ws_connected().await;
                    }
                    req = chan.recv() => {
                        let req = req?;
                        self.handle_backend_event(req).await?;
                    }
                    ev = ws.next_state_changed() => {
                        match ev {
                            Ok(Some(ev)) => {
                                // Keep fields "used" to avoid -D warnings while still being explicit
                                // about which parts drive Hue state updates.
                                let _entity_id = ev.entity_id;
                                let _old_state = ev.old_state;
                                if let Some(new_state) = ev.new_state {
                                    let _ = self.handle_state_update(new_state).await;
                                }
                            }
                            Ok(None) => {
                                // websocket closed, reconnect later
                                self.ws = None;
                            }
                            Err(err) => {
                                log::debug!("[{}] WS error: {}", self.name, err);
                                self.ws = None;
                            }
                        }
                    }
                }
            } else {
                tokio::select! {
                    _ = ws_tick.tick() => {
                        self.ensure_ws_connected().await;
                    }
                    req = chan.recv() => {
                        let req = req?;
                        self.handle_backend_event(req).await?;
                    }
                }
            }
        }
    }
}

#[async_trait]
impl Service for HassBackend {
    type Error = ApiError;

    async fn start(&mut self) -> ApiResult<()> {
        match self.apply_runtime_connection().await {
            Ok(()) => {
                log::info!("[{}] Home Assistant backend ready", self.name);
                self.ui_log("Home Assistant backend started").await;
            }
            Err(err) => {
                log::warn!(
                    "[{}] Home Assistant backend started without active HA connection: {}",
                    self.name,
                    err
                );
                self.ui_log(format!(
                    "Backend started without active HA connection: {}",
                    err
                ))
                .await;
            }
        }
        Ok(())
    }

    async fn run(&mut self) -> ApiResult<()> {
        let mut chan = self.state.lock().await.backend_event_stream();
        self.event_loop(&mut chan).await
    }

    async fn stop(&mut self) -> ApiResult<()> {
        Ok(())
    }
}
