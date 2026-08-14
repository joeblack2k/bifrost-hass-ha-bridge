use std::collections::HashMap;
use std::fs::{self, File};
use std::sync::Arc;
use std::time::{Duration, Instant};

use camino::Utf8Path;
use chrono::Utc;
use tokio::sync::Mutex;

use hue::legacy_api::{
    ApiConfig, ApiShortConfig, ConnectionState, Portal, PortalAction, PortalState, PortalTrust,
    Whitelist,
};
use svc::manager::SvmClient;

use crate::config::AppConfig;
use crate::error::ApiResult;
use crate::model::hass::{
    HassPortalAction, HassPortalCommunication, HassPortalConnectionState, HassRuntimeState,
    HassUiState,
};
use crate::model::state::{State, StateVersion};
use crate::resource::Resources;
use crate::server::certificate;
use crate::server::updater::VersionUpdater;

#[derive(Clone)]
pub struct AppState {
    conf: Arc<AppConfig>,
    upd: Arc<Mutex<VersionUpdater>>,
    svm: SvmClient,
    pub res: Arc<Mutex<Resources>>,
    hass_ui: Arc<Mutex<HassUiState>>,
    hass_runtime: Arc<Mutex<HassRuntimeState>>,
    linkbutton_until: Arc<Mutex<Option<Instant>>>,
}

impl AppState {
    pub async fn from_config(config: AppConfig, svm: SvmClient) -> ApiResult<Self> {
        let certfile = &config.bifrost.cert_file;

        let certpath = Utf8Path::new(certfile);
        if certpath.is_file() {
            certificate::check_certificate(certpath, config.bridge.mac)?;
        } else {
            log::warn!("Missing certificate file [{certfile}], generating..");
            certificate::generate_and_save(certpath, config.bridge.mac)?;
        }

        let mut res;
        let upd = Arc::new(Mutex::new(VersionUpdater::with_default_version()));
        let swversion = upd.lock().await.get().await.clone();

        if let Ok(fd) = File::open(&config.bifrost.state_file) {
            log::debug!("Existing state file found, loading..");
            let yaml = serde_yml::from_reader(fd)?;
            let state = match State::version(&yaml)? {
                StateVersion::V0 => {
                    log::info!("Detected state file version 0. Upgrading to new version..");
                    let backup_path = &config.bifrost.state_file.with_extension("v0.bak");
                    fs::rename(&config.bifrost.state_file, backup_path)?;
                    log::info!("  ..saved old state file as {backup_path}");
                    State::from_v0(yaml)?
                }
                StateVersion::V1 => {
                    log::info!("Detected state file version 1. Loading..");
                    State::from_v1(yaml)?
                }
            };
            res = Resources::new(swversion, state);
        } else {
            log::debug!("No state file found, initializing..");
            res = Resources::new(swversion, State::new());
            res.init(&hue::bridge_id(config.bridge.mac))?;
        }

        res.reset_all_streaming()?;
        res.ensure_core_bridge_resources(&hue::bridge_id(config.bridge.mac))?;

        let hass_ui_state = HassUiState::load(config.bifrost.hass_ui_file.clone())?;
        log::info!(
            "Loaded Bifrost bridge settings from {} ({} rooms, {} entity preferences)",
            hass_ui_state.file,
            hass_ui_state.config.rooms.len(),
            hass_ui_state.config.entity_preferences.len()
        );
        let hass_ui = Arc::new(Mutex::new(hass_ui_state));
        let fallback_hass_url = config
            .hass
            .servers
            .values()
            .next()
            .map(|server| server.url.to_string());
        let hass_runtime = Arc::new(Mutex::new(HassRuntimeState::load(
            config.bifrost.hass_runtime_file.clone(),
            fallback_hass_url,
        )?));
        let conf = Arc::new(config);
        let res = Arc::new(Mutex::new(res));

        Ok(Self {
            conf,
            upd,
            svm,
            res,
            hass_ui,
            hass_runtime,
            linkbutton_until: Arc::new(Mutex::new(None)),
        })
    }

    #[must_use]
    pub fn config(&self) -> Arc<AppConfig> {
        self.conf.clone()
    }

    #[must_use]
    pub fn updater(&self) -> Arc<Mutex<VersionUpdater>> {
        self.upd.clone()
    }

    #[must_use]
    pub fn manager(&self) -> SvmClient {
        self.svm.clone()
    }

