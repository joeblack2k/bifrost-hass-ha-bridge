use std::collections::BTreeSet;

use hue::api::{DeviceArchetype, RType, Resource, ResourceLink, SmartScene, Zone};
use serde::Deserialize;
use serde_json::Value;
use uuid::Uuid;

use crate::error::{ApiError, ApiResult};
use crate::resource::Resources;
use crate::routes::clip::{ApiV2Result, V2Reply};
use crate::server::appstate::AppState;

#[derive(Debug, Deserialize, Default)]
struct ZoneUpdate {
    #[serde(default)]
    metadata: Option<ZoneMetadataUpdate>,
    #[serde(default)]
    children: Option<BTreeSet<ResourceLink>>,
    #[serde(default)]
    services: Option<BTreeSet<ResourceLink>>,
}

#[derive(Debug, Deserialize, Default)]
struct ZoneMetadataUpdate {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    archetype: Option<DeviceArchetype>,
}

fn object_without_type(value: Value) -> ApiResult<Value> {
    let Value::Object(mut object) = value else {
        return Err(ApiError::service_error(
            "topology resource must be a JSON object",
        ));
    };
    object.remove("type");
    Ok(Value::Object(object))
}

fn validate_links(res: &Resources, links: impl IntoIterator<Item = ResourceLink>) -> ApiResult<()> {
    for link in links {
        res.get_resource(&link)?;
    }
    Ok(())
}

fn validate_service_group(res: &Resources, value: &Value) -> ApiResult<()> {
    let Some(services) = value.get("services") else {
        return Ok(());
    };
    let services = services
        .as_array()
        .ok_or_else(|| ApiError::service_error("service_group.services must be an array"))?;
    for service in services {
        let link: ResourceLink = serde_json::from_value(service.clone())?;
        res.get_resource(&link)?;
    }
    Ok(())
}

pub async fn post_zone(state: &AppState, req: Value) -> ApiV2Result {
    let zone: Zone = serde_json::from_value(req)?;
    let link = RType::Zone.link_to(Uuid::new_v4());

    let mut lock = state.res.lock().await;
    validate_links(
        &lock,
        zone.children.iter().chain(zone.services.iter()).copied(),
    )?;
    lock.add(&link, Resource::Zone(zone))?;
    drop(lock);

    V2Reply::ok(link)
}

pub async fn post_service_group(state: &AppState, req: Value) -> ApiV2Result {
    let value = object_without_type(req)?;
    let link = RType::ServiceGroup.link_to(Uuid::new_v4());

    let mut lock = state.res.lock().await;
    validate_service_group(&lock, &value)?;
    lock.add(&link, Resource::ServiceGroup(value))?;
    drop(lock);

    V2Reply::ok(link)
}

pub async fn post_smart_scene(state: &AppState, req: Value) -> ApiV2Result {
    let smart_scene: SmartScene = serde_json::from_value(req)?;
    let link = RType::SmartScene.link_to(Uuid::new_v4());

    let mut lock = state.res.lock().await;
    lock.get_resource(&smart_scene.group)?;
    lock.add(&link, Resource::SmartScene(smart_scene))?;
    drop(lock);

    V2Reply::ok(link)
}

pub async fn put_zone(state: &AppState, rlink: ResourceLink, put: Value) -> ApiV2Result {
    let update: ZoneUpdate = serde_json::from_value(put)?;
    let mut lock = state.res.lock().await;
    lock.get::<Zone>(&rlink)?;

    if let Some(children) = &update.children {
        validate_links(&lock, children.iter().copied())?;
    }
    if let Some(services) = &update.services {
        validate_links(&lock, services.iter().copied())?;
    }

    lock.update::<Zone>(&rlink.rid, |zone| {
        if let Some(metadata) = &update.metadata {
            if let Some(name) = &metadata.name {
                zone.metadata.name.clone_from(name);
            }
            if let Some(archetype) = &metadata.archetype {
                zone.metadata.archetype.clone_from(archetype);
            }
        }
        if let Some(children) = &update.children {
            zone.children.clone_from(children);
        }
        if let Some(services) = &update.services {
            zone.services.clone_from(services);
        }
    })?;
    drop(lock);

    V2Reply::ok(rlink)
}

pub async fn put_service_group(state: &AppState, rlink: ResourceLink, put: Value) -> ApiV2Result {
    let patch = object_without_type(put)?;
    let mut lock = state.res.lock().await;
    let current = match lock.get_resource(&rlink)?.obj {
        Resource::ServiceGroup(value) => value,
        _ => return Err(ApiError::UpdateNotYetSupported(RType::ServiceGroup)),
    };
    let mut next = current;
    let (Value::Object(next_object), Value::Object(patch_object)) = (&mut next, patch) else {
        return Err(ApiError::service_error(
            "service_group resource must be an object",
        ));
    };
    next_object.extend(patch_object);
    validate_service_group(&lock, &next)?;

    lock.update_service_group(&rlink, next)?;
    drop(lock);

    V2Reply::ok(rlink)
}

pub async fn put_smart_scene(state: &AppState, rlink: ResourceLink, put: Value) -> ApiV2Result {
    let patch = object_without_type(put)?;
    let mut lock = state.res.lock().await;
    let current = lock.get::<SmartScene>(&rlink)?.clone();
    let mut next = serde_json::to_value(current)?;
    let (Value::Object(next_object), Value::Object(patch_object)) = (&mut next, patch) else {
        return Err(ApiError::service_error(
            "smart_scene resource must be an object",
        ));
    };
    next_object.extend(patch_object);
    let next: SmartScene = serde_json::from_value(next)?;
    lock.get_resource(&next.group)?;
    lock.update::<SmartScene>(&rlink.rid, |smart_scene| *smart_scene = next.clone())?;
    drop(lock);

    V2Reply::ok(rlink)
}

pub async fn delete(state: &AppState, rlink: ResourceLink) -> ApiV2Result {
    let mut lock = state.res.lock().await;
    lock.get_resource(&rlink)?;
    lock.delete(&rlink)?;
    drop(lock);
    V2Reply::ok(rlink)
}

#[cfg(test)]
mod tests {
    use hue::api::{RType, ResourceLink};
    use serde_json::json;
    use uuid::Uuid;

    use super::{object_without_type, validate_service_group};
    use crate::model::state::State;
    use crate::resource::Resources;
    use hue::version::SwVersion;

    #[test]
    fn strips_protocol_tag_before_storing_raw_service_group() {
        let value = object_without_type(json!({
            "type": "service_group",
            "services": []
        }))
        .expect("object");

        assert_eq!(value, json!({"services": []}));
    }

    #[test]
    fn rejects_service_group_links_that_are_not_in_the_store() {
        let resources = Resources::new(SwVersion::default(), State::new());
        let link = ResourceLink::new(Uuid::nil(), RType::Light);

        assert!(validate_service_group(&resources, &json!({"services": [link]})).is_err());
    }
}
