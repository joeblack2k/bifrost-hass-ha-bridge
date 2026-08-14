use std::collections::{BTreeSet, HashMap};
use std::fs;
use std::fs::File;
use std::io::Write;

use camino::Utf8PathBuf;
use chrono::{DateTime, Local, Utc};
use serde::{Deserialize, Serialize};
use url::Url;
use uuid::Uuid;

use crate::error::{ApiError, ApiResult};

#[derive(Clone, Copy, Debug, Serialize, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum HassSensorKind {
    Motion,
    Contact,
    Ignore,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum HassSwitchMode {
    Plug,
    Light,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, Eq, PartialEq, Default)]
#[serde(rename_all = "snake_case")]
pub enum HassLightArchetype {
    #[default]
    ClassicBulb,
    SultanBulb,
    CandleBulb,
    SpotBulb,
    VintageBulb,
    FloodBulb,
    CeilingRound,
    CeilingSquare,
    PendantRound,
    PendantLong,
    FloorShade,
    FloorLantern,
    TableShade,
    WallSpot,
    WallLantern,
    RecessedCeiling,
    HueLightstrip,
    HuePlay,
    HueGo,
    HueBloom,
    HueIris,
    HueSigne,
    HueTube,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum HassFakeCloudMode {
    Off,
    Connected,
    Outage,
    Custom,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, Eq, PartialEq, Default)]
#[serde(rename_all = "snake_case")]
pub enum HassPortalCommunication {
    #[default]
    Connected,
    Disconnected,
    Error,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, Eq, PartialEq, Default)]
#[serde(rename_all = "snake_case")]
pub enum HassPortalConnectionState {
    #[default]
    Connected,
    Disconnected,
    Connecting,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, Eq, PartialEq, Default)]
