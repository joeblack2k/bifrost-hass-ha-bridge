use std::collections::HashMap;

use serde_json::{Value, json};

use hue::api::{Device, Resource, ResourceLink};
use hue::legacy_api::ApiSensor;

use crate::error::ApiResult;
use crate::resource::Resources;

fn sensor_from_device(
    res: &Resources,
    owner: ResourceLink,
    sensor_type: &str,
    config: Value,
    state: Value,
    capabilities: Value,
) -> ApiResult<ApiSensor> {
    let device = res.get::<Device>(&owner).ok();
    let product = device.map(|device| &device.product_data);
    let name = device
        .map(|device| device.metadata.name.clone())
        .unwrap_or_else(|| "Home Assistant sensor".to_string());

    Ok(ApiSensor {
        sensor_type: sensor_type.to_string(),
        config,
        name,
        state,
        manufacturername: product.map_or_else(
            || "Home Assistant".to_string(),
            |data| data.manufacturer_name.clone(),
        ),
        modelid: product.map_or_else(|| "hass-sensor".to_string(), |data| data.model_id.clone()),
        swversion: product
            .map_or_else(|| "1.0.0".to_string(), |data| data.software_version.clone()),
        swupdate: None,
        uniqueid: None,
        diversityid: None,
        productname: product.map(|data| data.product_name.clone()),
        recycle: Some(false),
        capabilities,
    })
}

fn project_resource(res: &Resources, id: u32, record: &Resource) -> ApiResult<Option<ApiSensor>> {
    let sensor = match record {
        Resource::Motion(motion) => sensor_from_device(
            res,
            motion.owner,
            "ZLLPresence",
            json!({"on": motion.enabled}),
            motion.motion.clone(),
            Value::Null,
        )?,
        Resource::Contact(value) => {
            let owner = value
                .get("owner")
                .cloned()
                .map(serde_json::from_value::<ResourceLink>)
                .transpose()?;
            let Some(owner) = owner else {
                return Ok(None);
            };
            sensor_from_device(
                res,
                owner,
                "ZLLContact",
                json!({"on": value.get("enabled").and_then(Value::as_bool).unwrap_or(true)}),
                value.get("contact").cloned().unwrap_or(Value::Null),
                Value::Null,
            )?
        }
        Resource::Temperature(temperature) => sensor_from_device(
            res,
            temperature.owner,
            "ZLLTemperature",
            json!({"on": temperature.enabled}),
            temperature.temperature.clone(),
            Value::Null,
        )?,
        Resource::LightLevel(light_level) => sensor_from_device(
            res,
            light_level.owner,
            "ZLLLightLevel",
            json!({"on": light_level.enabled}),
            light_level.light.clone(),
            Value::Null,
        )?,
        _ => return Ok(None),
    };

    log::trace!("Projected Hue v1 sensor {id}");
    Ok(Some(sensor))
}

pub fn get_sensors(res: &Resources) -> ApiResult<HashMap<u32, ApiSensor>> {
    let mut sensors = HashMap::new();

    for record in res.get_resources() {
        let Some(id) = res.get_id_v1_index(record.id).ok() else {
            continue;
        };
        if let Some(sensor) = project_resource(res, id, &record.obj)? {
            sensors.insert(id, sensor);
        }
    }

    // Hue clients commonly probe the built-in daylight sensor even when no physical
    // daylight sensor exists. Keep this compatibility entry without hiding imported sensors.
    sensors
        .entry(1)
        .or_insert_with(ApiSensor::builtin_daylight_sensor);

    Ok(sensors)
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use uuid::Uuid;

    use super::project_resource;
    use hue::api::{Device, DeviceArchetype, DeviceProductData, Metadata, Motion, RType, Resource};
    use hue::version::SwVersion;

    #[test]
    fn projects_motion_as_v1_sensor() {
        let owner = RType::Device.link_to(Uuid::NAMESPACE_DNS);
        let device = Device {
            product_data: DeviceProductData::hue_bridge_v2(&SwVersion::default()),
            metadata: Metadata::new(DeviceArchetype::UnknownArchetype, "Motion sensor"),
            services: Default::default(),
            usertest: None,
            identify: None,
        };
        let mut state = crate::model::state::State::new();
        state.insert(owner.rid, Resource::Device(device));
        let resources = crate::resource::Resources::new(SwVersion::default(), state);
        let sensor = project_resource(
            &resources,
            4,
            &Resource::Motion(Motion {
                enabled: true,
                owner,
                motion: json!({"motion": true}),
                sensitivity: json!({}),
            }),
        )
        .expect("projection")
        .expect("sensor");

        assert_eq!(sensor.sensor_type, "ZLLPresence");
        assert_eq!(sensor.name, "Motion sensor");
        assert_eq!(sensor.state["motion"], true);
    }
}
