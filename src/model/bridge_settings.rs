use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::{ApiError, ApiResult};
use crate::model::hass::HassUiConfig;

pub const SETTINGS_KIND: &str = "bifrost-bridge-settings";
pub const SETTINGS_SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct BridgeSettingsExport {
    pub kind: String,
    pub schema_version: u32,
    pub config: HassUiConfig,
}

impl BridgeSettingsExport {
    #[must_use]
    pub fn from_config(config: HassUiConfig) -> Self {
        Self {
            kind: SETTINGS_KIND.to_string(),
            schema_version: SETTINGS_SCHEMA_VERSION,
            config,
        }
    }

    pub fn into_config(self) -> ApiResult<HassUiConfig> {
        if self.kind != SETTINGS_KIND {
            return Err(ApiError::service_error(format!(
                "Unsupported bridge settings kind: {}",
                self.kind
            )));
        }
        if self.schema_version != SETTINGS_SCHEMA_VERSION {
            return Err(ApiError::service_error(format!(
                "Unsupported bridge settings schema version: {}",
                self.schema_version
            )));
        }

        let mut config = self.config;
        config.normalize();
        Ok(config)
    }

    pub fn parse_value(value: Value) -> ApiResult<HassUiConfig> {
        let mut config = if value.get("config").is_some() {
            serde_json::from_value::<Self>(value)?.into_config()?
        } else {
            serde_json::from_value::<HassUiConfig>(value)?
        };
        config.normalize();
        Ok(config)
    }
}

#[cfg(test)]
mod tests {
    use super::{BridgeSettingsExport, SETTINGS_KIND, SETTINGS_SCHEMA_VERSION};
    use crate::model::hass::HassUiConfig;
    use serde_json::json;

    #[test]
    fn export_round_trips_normalized_bridge_settings() {
        let mut config = HassUiConfig::default();
        config.rooms[0].name = " Home ".to_string();
        config.normalize();

        let exported = BridgeSettingsExport::from_config(config.clone());
        assert_eq!(exported.kind, SETTINGS_KIND);
        assert_eq!(exported.schema_version, SETTINGS_SCHEMA_VERSION);
        assert_eq!(
            exported.into_config().expect("settings should import"),
            config
        );
    }

    #[test]
    fn import_accepts_raw_config_but_rejects_unknown_envelope() {
        let config = HassUiConfig::default();
        let raw = serde_json::to_value(&config).expect("config should serialize");
        assert!(BridgeSettingsExport::parse_value(raw).is_ok());
        assert!(
            BridgeSettingsExport::parse_value(json!({
                "kind": "other",
                "schema_version": SETTINGS_SCHEMA_VERSION,
                "config": config,
            }))
            .is_err()
        );
    }
}