#[serde(rename_all = "snake_case")]
pub enum HassPortalAction {
    #[default]
    None,
    LinkButton,
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct HassFakeCloudState {
    #[serde(default)]
    pub internet: bool,
    #[serde(default)]
    pub signedon: bool,
    #[serde(default)]
    pub incoming: bool,
    #[serde(default)]
    pub outgoing: bool,
    #[serde(default)]
    pub communication: HassPortalCommunication,
    #[serde(default)]
    pub connectionstate: HassPortalConnectionState,
    #[serde(default)]
    pub legacy: bool,
    #[serde(default)]
    pub trusted: bool,
    #[serde(default)]
    pub action: HassPortalAction,
}

impl Default for HassFakeCloudState {
    fn default() -> Self {
        Self {
            internet: false,
            signedon: false,
            incoming: false,
            outgoing: false,
            communication: HassPortalCommunication::Disconnected,
            connectionstate: HassPortalConnectionState::Disconnected,
            legacy: false,
            trusted: true,
            action: HassPortalAction::None,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct HassRoomConfig {
    pub id: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_area: Option<String>,
    #[serde(default)]
    pub auto_created: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq, Default)]
pub struct HassEntityPreference {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub visible: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub room_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alias: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sensor_kind: Option<HassSensorKind>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sensor_enabled: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub switch_mode: Option<HassSwitchMode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub light_archetype: Option<HassLightArchetype>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct HassUiConfig {
    #[serde(default)]
    pub hidden_entity_ids: Vec<String>,
    #[serde(default)]
    pub exclude_entity_ids: Vec<String>,
    #[serde(default)]
    pub exclude_name_patterns: Vec<String>,
    #[serde(default = "HassUiConfig::default_include_unavailable")]
    pub include_unavailable: bool,
    #[serde(default)]
    pub rooms: Vec<HassRoomConfig>,
    #[serde(default)]
    pub entity_preferences: HashMap<String, HassEntityPreference>,
    #[serde(default)]
    pub ignored_area_names: Vec<String>,
    #[serde(default = "HassUiConfig::default_add_new")]
    pub default_add_new_devices_to_hue: bool,
    #[serde(default = "HassUiConfig::default_sync_areas")]
    pub sync_hass_areas_to_rooms: bool,
    #[serde(default = "HassUiConfig::default_fake_cloud_mode")]
    pub fake_cloud_mode: HassFakeCloudMode,
    #[serde(default)]
    pub fake_cloud_custom: HassFakeCloudState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hass_timezone: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hass_lat: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hass_long: Option<String>,
}

impl Default for HassUiConfig {
    fn default() -> Self {
        let mut cfg = Self {
            hidden_entity_ids: Vec::new(),
            exclude_entity_ids: Vec::new(),
            exclude_name_patterns: Vec::new(),
            include_unavailable: Self::default_include_unavailable(),
            rooms: Vec::new(),
            entity_preferences: HashMap::new(),
            ignored_area_names: Vec::new(),
            default_add_new_devices_to_hue: Self::default_add_new(),
            sync_hass_areas_to_rooms: Self::default_sync_areas(),
            fake_cloud_mode: Self::default_fake_cloud_mode(),
            fake_cloud_custom: HassFakeCloudState::default(),
            hass_timezone: None,
            hass_lat: None,
            hass_long: None,
        };
        cfg.ensure_default_room();
        cfg
    }
}

impl HassUiConfig {
    pub const DEFAULT_ROOM_ID: &'static str = "home-assistant";
    const DEFAULT_ROOM_NAME: &'static str = "Home Assistant";

    const fn default_include_unavailable() -> bool {
        true
    }

    // New installs should hide everything until explicitly added to Hue.
    const fn default_add_new() -> bool {
        false
    }

    const fn default_sync_areas() -> bool {
        true
    }

    const fn default_fake_cloud_mode() -> HassFakeCloudMode {
        HassFakeCloudMode::Off
    }

    fn sanitize_id(text: &str) -> String {
        let mut out = String::new();
        let mut last_dash = false;
        for ch in text.chars() {
            let low = ch.to_ascii_lowercase();
            if low.is_ascii_alphanumeric() {
                out.push(low);
                last_dash = false;
            } else if (low.is_ascii_whitespace() || low == '-' || low == '_') && !last_dash {
                out.push('-');
                last_dash = true;
            }
        }
        out.trim_matches('-').to_string()
    }

    pub fn ensure_default_room(&mut self) {
        if !self.rooms.iter().any(|x| x.id == Self::DEFAULT_ROOM_ID) {
            self.rooms.insert(
                0,
                HassRoomConfig {
                    id: Self::DEFAULT_ROOM_ID.to_string(),
                    name: Self::DEFAULT_ROOM_NAME.to_string(),
                    source_area: None,
                    auto_created: false,
                },
            );
        }
    }

    pub fn normalize(&mut self) {
        self.hidden_entity_ids = self
            .hidden_entity_ids
            .iter()
            .map(|x| x.trim().to_string())
            .filter(|x| !x.is_empty())
            .collect();
        self.exclude_entity_ids = self
            .exclude_entity_ids
            .iter()
            .map(|x| x.trim().to_string())
            .filter(|x| !x.is_empty())
            .collect();
        self.exclude_name_patterns = self
            .exclude_name_patterns
            .iter()
            .map(|x| x.trim().to_string())
            .filter(|x| !x.is_empty())
            .collect();
        self.ignored_area_names = self
            .ignored_area_names
            .iter()
            .map(|x| x.trim().to_string())
            .filter(|x| !x.is_empty())
            .collect();
        self.hass_timezone = self
            .hass_timezone
            .as_ref()
            .map(|x| x.trim().to_string())
            .filter(|x| !x.is_empty());
        self.hass_lat = self
            .hass_lat
            .as_ref()
            .map(|x| x.trim().to_string())
            .filter(|x| !x.is_empty());
        self.hass_long = self
            .hass_long
            .as_ref()
            .map(|x| x.trim().to_string())
            .filter(|x| !x.is_empty());

        let mut seen = BTreeSet::new();
        let mut normalized = Vec::new();
        for room in &self.rooms {
            let mut id = Self::sanitize_id(&room.id);
            if id.is_empty() {
                id = Self::sanitize_id(&room.name);
            }
            if id.is_empty() || seen.contains(&id) {
                continue;
            }
            seen.insert(id.clone());
            normalized.push(HassRoomConfig {
                id,
                name: room.name.trim().to_string(),
                source_area: room
                    .source_area
                    .as_ref()
                    .map(|x| x.trim().to_string())
                    .filter(|x| !x.is_empty()),
                auto_created: room.auto_created,
            });
        }
        self.rooms = normalized;
        self.ensure_default_room();

        for entity_id in self
            .hidden_entity_ids
            .iter()
            .chain(self.exclude_entity_ids.iter())
        {
            self.entity_preferences
                .entry(entity_id.to_string())
                .or_default()
                .visible
                .get_or_insert(false);
        }

        let room_ids = self
            .rooms
            .iter()
            .map(|x| x.id.clone())
            .collect::<BTreeSet<_>>();
        self.entity_preferences.retain(|entity_id, pref| {
            let id = entity_id.trim();
            if id.is_empty() {
                return false;
            }
            if let Some(room_id) = pref.room_id.as_ref() {
                if !room_ids.contains(room_id) {
                    pref.room_id = None;
                }
            }
            pref.alias = pref
                .alias
                .as_ref()
                .map(|x| x.trim().to_string())
                .filter(|x| !x.is_empty());
            pref.visible.is_some()
                || pref.room_id.is_some()
                || pref.alias.is_some()
                || pref.sensor_kind.is_some()
                || pref.sensor_enabled.is_some()
                || pref.switch_mode.is_some()
                || pref.light_archetype.is_some()
        });
    }

    pub fn is_manually_hidden(&self, entity_id: &str) -> bool {
        if self
            .entity_preferences
            .get(entity_id)
            .and_then(|x| x.visible)
            == Some(false)
        {
            return true;
        }
        self.hidden_entity_ids
            .iter()
            .any(|x| x.eq_ignore_ascii_case(entity_id))
            || self
                .exclude_entity_ids
                .iter()
                .any(|x| x.eq_ignore_ascii_case(entity_id))
    }

    pub fn set_entity_hidden(&mut self, entity_id: &str, hidden: bool) {
        let pref = self
            .entity_preferences
            .entry(entity_id.to_string())
            .or_default();
        pref.visible = Some(!hidden);
        self.hidden_entity_ids
            .retain(|x| !x.eq_ignore_ascii_case(entity_id));
        self.exclude_entity_ids
            .retain(|x| !x.eq_ignore_ascii_case(entity_id));
        if hidden {
            self.hidden_entity_ids.push(entity_id.to_string());
        }
        self.normalize();
    }

    pub fn set_entity_room(&mut self, entity_id: &str, room_id: Option<String>) {
        let pref = self
            .entity_preferences
            .entry(entity_id.to_string())
            .or_default();
        pref.room_id = room_id;
        self.normalize();
    }

    pub fn set_entity_alias(&mut self, entity_id: &str, alias: Option<String>) {
        let pref = self
            .entity_preferences
            .entry(entity_id.to_string())
            .or_default();
        pref.alias = alias
            .map(|x| x.trim().to_string())
            .filter(|x| !x.is_empty());
        self.normalize();
    }

    pub fn set_entity_sensor_kind(&mut self, entity_id: &str, sensor_kind: Option<HassSensorKind>) {
        let pref = self
            .entity_preferences
            .entry(entity_id.to_string())
            .or_default();
        pref.sensor_kind = sensor_kind;
        self.normalize();
    }

    pub fn set_entity_sensor_enabled(&mut self, entity_id: &str, enabled: bool) {
        let pref = self
            .entity_preferences
            .entry(entity_id.to_string())
            .or_default();
        pref.sensor_enabled = Some(enabled);
        self.normalize();
    }

    pub fn set_entity_switch_mode(&mut self, entity_id: &str, switch_mode: Option<HassSwitchMode>) {
        let pref = self
            .entity_preferences
            .entry(entity_id.to_string())
            .or_default();
        pref.switch_mode = switch_mode;
        self.normalize();
    }

    pub fn set_entity_light_archetype(
        &mut self,
        entity_id: &str,
        light_archetype: Option<HassLightArchetype>,
    ) {
        let pref = self
            .entity_preferences
            .entry(entity_id.to_string())
            .or_default();
        pref.light_archetype = light_archetype;
        self.normalize();
    }

    #[must_use]
    pub fn entity_alias(&self, entity_id: &str) -> Option<String> {
        self.entity_preferences
            .get(entity_id)
            .and_then(|x| x.alias.as_ref())
            .cloned()
    }

    #[must_use]
    pub fn sensor_kind(&self, entity_id: &str, detected: HassSensorKind) -> HassSensorKind {
        self.entity_preferences
            .get(entity_id)
            .and_then(|x| x.sensor_kind)
            .unwrap_or(detected)
    }

    #[must_use]
    pub fn sensor_enabled(&self, entity_id: &str) -> bool {
        self.entity_preferences
            .get(entity_id)
            .and_then(|x| x.sensor_enabled)
            .unwrap_or(true)
    }

    #[must_use]
    pub fn switch_mode(&self, entity_id: &str) -> HassSwitchMode {
        self.entity_preferences
            .get(entity_id)
            .and_then(|x| x.switch_mode)
            .unwrap_or(HassSwitchMode::Plug)
    }

    #[must_use]
    pub fn light_archetype(&self, entity_id: &str) -> HassLightArchetype {
        self.entity_preferences
            .get(entity_id)
            .and_then(|x| x.light_archetype)
            .unwrap_or(HassLightArchetype::ClassicBulb)
    }

    pub fn room_for_area(&self, area_name: &str) -> Option<String> {
        self.rooms
            .iter()
            .find(|x| {
                x.source_area
                    .as_ref()
                    .is_some_and(|src| src.eq_ignore_ascii_case(area_name))
            })
            .map(|x| x.id.clone())
    }

    pub fn ensure_room_for_area(&mut self, area_name: &str) -> String {
        if self
            .ignored_area_names
            .iter()
            .any(|x| x.eq_ignore_ascii_case(area_name))
        {
            return Self::DEFAULT_ROOM_ID.to_string();
        }

        if let Some(room_id) = self.room_for_area(area_name) {
            return room_id;
        }

        let mut base = format!("area-{}", Self::sanitize_id(area_name));
        if base == "area-" || base.is_empty() {
            base = "area-room".to_string();
        }
        let mut room_id = base.clone();
        let mut i = 2_u32;
        let room_ids = self
            .rooms
            .iter()
            .map(|x| x.id.to_ascii_lowercase())
            .collect::<BTreeSet<_>>();
        while room_ids.contains(&room_id.to_ascii_lowercase()) {
            room_id = format!("{base}-{i}");
            i += 1;
        }

        self.rooms.push(HassRoomConfig {
            id: room_id.clone(),
            name: area_name.to_string(),
            source_area: Some(area_name.to_string()),
            auto_created: true,
        });
        self.normalize();
        room_id
    }

    #[must_use]
    pub fn room_name(&self, room_id: &str) -> String {
        self.rooms
            .iter()
            .find(|x| x.id == room_id)
            .map(|x| x.name.clone())
            .unwrap_or_else(|| room_id.to_string())
    }

    pub fn set_hass_location(
        &mut self,
        timezone: Option<String>,
        lat: Option<String>,
        long: Option<String>,
    ) {
        self.hass_timezone = timezone;
        self.hass_lat = lat;
        self.hass_long = long;
        self.normalize();
    }

    #[must_use]
    pub fn effective_fake_cloud(&self) -> HassFakeCloudState {
        match self.fake_cloud_mode {
            HassFakeCloudMode::Off => HassFakeCloudState::default(),
            HassFakeCloudMode::Connected => HassFakeCloudState {
                internet: true,
                signedon: true,
                incoming: true,
                outgoing: true,
                communication: HassPortalCommunication::Connected,
                connectionstate: HassPortalConnectionState::Connected,
                legacy: false,
                trusted: true,
                action: HassPortalAction::None,
            },
            HassFakeCloudMode::Outage => HassFakeCloudState {
                internet: false,
                signedon: true,
                incoming: false,
                outgoing: false,
                communication: HassPortalCommunication::Disconnected,
                connectionstate: HassPortalConnectionState::Disconnected,
                legacy: false,
                trusted: true,
                action: HassPortalAction::None,
            },
            HassFakeCloudMode::Custom => self.fake_cloud_custom.clone(),
        }
    }

    #[must_use]
    pub fn should_include(&self, entity_id: &str, display_name: &str, available: bool) -> bool {
        if !self.include_unavailable && !available {
            return false;
        }

        let entity_id_lc = entity_id.to_ascii_lowercase();
        let name_lc = display_name.to_ascii_lowercase();

        // Explicit per-entity visibility overrides patterns/defaults.
        if let Some(visible) = self
            .entity_preferences
            .get(entity_id)
            .and_then(|x| x.visible)
        {
            return visible;
        }

        if self.is_manually_hidden(entity_id) {
            return false;
        }

        if self.exclude_name_patterns.iter().any(|x| {
            if x.is_empty() {
                return false;
            }
            let pat = x.to_ascii_lowercase();
            entity_id_lc.contains(&pat) || name_lc.contains(&pat)
        }) {
            return false;
        }

        self.default_add_new_devices_to_hue
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct HassEntitySummary {
    pub entity_id: String,
    pub domain: String,
    pub name: String,
    pub state: String,
    pub available: bool,
    pub included: bool,
    #[serde(default)]
    pub hidden: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub area_name: Option<String>,
    #[serde(default)]
    pub room_id: String,
    #[serde(default)]
    pub room_name: String,
    #[serde(default)]
    pub mapped_type: String,
    #[serde(default)]
    pub supports_brightness: bool,
    #[serde(default)]
    pub supports_color: bool,
    #[serde(default)]
    pub supports_color_temp: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub switch_mode: Option<HassSwitchMode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sensor_kind: Option<HassSensorKind>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub light_archetype: Option<HassLightArchetype>,
    #[serde(default)]
    pub enabled: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq, Default)]
pub struct HassSyncStatus {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_sync_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_sync_result: Option<String>,
    #[serde(default)]
    pub sync_in_progress: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_sync_duration_ms: Option<u64>,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum HassPatinaStage {
    Fresh,
    Used,
    Loved,
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct HassPatinaState {
    pub install_date: String,
    #[serde(default)]
    pub interaction_count: u64,
    #[serde(default)]
    pub interactions_by_key: HashMap<String, u64>,
}

impl Default for HassPatinaState {
    fn default() -> Self {
        Self {
            install_date: Utc::now().to_rfc3339(),
            interaction_count: 0,
            interactions_by_key: HashMap::new(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct HassPatinaPublic {
    pub install_date: String,
    pub interaction_count: u64,
    pub patina_level: u8,
    pub stage: HassPatinaStage,
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct HassRuntimeConfig {
    pub enabled: bool,
    pub url: String,
    pub sync_mode: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token: Option<String>,
}

impl Default for HassRuntimeConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            url: String::new(),
            sync_mode: "manual".to_string(),
            token: None,
        }
    }
}

fn atomic_write(path: &Utf8PathBuf, contents: &[u8]) -> ApiResult<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut temp_path = path.clone();
    temp_path.set_file_name(format!(
        ".{}.{}.tmp",
        path.file_name().unwrap_or("bifrost"),
        Uuid::new_v4()
    ));

    let mut temp_created = false;
    let result = (|| {
        let mut options = fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;

            options.mode(0o600);
        }

        let mut file = options.open(&temp_path)?;
        temp_created = true;
        file.write_all(contents)?;
        file.sync_all()?;
        drop(file);
        fs::rename(&temp_path, path)?;

        #[cfg(unix)]
        if let Some(parent) = path.parent() {
            if let Ok(directory) = File::open(parent) {
                let _ = directory.sync_all();
            }
        }

        Ok(())
    })();

    if temp_created && result.is_err() {
        let _ = fs::remove_file(&temp_path);
    }
    result
}

fn backup_path(path: &Utf8PathBuf) -> Utf8PathBuf {
    let mut backup = path.clone();
    backup.set_file_name(format!(
        "{}.bak",
        path.file_name().unwrap_or("bifrost-settings")
    ));
    backup
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct HassRuntimeConfigPublic {
    pub enabled: bool,
    pub url: String,
    pub sync_mode: String,
    pub token_present: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct HassRuntimeConfigUpdate {
    pub enabled: bool,
    pub url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sync_mode: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct HassTokenRequest {
    pub token: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct HassRuntimeState {
    pub file: Utf8PathBuf,
    pub config: HassRuntimeConfig,
}

impl HassRuntimeState {
    pub fn load(file: Utf8PathBuf, fallback_url: Option<String>) -> ApiResult<Self> {
        let mut config = if file.is_file() {
            match File::open(&file).and_then(|fd| {
                serde_yml::from_reader::<_, HassRuntimeConfig>(fd).map_err(std::io::Error::other)
            }) {
                Ok(config) => config,
                Err(err) => {
                    log::warn!("Failed to parse {}, using defaults: {}", file, err);
                    HassRuntimeConfig::default()
                }
            }
        } else {
            HassRuntimeConfig::default()
        };

        if config.url.trim().is_empty() {
            if let Some(url) = fallback_url {
                config.url = url;
            }
        }

        config.url = config.url.trim().to_string();
        if config.sync_mode.trim().is_empty() {
            config.sync_mode = "manual".to_string();
        }

        let state = Self { file, config };
        if !state.file.is_file() {
            state.save()?;
        }
        Ok(state)
    }

    pub fn save(&self) -> ApiResult<()> {
        let yaml = serde_yml::to_string(&self.config)?;
        atomic_write(&self.file, yaml.as_bytes())
    }

    pub fn public_config(&self) -> HassRuntimeConfigPublic {
        HassRuntimeConfigPublic {
            enabled: self.config.enabled,
            url: self.config.url.clone(),
            sync_mode: self.config.sync_mode.clone(),
            token_present: self
                .config
                .token
                .as_ref()
                .is_some_and(|x| !x.trim().is_empty()),
        }
    }

    pub fn set_config_update(&mut self, update: HassRuntimeConfigUpdate) -> ApiResult<()> {
        let url = Self::parse_url(&update.url)?;
        let previous_url = Self::parse_url(&self.config.url).ok();
        let origin_changed =
            previous_url.is_none_or(|previous| !Self::same_origin(&previous, &url));

        self.config.enabled = update.enabled;
        self.config.url = url.to_string();
        self.config.sync_mode = if update
            .sync_mode
            .as_ref()
            .is_none_or(|x| x.trim().is_empty())
        {
            "manual".to_string()
        } else {
            update
                .sync_mode
                .as_ref()
                .map(|x| x.trim().to_string())
                .unwrap_or_else(|| "manual".to_string())
        };

        // A runtime URL change must not reuse a token against another HA origin.
        if origin_changed {
            self.clear_token();
        }

        Ok(())
    }

    pub fn set_token(&mut self, token: String) -> ApiResult<()> {
        let token = token.trim().to_string();
        if token.is_empty() {
            return Err(ApiError::service_error(
                "HASS token cannot be empty".to_string(),
            ));
        }
        self.config.token = Some(token);
        Ok(())
    }

    pub fn clear_token(&mut self) {
        self.config.token = None;
    }

    #[must_use]
    pub fn enabled(&self) -> bool {
        self.config.enabled
    }

    pub fn parsed_url(&self) -> ApiResult<Url> {
        Self::parse_url(&self.config.url)
    }

    pub fn parse_url(raw: &str) -> ApiResult<Url> {
        if raw.trim().is_empty() {
            return Err(ApiError::service_error(
                "Home Assistant URL not set".to_string(),
            ));
        }

        let url = Url::parse(raw.trim())?;
        if !matches!(url.scheme(), "http" | "https") {
            return Err(ApiError::service_error(
                "Home Assistant URL must use http or https".to_string(),
            ));
        }
        if url.host_str().is_none() {
            return Err(ApiError::service_error(
                "Home Assistant URL must include a host".to_string(),
            ));
        }
        if !url.username().is_empty()
            || url.password().is_some()
            || url.query().is_some()
            || url.fragment().is_some()
        {
            return Err(ApiError::service_error(
                "Home Assistant URL cannot contain credentials, query parameters, or fragments"
                    .to_string(),
            ));
        }

        Ok(url)
    }

    #[must_use]
    pub fn same_origin(left: &Url, right: &Url) -> bool {
        left.scheme() == right.scheme()
            && left.host_str() == right.host_str()
            && left.port_or_known_default() == right.port_or_known_default()
    }

    #[must_use]
    pub fn token(&self) -> Option<String> {
        self.config
            .token
            .as_ref()
            .map(|x| x.trim().to_string())
            .filter(|x| !x.is_empty())
    }
}

#[cfg(test)]
mod runtime_state_tests {
    use super::{HassRuntimeConfig, HassRuntimeConfigUpdate, HassRuntimeState};
    use camino::Utf8PathBuf;

    fn state(url: &str) -> HassRuntimeState {
        HassRuntimeState {
            file: Utf8PathBuf::from("/dev/null"),
            config: HassRuntimeConfig {
                url: url.to_string(),
                token: Some("secret".to_string()),
                ..HassRuntimeConfig::default()
            },
        }
    }

    #[test]
    fn runtime_url_change_clears_token_only_for_a_new_origin() {
        let mut same_origin = state("http://ha.local:8123");
        same_origin
            .set_config_update(HassRuntimeConfigUpdate {
                enabled: true,
                url: "http://ha.local:8123/".to_string(),
                sync_mode: None,
            })
            .expect("same-origin URL should be accepted");
        assert!(same_origin.config.token.is_some());

        let mut new_origin = state("http://ha.local:8123");
        new_origin
            .set_config_update(HassRuntimeConfigUpdate {
                enabled: true,
                url: "https://other.local:8123".to_string(),
                sync_mode: None,
            })
            .expect("new URL should be accepted");
        assert!(new_origin.config.token.is_none());
    }

    #[test]
    fn runtime_url_rejects_non_http_and_embedded_credentials() {
        for url in ["file:///tmp/ha", "http://user:pass@ha.local:8123"] {
            let mut state = state("http://ha.local:8123");
            assert!(
                state
                    .set_config_update(HassRuntimeConfigUpdate {
                        enabled: true,
                        url: url.to_string(),
                        sync_mode: None,
                    })
                    .is_err(),
                "{url} should be rejected"
            );
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct HassUiState {
    pub file: Utf8PathBuf,
    pub config: HassUiConfig,
    #[serde(default)]
    pub patina: HassPatinaState,
    pub entities: Vec<HassEntitySummary>,
    pub logs: Vec<String>,
    #[serde(default)]
    pub sync: HassSyncStatus,
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
struct HassUiStateFile {
    #[serde(default)]
    config: HassUiConfig,
    #[serde(default)]
    patina: HassPatinaState,
}

fn parse_ui_state_file(raw: &str) -> ApiResult<(HassUiConfig, HassPatinaState)> {
    let has_v2_shape = serde_yml::from_str::<serde_yml::Value>(raw)
        .ok()
        .and_then(|value| value.as_mapping().cloned())
        .is_some_and(|mapping| {
            mapping.contains_key(serde_yml::Value::from("config"))
                || mapping.contains_key(serde_yml::Value::from("patina"))
        });

    if has_v2_shape {
        let state = serde_yml::from_str::<HassUiStateFile>(raw)?;
        Ok((state.config, state.patina))
    } else {
        let config = serde_yml::from_str::<HassUiConfig>(raw)?;
        Ok((config, HassPatinaState::default()))
    }
}

impl HassUiState {
    pub fn load(file: Utf8PathBuf) -> ApiResult<Self> {
        let (mut config, patina, recovered_from_backup) = if file.is_file() {
            let raw = fs::read_to_string(&file)?;
            match parse_ui_state_file(&raw) {
                Ok((config, patina)) => (config, patina, false),
                Err(primary_err) => {
                    let backup = backup_path(&file);
                    if !backup.is_file() {
                        return Err(ApiError::service_error(format!(
                            "Failed to parse bridge settings {}: {primary_err}",
                            file
                        )));
                    }

                    let backup_raw = fs::read_to_string(&backup)?;
                    let (config, patina) = parse_ui_state_file(&backup_raw).map_err(|backup_err| {
                        ApiError::service_error(format!(
                            "Failed to parse bridge settings {} ({primary_err}) and backup {} ({backup_err})",
                            file, backup
                        ))
                    })?;
                    log::warn!(
                        "Recovered bridge settings from {} because {} could not be parsed: {}",
                        backup,
                        file,
                        primary_err
                    );
                    (config, patina, true)
                }
            }
        } else {
            (HassUiConfig::default(), HassPatinaState::default(), false)
        };
        config.normalize();

        let state = Self {
            file,
            config,
            patina,
            entities: Vec::new(),
            logs: Vec::new(),
            sync: HassSyncStatus::default(),
        };

        if recovered_from_backup || !state.file.is_file() {
            state.save_config()?;
        }

        Ok(state)
    }

    pub fn save_config(&self) -> ApiResult<()> {
        let mut cfg = self.config.clone();
        cfg.normalize();
        let mut patina = self.patina.clone();
        if patina.install_date.trim().is_empty() {
            patina.install_date = Utc::now().to_rfc3339();
        }
        patina
            .interactions_by_key
            .retain(|k, _| !k.trim().is_empty());
        let state = HassUiStateFile {
            config: cfg,
            patina,
        };
        let yaml = serde_yml::to_string(&state)?;

        // Keep the last known-good settings beside the canonical file. The backup is written
        // before the replacement, so a failed new write never destroys the previous snapshot.
        if self.file.is_file() {
            let previous = fs::read_to_string(&self.file)?;
            if parse_ui_state_file(&previous).is_ok() {
                atomic_write(&backup_path(&self.file), previous.as_bytes())?;
            }
        }
        atomic_write(&self.file, yaml.as_bytes())
    }

    fn patina_days_since_install(&self) -> u64 {
        if self.patina.install_date.trim().is_empty() {
            return 0;
        }
        let parsed = DateTime::parse_from_rfc3339(self.patina.install_date.trim());
        let Ok(parsed) = parsed else {
            return 0;
        };
        let delta = Utc::now().signed_duration_since(parsed.with_timezone(&Utc));
        u64::try_from(delta.num_days().max(0)).unwrap_or(0)
    }

    #[must_use]
    pub fn patina_public(&self) -> HassPatinaPublic {
        let days = self.patina_days_since_install().min(365);
        let age_component = u8::try_from((days * 20) / 365).unwrap_or(20);
        let interaction_component =
            u8::try_from((self.patina.interaction_count.min(5000) * 80) / 5000).unwrap_or(80);
        let level = age_component.saturating_add(interaction_component).min(100);
        let stage = if level >= 71 {
            HassPatinaStage::Loved
        } else if level >= 26 {
            HassPatinaStage::Used
        } else {
            HassPatinaStage::Fresh
        };

        HassPatinaPublic {
            install_date: self.patina.install_date.clone(),
            interaction_count: self.patina.interaction_count,
            patina_level: level,
            stage,
        }
    }

    pub fn record_patina_event(&mut self, kind: &str, key: Option<&str>) {
        if self.patina.install_date.trim().is_empty() {
            self.patina.install_date = Utc::now().to_rfc3339();
        }

        let weight: u64 = match kind {
            "toggle" => 2,
            "apply" => 4,
            "sync" => 3,
            "reset" => 5,
            _ => 1,
        };
        self.patina.interaction_count = self.patina.interaction_count.saturating_add(weight);
        if let Some(key) = key.map(str::trim).filter(|k| !k.is_empty()) {
            let count = self
                .patina
                .interactions_by_key
                .entry(key.to_string())
                .or_insert(0);
            *count = count.saturating_add(weight);
        }
    }

    pub fn push_log(&mut self, message: impl AsRef<str>) {
        let ts = Local::now().format("%Y-%m-%d %H:%M:%S");
        self.logs.push(format!("[{ts}] {}", message.as_ref()));
        if self.logs.len() > 200 {
            let drain = self.logs.len() - 200;
            self.logs.drain(0..drain);
        }
    }

    pub fn mark_sync_started(&mut self) {
        self.sync.sync_in_progress = true;
        self.sync.last_sync_result = Some("running".to_string());
        self.sync.last_sync_at = Some(Utc::now().to_rfc3339());
    }

    pub fn mark_sync_finished(&mut self, result: Result<u64, String>) {
        self.sync.sync_in_progress = false;
        self.sync.last_sync_at = Some(Utc::now().to_rfc3339());
        match result {
            Ok(duration_ms) => {
                self.sync.last_sync_duration_ms = Some(duration_ms);
                self.sync.last_sync_result = Some("ok".to_string());
            }
            Err(err) => {
                self.sync.last_sync_result = Some(format!("error: {err}"));
            }
        }
    }

    pub fn add_room(&mut self, room_name: &str) -> Option<HassRoomConfig> {
        let name = room_name.trim();
        if name.is_empty() {
            return None;
        }
        let mut id = HassUiConfig::sanitize_id(name);
        if id.is_empty() {
            id = "room".to_string();
        }
        let mut candidate = id.clone();
        let mut i = 2_u32;
        let ids = self
            .config
            .rooms
            .iter()
            .map(|x| x.id.to_ascii_lowercase())
            .collect::<BTreeSet<_>>();
        while ids.contains(&candidate.to_ascii_lowercase()) {
            candidate = format!("{id}-{i}");
            i += 1;
        }
        let room = HassRoomConfig {
            id: candidate,
            name: name.to_string(),
            source_area: None,
            auto_created: false,
        };
        self.config.rooms.push(room.clone());
        self.config.normalize();
        Some(room)
    }

    pub fn remove_room(&mut self, room_id: &str) {
        if room_id == HassUiConfig::DEFAULT_ROOM_ID {
            return;
        }
        if let Some(source_area) = self
            .config
            .rooms
            .iter()
            .find(|x| x.id == room_id)
            .and_then(|x| x.source_area.clone())
        {
            self.config.ignored_area_names.push(source_area);
        }
        self.config.rooms.retain(|x| x.id != room_id);
        for pref in self.config.entity_preferences.values_mut() {
            if pref.room_id.as_deref() == Some(room_id) {
                pref.room_id = None;
            }
        }
        self.config.normalize();
    }

    pub fn rename_room(&mut self, room_id: &str, name: &str) {
        let trimmed = name.trim();
        if trimmed.is_empty() {
            return;
        }
        if let Some(room) = self.config.rooms.iter_mut().find(|room| room.id == room_id) {
            room.name = trimmed.to_string();
        }
        self.config.normalize();
    }

    pub fn set_entity_visibility(&mut self, entity_id: &str, hidden: bool) {
        self.config.set_entity_hidden(entity_id, hidden);
    }

    pub fn set_entity_room(&mut self, entity_id: &str, room_id: Option<String>) {
        self.config.set_entity_room(entity_id, room_id);
        self.config.normalize();
    }

    pub fn set_entity_alias(&mut self, entity_id: &str, alias: Option<String>) {
        self.config.set_entity_alias(entity_id, alias);
    }

    pub fn set_entity_sensor_kind(&mut self, entity_id: &str, sensor_kind: Option<HassSensorKind>) {
        self.config.set_entity_sensor_kind(entity_id, sensor_kind);
    }

    pub fn set_entity_sensor_enabled(&mut self, entity_id: &str, enabled: bool) {
        self.config.set_entity_sensor_enabled(entity_id, enabled);
    }

    pub fn set_entity_switch_mode(&mut self, entity_id: &str, switch_mode: Option<HassSwitchMode>) {
        self.config.set_entity_switch_mode(entity_id, switch_mode);
    }

    pub fn set_entity_light_archetype(
        &mut self,
        entity_id: &str,
        light_archetype: Option<HassLightArchetype>,
    ) {
        self.config
            .set_entity_light_archetype(entity_id, light_archetype);
    }

    pub fn visible_logs(&self) -> Vec<String> {
        self.logs.iter().rev().cloned().collect()
    }

    pub fn set_config(&mut self, config: HassUiConfig) {
        self.config = config;
        self.config.normalize();
    }

    pub fn config_normalized(&self) -> HassUiConfig {
        let mut cfg = self.config.clone();
        cfg.normalize();
        cfg
    }

    pub fn bridge_log_snapshot(&self) -> Vec<HassEntitySummary> {
        self.entities.clone()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct HassBridgeInfo {
    pub bridge_name: String,
    pub bridge_id: String,
    pub software_version: String,
    pub mac: String,
    pub ipaddress: String,
    pub netmask: String,
    pub gateway: String,
    pub timezone: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hass_lat: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hass_long: Option<String>,
    pub total_entities: usize,
    pub included_entities: usize,
    pub hidden_entities: usize,
    pub room_count: usize,
    pub linkbutton_active: bool,
    pub default_add_new_devices_to_hue: bool,
    pub sync_hass_areas_to_rooms: bool,
    pub sync_status: HassSyncStatus,
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct HassLinkButtonResponse {
    pub active: bool,
    pub active_for_seconds: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct HassRoomCreateRequest {
    pub name: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct HassRoomDeleteRequest {
    pub room_id: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct HassRoomRenameRequest {
    pub room_id: String,
    pub name: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct HassEntityPatchRequest {
    pub entity_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hidden: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub room_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alias: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sensor_kind: Option<HassSensorKind>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub switch_mode: Option<HassSwitchMode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub light_archetype: Option<HassLightArchetype>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct HassLogsResponse {
    pub logs: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct HassRoomsResponse {
    pub rooms: Vec<HassRoomConfig>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct HassEntitiesResponse {
    pub entities: Vec<HassEntitySummary>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct HassUiPayload {
    pub config: HassUiConfig,
    pub entities: Vec<HassEntitySummary>,
    pub logs: Vec<String>,
    pub sync: HassSyncStatus,
    pub patina: HassPatinaPublic,
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct HassSyncResponse {
    pub queued: bool,
    pub sync: HassSyncStatus,
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct HassResetBridgeResponse {
    pub reset: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct HassConnectResponse {
    pub queued: bool,
    pub enabled: bool,
    pub runtime: HassRuntimeConfigPublic,
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct HassPatinaEventRequest {
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
}

impl HassUiState {
    pub fn payload(&self) -> HassUiPayload {
        HassUiPayload {
            config: self.config_normalized(),
            entities: self.bridge_log_snapshot(),
            logs: self.visible_logs(),
            sync: self.sync.clone(),
            patina: self.patina_public(),
        }
    }

    pub fn persist_and_log(&mut self, reason: &str) -> ApiResult<()> {
        self.config.normalize();
        self.save_config()?;
        self.push_log(reason);
        Ok(())
    }
}

#[cfg(test)]
mod atomic_save_tests {
    use super::{
        HassEntityPreference, HassFakeCloudMode, HassPatinaState, HassRuntimeConfig,
        HassRuntimeState, HassSyncStatus, HassUiConfig, HassUiState, HassUiStateFile,
        parse_ui_state_file,
    };
    use camino::Utf8PathBuf;
    use std::collections::HashMap;
    use std::fs;
    use uuid::Uuid;

    struct TestDir(Utf8PathBuf);

    impl TestDir {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!("bifrost-hass-{}", Uuid::new_v4()));
            fs::create_dir(&path).expect("test directory should be created");
            Self(Utf8PathBuf::from_path_buf(path).expect("temporary path should be UTF-8"))
        }

        fn file(&self, name: &str) -> Utf8PathBuf {
            self.0.join(name)
        }

        fn assert_no_temp_files(&self) {
            let leftovers = fs::read_dir(&self.0)
                .expect("test directory should be readable")
                .map(|entry| {
                    entry
                        .expect("directory entry should be readable")
                        .file_name()
                })
                .filter(|name| name.to_string_lossy().ends_with(".tmp"))
                .collect::<Vec<_>>();
            assert!(
                leftovers.is_empty(),
                "temporary files remain: {leftovers:?}"
            );
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[cfg(unix)]
    fn assert_private(path: &Utf8PathBuf) {
        use std::os::unix::fs::PermissionsExt;

        let mode = fs::metadata(path)
            .expect("saved file should exist")
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600, "saved file must be private");
    }

    #[test]
    fn runtime_save_writes_complete_yaml_and_reloads_all_fields() {
        let dir = TestDir::new();
        let file = dir.file("runtime.yaml");
        let config = HassRuntimeConfig {
            enabled: false,
            url: "https://ha.local:8123".to_string(),
            sync_mode: "websocket".to_string(),
            token: Some("secret".to_string()),
        };
        let state = HassRuntimeState {
            file: file.clone(),
            config: config.clone(),
        };

        state.save().expect("runtime state should save");

        let yaml = fs::read_to_string(&file).expect("saved runtime YAML should be readable");
        let parsed: HassRuntimeConfig =
            serde_yml::from_str(&yaml).expect("runtime YAML should be complete and parseable");
        assert_eq!(parsed, config);
        assert_eq!(HassRuntimeState::load(file, None).unwrap().config, config);
        dir.assert_no_temp_files();
        #[cfg(unix)]
        assert_private(&state.file);
    }

    #[test]
    fn atomic_save_keeps_existing_target_when_rename_fails() {
        let dir = TestDir::new();
        let file = dir.file("runtime.yaml");
        fs::create_dir(&file).expect("target directory should be created");
        fs::write(file.join("marker"), b"old target").expect("target marker should be written");
        let state = HassRuntimeState {
            file: file.clone(),
            config: HassRuntimeConfig::default(),
        };

        assert!(
            state.save().is_err(),
            "renaming over a directory should fail"
        );
        assert_eq!(
            fs::read(file.join("marker")).expect("old target should remain"),
            b"old target"
        );
        dir.assert_no_temp_files();
    }

    #[test]
    fn ui_save_writes_normalized_yaml_and_reloads_persistent_fields() {
        let dir = TestDir::new();
        let file = dir.file("hass-ui.yaml");
        let mut config = HassUiConfig::default();
        config.hidden_entity_ids = vec![" light.kitchen ".to_string()];
        config.include_unavailable = false;
        config.default_add_new_devices_to_hue = true;
        config.sync_hass_areas_to_rooms = false;
        config.fake_cloud_mode = HassFakeCloudMode::Connected;
        config.hass_timezone = Some(" Europe/Amsterdam ".to_string());
        config.entity_preferences.insert(
            "light.kitchen".to_string(),
            HassEntityPreference {
                alias: Some("Kitchen".to_string()),
                ..HassEntityPreference::default()
            },
        );
        let patina = HassPatinaState {
            install_date: "2026-08-14T10:00:00Z".to_string(),
            interaction_count: 3,
            interactions_by_key: HashMap::from([(String::from("boot"), 2)]),
        };
        let state = HassUiState {
            file: file.clone(),
            config,
            patina: patina.clone(),
            entities: Vec::new(),
            logs: Vec::new(),
            sync: HassSyncStatus::default(),
        };

        state.save_config().expect("UI state should save");

        let yaml = fs::read_to_string(&file).expect("saved UI YAML should be readable");
        let stored: HassUiStateFile =
            serde_yml::from_str(&yaml).expect("UI YAML should be complete and parseable");
        assert_eq!(stored.patina, patina);
        assert!(!stored.config.include_unavailable);
        assert!(stored.config.default_add_new_devices_to_hue);
        assert!(!stored.config.sync_hass_areas_to_rooms);
        assert_eq!(stored.config.fake_cloud_mode, HassFakeCloudMode::Connected);
        assert_eq!(
            stored.config.hass_timezone.as_deref(),
            Some("Europe/Amsterdam")
        );
        assert_eq!(stored.config.hidden_entity_ids, ["light.kitchen"]);
        assert_eq!(
            stored
                .config
                .entity_preferences
                .get("light.kitchen")
                .and_then(|pref| pref.alias.as_deref()),
            Some("Kitchen")
        );

        let loaded = HassUiState::load(file).expect("UI state should reload");
        assert_eq!(loaded.patina, patina);
        assert_eq!(loaded.config.fake_cloud_mode, HassFakeCloudMode::Connected);
        assert_eq!(
            loaded.config.hass_timezone.as_deref(),
            Some("Europe/Amsterdam")
        );
        dir.assert_no_temp_files();
        #[cfg(unix)]
        assert_private(&state.file);
    }

    #[test]
    fn ui_save_keeps_previous_settings_in_a_private_backup() {
        let dir = TestDir::new();
        let file = dir.file("hass-ui.yaml");
        let mut state = HassUiState {
            file: file.clone(),
            config: HassUiConfig::default(),
            patina: HassPatinaState::default(),
            entities: Vec::new(),
            logs: Vec::new(),
            sync: HassSyncStatus::default(),
        };
        state.config.rooms.push(super::HassRoomConfig {
            id: "first".to_string(),
            name: "First".to_string(),
            source_area: None,
            auto_created: false,
        });
        state.save_config().expect("first settings should save");

        state.config.rooms.push(super::HassRoomConfig {
            id: "second".to_string(),
            name: "Second".to_string(),
            source_area: None,
            auto_created: false,
        });
        state.save_config().expect("second settings should save");

        let backup = dir.file("hass-ui.yaml.bak");
        let stored: HassUiStateFile =
            serde_yml::from_str(&fs::read_to_string(&backup).expect("backup should exist"))
                .expect("backup should be valid YAML");
        assert!(stored.config.rooms.iter().any(|room| room.id == "first"));
        assert!(!stored.config.rooms.iter().any(|room| room.id == "second"));
        #[cfg(unix)]
        assert_private(&backup);
    }

    #[test]
    fn ui_load_recovers_from_backup_but_fails_closed_without_one() {
        let dir = TestDir::new();
        let file = dir.file("hass-ui.yaml");
        let state = HassUiState {
            file: file.clone(),
            config: HassUiConfig::default(),
            patina: HassPatinaState::default(),
            entities: Vec::new(),
            logs: Vec::new(),
            sync: HassSyncStatus::default(),
        };
        state.save_config().expect("initial settings should save");
        let mut changed = state.clone();
        changed.config.rooms.push(super::HassRoomConfig {
            id: "office".to_string(),
            name: "Office".to_string(),
            source_area: None,
            auto_created: false,
        });
        changed.save_config().expect("changed settings should save");

        fs::write(&file, b"not: [valid").expect("canonical settings should be corruptible");
        let recovered = HassUiState::load(file.clone()).expect("backup should recover settings");
        assert_eq!(recovered.config.rooms, state.config.rooms);
        assert!(
            parse_ui_state_file(&fs::read_to_string(&file).expect("settings should be repaired"))
                .is_ok()
        );

        fs::write(dir.file("hass-ui.yaml.bak"), b"also: [not valid")
            .expect("backup should be corruptible");
        fs::write(&file, b"still: [not valid").expect("canonical settings should be corruptible");
        assert!(HassUiState::load(file).is_err());
    }
}

#[cfg(test)]
mod response_serialization_tests {
    use super::{HassConnectResponse, HassRuntimeConfigPublic, HassSyncResponse, HassSyncStatus};

    #[test]
    fn sync_and_apply_response_serializes_as_queued_sync_only() {
        let response = HassSyncResponse {
            queued: true,
            sync: HassSyncStatus {
                last_sync_at: Some("2026-08-14T12:00:00Z".to_string()),
                last_sync_result: Some("ok".to_string()),
                sync_in_progress: true,
                last_sync_duration_ms: Some(42),
            },
        };

        let value = serde_json::to_value(response).expect("sync response should serialize");

        assert_eq!(value["queued"], true);
        assert!(value.get("sync").is_some());
        assert!(value.get("applied").is_none());
        assert!(value.get("removed_devices").is_none());
    }

    #[test]
    fn connect_and_disconnect_response_serializes_as_queued_enabled_runtime() {
        for enabled in [true, false] {
            let response = HassConnectResponse {
                queued: true,
                enabled,
                runtime: HassRuntimeConfigPublic {
                    enabled,
                    url: "https://ha.local:8123".to_string(),
                    sync_mode: "manual".to_string(),
                    token_present: true,
                },
            };

            let value = serde_json::to_value(response).expect("connect response should serialize");

            assert_eq!(value["queued"], true);
            assert_eq!(value["enabled"], enabled);
            assert!(value.get("runtime").is_some());
            assert!(value.get("connected").is_none());
        }
    }

    #[test]
    fn runtime_public_response_contains_no_token() {
        let response = HassRuntimeConfigPublic {
            enabled: true,
            url: "https://ha.local:8123".to_string(),
            sync_mode: "manual".to_string(),
            token_present: true,
        };

        let value = serde_json::to_value(response).expect("runtime response should serialize");

        assert!(value.get("token").is_none());
        assert!(value.get("token_present").is_some());
    }
}
