use std::collections::{BTreeSet, HashMap};
use std::fs::File;

use camino::Utf8PathBuf;
use chrono::{DateTime, Local, Utc};
use serde::{Deserialize, Serialize};
use url::Url;

use crate::error::{ApiError, ApiResult};

#[derive(Clone, Copy, Debug, Serialize, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum HassSensorKind {
    Motion,
    Contact,
    Ignore,
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
        pref.alias = alias.map(|x| x.trim().to_string()).filter(|x| !x.is_empty());
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

    #[must_use]
    pub fn should_include(&self, entity_id: &str, display_name: &str, available: bool) -> bool {
        if !self.include_unavailable && !available {
            return false;
        }

        let entity_id_lc = entity_id.to_ascii_lowercase();
        let name_lc = display_name.to_ascii_lowercase();

        // Explicit per-entity visibility overrides patterns/defaults.
        if let Some(visible) = self.entity_preferences.get(entity_id).and_then(|x| x.visible) {
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
    pub sensor_kind: Option<HassSensorKind>,
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
        let file = File::create(&self.file)?;
        serde_yml::to_writer(file, &self.config)?;
        Ok(())
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

    pub fn set_config_update(&mut self, update: HassRuntimeConfigUpdate) {
        self.config.enabled = update.enabled;
        self.config.url = update.url.trim().to_string();
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
        if self.config.url.trim().is_empty() {
            return Err(ApiError::service_error(
                "Home Assistant URL not set".to_string(),
            ));
        }
        Ok(Url::parse(self.config.url.trim())?)
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
#[serde(deny_unknown_fields)]
struct HassUiStateFile {
    #[serde(default)]
    config: HassUiConfig,
    #[serde(default)]
    patina: HassPatinaState,
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
#[serde(untagged)]
enum HassUiStateFileCompat {
    V2(HassUiStateFile),
    V1(HassUiConfig),
}

impl HassUiState {
    pub fn load(file: Utf8PathBuf) -> ApiResult<Self> {
        let (mut config, patina) = if file.is_file() {
            match File::open(&file).and_then(|fd| {
                serde_yml::from_reader::<_, HassUiStateFileCompat>(fd).map_err(std::io::Error::other)
            }) {
                Ok(HassUiStateFileCompat::V2(state)) => (state.config, state.patina),
                Ok(HassUiStateFileCompat::V1(config)) => (config, HassPatinaState::default()),
                Err(err) => {
                    log::warn!("Failed to parse {}, using defaults: {}", file, err);
                    (HassUiConfig::default(), HassPatinaState::default())
                }
            }
        } else {
            (HassUiConfig::default(), HassPatinaState::default())
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

        if !state.file.is_file() {
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
        patina.interactions_by_key.retain(|k, _| !k.trim().is_empty());
        let file = File::create(&self.file)?;
        let state = HassUiStateFile {
            config: cfg,
            patina,
        };
        serde_yml::to_writer(file, &state)?;
        Ok(())
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
        let interaction_component = u8::try_from((self.patina.interaction_count.min(5000) * 80) / 5000)
            .unwrap_or(80);
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
pub struct HassApplyResponse {
    pub applied: bool,
    pub removed_devices: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct HassResetBridgeResponse {
    pub reset: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct HassConnectResponse {
    pub connected: bool,
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