    #[must_use]
    pub fn hass_ui(&self) -> Arc<Mutex<HassUiState>> {
        self.hass_ui.clone()
    }

    #[must_use]
    pub fn hass_runtime(&self) -> Arc<Mutex<HassRuntimeState>> {
        self.hass_runtime.clone()
    }

    pub async fn press_linkbutton(&self, active_for: Duration) {
        let mut lock = self.linkbutton_until.lock().await;
        *lock = Some(Instant::now() + active_for);
    }

    pub async fn linkbutton_active(&self) -> bool {
        let now = Instant::now();
        let mut lock = self.linkbutton_until.lock().await;
        match *lock {
            Some(until) if until > now => true,
            Some(_) => {
                *lock = None;
                false
            }
            None => false,
        }
    }

    #[must_use]
    pub async fn api_short_config(&self) -> ApiShortConfig {
        let mac = self.conf.bridge.mac;
        ApiShortConfig::from_mac_and_version(mac, self.upd.lock().await.get().await)
    }

    pub async fn api_config(&self, username: String) -> ApiResult<ApiConfig> {
        let (ui_cfg, cloud) = {
            let ui = self.hass_ui.lock().await;
            let cfg = ui.config_normalized();
            let cloud = cfg.effective_fake_cloud();
            (cfg, cloud)
        };
        let timezone = ui_cfg
            .hass_timezone
            .clone()
            .unwrap_or_else(|| self.conf.bridge.timezone.clone());
        let tz = tzfile::Tz::named(&timezone)?;
        let localtime = Utc::now().with_timezone(&&tz).naive_local();
        let linkbutton = self.linkbutton_active().await;

        let res = ApiConfig {
            short_config: self.api_short_config().await,
            ipaddress: self.conf.bridge.ipaddress,
            netmask: self.conf.bridge.netmask,
            gateway: self.conf.bridge.gateway,
            timezone,
            lat: ui_cfg.hass_lat.unwrap_or_else(|| "0.0000".to_string()),
            long: ui_cfg.hass_long.unwrap_or_else(|| "0.0000".to_string()),
            whitelist: HashMap::from([(
                username,
                Whitelist {
                    create_date: Utc::now(),
                    last_use_date: Utc::now(),
                    name: "User#foo".to_string(),
                },
            )]),
            localtime,
            linkbutton,
            internet: cloud.internet,
            internetservices: hue::legacy_api::ApiInternetServices {
                internet: if cloud.internet {
                    ConnectionState::Connected
                } else {
                    ConnectionState::Disconnected
                },
                remoteaccess: if cloud.outgoing {
                    ConnectionState::Connected
                } else {
                    ConnectionState::Disconnected
                },
                swupdate: if cloud.outgoing {
                    ConnectionState::Connected
                } else {
                    ConnectionState::Disconnected
                },
                time: if cloud.outgoing {
                    ConnectionState::Connected
                } else {
                    ConnectionState::Disconnected
                },
            },
            portalconnection: map_connection_state(cloud.connectionstate),
            portalstate: PortalState {
                communication: map_communication_state(cloud.communication),
                incoming: cloud.incoming,
                outgoing: cloud.outgoing,
                signedon: cloud.signedon,
            },
            portal: Portal {
                legacy: cloud.legacy,
                connectionstate: map_connection_state(cloud.connectionstate),
                trust: PortalTrust {
                    trusted: cloud.trusted,
                },
                action: map_portal_action(cloud.action),
            },
            portalservices: cloud.signedon,
            ..ApiConfig::default()
        };

        Ok(res)
    }
}

fn map_communication_state(value: HassPortalCommunication) -> ConnectionState {
    match value {
        HassPortalCommunication::Connected => ConnectionState::Connected,
        HassPortalCommunication::Disconnected => ConnectionState::Disconnected,
        HassPortalCommunication::Error => ConnectionState::Error,
    }
}

fn map_connection_state(value: HassPortalConnectionState) -> ConnectionState {
    match value {
        HassPortalConnectionState::Connected => ConnectionState::Connected,
        HassPortalConnectionState::Disconnected => ConnectionState::Disconnected,
        HassPortalConnectionState::Connecting => ConnectionState::Connecting,
    }
}

fn map_portal_action(value: HassPortalAction) -> PortalAction {
    match value {
        HassPortalAction::None => PortalAction::None,
        HassPortalAction::LinkButton => PortalAction::LinkButton,
    }
}
