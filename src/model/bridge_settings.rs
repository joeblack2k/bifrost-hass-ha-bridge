use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::{ApiError, ApiResult};
use crate::model::hass::{HassUiConfig, validate_hass_ui_config_fields};

pub const SETTINGS_KIND: &str = "bifrost-bridge-settings";
pub const SETTINGS_SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
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
        let object = value.as_object().ok_or_else(|| {
            ApiError::service_error("Bridge settings import must be a JSON object")
        })?;
        let is_envelope = object.contains_key("config")
            || object.contains_key("kind")
            || object.contains_key("schema_version");
        let mut config = if is_envelope {
            let config = object
                .get("config")
                .and_then(Value::as_object)
                .ok_or_else(|| {
                    ApiError::service_error(
                        "Bridge settings import envelope must contain an object config field",
                    )
                })?;
            validate_hass_ui_config_fields(config.keys().map(String::as_str))?;
            serde_json::from_value::<Self>(value)?.into_config()?
        } else {
            validate_hass_ui_config_fields(object.keys().map(String::as_str))?;
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
    use serde_json::{Value, json};

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

    #[test]
    fn import_rejects_incomplete_or_unknown_objects_instead_of_defaulting() {
        let complete =
            serde_json::to_value(HassUiConfig::default()).expect("config should serialize");
        assert!(
            BridgeSettingsExport::parse_value(json!({
                "kind": SETTINGS_KIND,
                "schema_version": SETTINGS_SCHEMA_VERSION,
            }))
            .is_err()
        );
        assert!(
            BridgeSettingsExport::parse_value(json!({
                "unrelated": true,
            }))
            .is_err()
        );
        assert!(
            BridgeSettingsExport::parse_value(json!({
                "rooms": [],
            }))
            .is_err()
        );
        assert!(
            BridgeSettingsExport::parse_value(json!({
                "config": {},
                "kind": SETTINGS_KIND,
                "schema_version": SETTINGS_SCHEMA_VERSION,
            }))
            .is_err()
        );
        assert!(
            BridgeSettingsExport::parse_value(json!({
                "config": {"unrelated": true},
                "kind": SETTINGS_KIND,
                "schema_version": SETTINGS_SCHEMA_VERSION,
            }))
            .is_err()
        );
        let mut unknown = complete
            .as_object()
            .expect("config should be an object")
            .clone();
        unknown.insert("unrelated".to_string(), json!(true));
        assert!(BridgeSettingsExport::parse_value(Value::Object(unknown)).is_err());
        let mut partial = complete
            .as_object()
            .expect("config should be an object")
            .clone();
        partial.remove("rooms");
        assert!(BridgeSettingsExport::parse_value(Value::Object(partial)).is_err());
        assert!(
            BridgeSettingsExport::parse_value(json!({
                "config": complete,
                "kind": SETTINGS_KIND,
                "schema_version": SETTINGS_SCHEMA_VERSION,
            }))
            .is_ok()
        );
    }
}
