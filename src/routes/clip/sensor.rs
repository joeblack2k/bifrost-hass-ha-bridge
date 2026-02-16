use serde_json::Value;

use bifrost_api::backend::BackendRequest;
use hue::api::{Motion, RType, ResourceLink};

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
    match rlink.rtype {
        RType::Motion => {
            let _ = lock.get::<Motion>(&rlink)?;
            lock.update::<Motion>(&rlink.rid, |motion| {
                motion.enabled = enabled;
            })?;
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
        }
        _ => return Err(ApiError::UpdateNotYetSupported(rlink.rtype)),
    }

    lock.backend_request(BackendRequest::SensorEnabledUpdate(rlink, enabled))?;
    drop(lock);

    V2Reply::ok(rlink)
}
