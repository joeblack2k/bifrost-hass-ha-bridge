use serde_json::Value;

use bifrost_api::backend::BackendRequest;
use hue::api::{LightLevel, Motion, RType, ResourceLink, Temperature};

use crate::error::ApiError;
use crate::routes::V2Reply;
use crate::routes::clip::ApiV2Result;
use crate::server::appstate::AppState;

fn parse_enabled(put: &Value) -> Result<bool, ApiError> {
    if let Some(enabled) = put.get("enabled").and_then(Value::as_bool) {
        return Ok(enabled);
    }
    if let Some(enabled) = put
        .get("enabled")
        .and_then(Value::as_object)
        .and_then(|x| x.get("enabled"))
        .and_then(Value::as_bool)
    {
        return Ok(enabled);
    }

    Err(ApiError::UpdateNotYetSupported(RType::Motion))
}

pub async fn put_sensor(state: &AppState, rlink: ResourceLink, put: Value) -> ApiV2Result {
    let enabled = parse_enabled(&put)?;

    let mut lock = state.res.lock().await;
    let notify_backend = match rlink.rtype {
        RType::Motion => {
            let _ = lock.get::<Motion>(&rlink)?;
            lock.update::<Motion>(&rlink.rid, |motion| {
                motion.enabled = enabled;
            })?;
            true
        }
        RType::Contact => {
            let record = lock.get_resource(&rlink)?;
            let mut raw = match record.obj {
                hue::api::Resource::Contact(value) => value,
                _ => return Err(ApiError::UpdateNotYetSupported(RType::Contact)),
            };
            if let Some(map) = raw.as_object_mut() {
                map.insert("enabled".to_string(), Value::Bool(enabled));
            }
            let _ = lock.delete(&rlink);
            lock.add(&rlink, hue::api::Resource::Contact(raw))?;
            true
        }
        RType::Temperature => {
            lock.update::<Temperature>(&rlink.rid, |temperature| {
                temperature.enabled = enabled;
            })?;
            true
        }
        RType::LightLevel => {
            lock.update::<LightLevel>(&rlink.rid, |light_level| {
                light_level.enabled = enabled;
            })?;
            true
        }
        _ => return Err(ApiError::UpdateNotYetSupported(rlink.rtype)),
    };

    if notify_backend {
        lock.backend_request(BackendRequest::SensorEnabledUpdate(rlink, enabled))?;
    }
    drop(lock);

    V2Reply::ok(rlink)
}
