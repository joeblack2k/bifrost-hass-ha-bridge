use serde_json::Value;
use uuid::Uuid;

use hue::api::{BehaviorInstance, BehaviorInstanceUpdate, BehaviorScript, RType, ResourceLink};

use crate::routes::clip::{ApiV2Result, V2Reply};
use crate::server::appstate::AppState;

fn validate_instance(
    instance: &BehaviorInstance,
    res: &crate::resource::Resources,
) -> crate::error::ApiResult<()> {
    let script = ResourceLink::new(instance.script_id, RType::BehaviorScript);
    res.get::<BehaviorScript>(&script)?;
    for dependee in &instance.dependees {
        res.get_resource(&dependee.target)?;
    }
    Ok(())
}

pub async fn post_behavior_instance(state: &AppState, req: Value) -> ApiV2Result {
    let instance: BehaviorInstance = serde_json::from_value(req)?;
    let link = RType::BehaviorInstance.link_to(Uuid::new_v4());

    let mut lock = state.res.lock().await;
    validate_instance(&instance, &lock)?;
    lock.add(&link, hue::api::Resource::BehaviorInstance(instance))?;
    drop(lock);

    V2Reply::ok(link)
}

pub async fn put_behavior_instance(
    state: &AppState,
    rlink: ResourceLink,
    put: Value,
) -> ApiV2Result {
    let update: BehaviorInstanceUpdate = serde_json::from_value(put)?;
    let mut lock = state.res.lock().await;
    lock.get::<BehaviorInstance>(&rlink)?;
    lock.update::<BehaviorInstance>(&rlink.rid, |instance| *instance += update.clone())?;
    drop(lock);

    V2Reply::ok(rlink)
}

pub async fn delete_behavior_instance(state: &AppState, rlink: ResourceLink) -> ApiV2Result {
    let mut lock = state.res.lock().await;
    lock.get_resource(&rlink)?;
    lock.delete(&rlink)?;
    drop(lock);

    V2Reply::ok(rlink)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::validate_instance;
    use crate::model::state::State;
    use crate::resource::Resources;
    use hue::api::{BehaviorInstance, BehaviorInstanceMetadata};
    use hue::version::SwVersion;

    #[test]
    fn rejects_an_instance_when_its_script_is_not_available() {
        let instance: BehaviorInstance = serde_json::from_value(json!({
            "dependees": [],
            "enabled": true,
            "last_error": null,
            "metadata": {"name": "test"},
            "script_id": "00000000-0000-0000-0000-000000000000",
            "status": null,
            "configuration": {}
        }))
        .expect("behavior instance");
        let resources = Resources::new(SwVersion::default(), State::new());

        assert!(validate_instance(&instance, &resources).is_err());
        assert_eq!(
            instance.metadata,
            BehaviorInstanceMetadata {
                name: "test".into()
            }
        );
    }
}
