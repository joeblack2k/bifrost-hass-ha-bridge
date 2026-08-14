use std::collections::HashMap;

use chrono::Utc;
use hue::error::{HueError, HueResult};
use hue::legacy_api::{ApiRule, ApiSchedule};
use serde_json::{Map, Value, json};

use crate::error::ApiResult;
use crate::resource::Resources;

fn object(value: Value, kind: &str) -> ApiResult<Map<String, Value>> {
    match value {
        Value::Object(value) => Ok(value),
        _ => Err(crate::error::ApiError::service_error(format!(
            "{kind} must be a JSON object"
        ))),
    }
}

fn legacy_now() -> Value {
    Value::String(Utc::now().format("%Y-%m-%dT%H:%M:%S").to_string())
}

fn normalize_rule(value: Value) -> ApiResult<Value> {
    let mut value = object(value, "rule")?;
    value
        .entry("name".to_string())
        .or_insert_with(|| Value::String(String::new()));
    value
        .entry("recycle".to_string())
        .or_insert_with(|| Value::Bool(false));
    value
        .entry("status".to_string())
        .or_insert_with(|| Value::String("enabled".to_string()));
    value
        .entry("conditions".to_string())
        .or_insert_with(|| Value::Array(vec![]));
    value
        .entry("actions".to_string())
        .or_insert_with(|| Value::Array(vec![]));
    value
        .entry("owner".to_string())
        .or_insert_with(|| json!(uuid::Uuid::nil()));
    value
        .entry("timestriggered".to_string())
        .or_insert_with(|| Value::Number(0.into()));
    value
        .entry("created".to_string())
        .or_insert_with(legacy_now);
    value
        .entry("lasttriggered".to_string())
        .or_insert_with(|| Value::String("none".to_string()));
    Ok(Value::Object(value))
}

fn normalize_schedule(value: Value) -> ApiResult<Value> {
    let mut value = object(value, "schedule")?;
    value
        .entry("recycle".to_string())
        .or_insert_with(|| Value::Bool(false));
    value
        .entry("name".to_string())
        .or_insert_with(|| Value::String(String::new()));
    value
        .entry("description".to_string())
        .or_insert_with(|| Value::String(String::new()));
    value
        .entry("command".to_string())
        .or_insert_with(|| Value::Object(Map::new()));
    value
        .entry("created".to_string())
        .or_insert_with(legacy_now);
    value.entry("starttime".to_string()).or_insert(Value::Null);
    value
        .entry("time".to_string())
        .or_insert_with(|| Value::String(String::new()));
    value
        .entry("localtime".to_string())
        .or_insert_with(|| Value::String(String::new()));
    value
        .entry("status".to_string())
        .or_insert_with(|| Value::String("enabled".to_string()));
    Ok(Value::Object(value))
}

fn merge(current: Value, patch: Value, kind: &str) -> ApiResult<Value> {
    let mut current = object(current, kind)?;
    let patch = object(patch, kind)?;
    current.extend(patch);
    Ok(Value::Object(current))
}

pub fn get_rules(res: &Resources) -> ApiResult<HashMap<u32, ApiRule>> {
    res.legacy_rules()
        .into_iter()
        .map(|(id, value)| Ok((id, serde_json::from_value(value)?)))
        .collect()
}

pub fn get_rule(res: &Resources, id: u32) -> HueResult<ApiRule> {
    res.legacy_rule(id)
        .ok_or(HueError::V1NotFound(id))
        .and_then(|value| serde_json::from_value(value).map_err(HueError::from))
}

pub fn create_rule(res: &mut Resources, value: Value) -> ApiResult<u32> {
    let id = res.next_legacy_rule_id()?;
    res.put_legacy_rule(id, normalize_rule(value)?);
    Ok(id)
}

pub fn update_rule(res: &mut Resources, id: u32, value: Value) -> ApiResult<()> {
    let current = res.legacy_rule(id).ok_or(HueError::V1NotFound(id))?;
    res.put_legacy_rule(id, normalize_rule(merge(current, value, "rule")?)?);
    Ok(())
}

pub fn delete_rule(res: &mut Resources, id: u32) -> ApiResult<()> {
    if res.delete_legacy_rule(id) {
        Ok(())
    } else {
        Err(HueError::V1NotFound(id).into())
    }
}

pub fn get_schedules(res: &Resources) -> ApiResult<HashMap<u32, ApiSchedule>> {
    res.legacy_schedules()
        .into_iter()
        .map(|(id, value)| Ok((id, serde_json::from_value(value)?)))
        .collect()
}

pub fn get_schedule(res: &Resources, id: u32) -> HueResult<ApiSchedule> {
    res.legacy_schedule(id)
        .ok_or(HueError::V1NotFound(id))
        .and_then(|value| serde_json::from_value(value).map_err(HueError::from))
}

pub fn create_schedule(res: &mut Resources, value: Value) -> ApiResult<u32> {
    let id = res.next_legacy_schedule_id()?;
    res.put_legacy_schedule(id, normalize_schedule(value)?);
    Ok(id)
}

pub fn update_schedule(res: &mut Resources, id: u32, value: Value) -> ApiResult<()> {
    let current = res.legacy_schedule(id).ok_or(HueError::V1NotFound(id))?;
    res.put_legacy_schedule(id, normalize_schedule(merge(current, value, "schedule")?)?);
    Ok(())
}

pub fn delete_schedule(res: &mut Resources, id: u32) -> ApiResult<()> {
    if res.delete_legacy_schedule(id) {
        Ok(())
    } else {
        Err(HueError::V1NotFound(id).into())
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{normalize_rule, normalize_schedule};

    #[test]
    fn normalizes_minimal_rule_without_losing_user_fields() {
        let value = normalize_rule(json!({"name": "test", "actions": [{"address": "/lights/1"}]}))
            .expect("rule");
        assert_eq!(value["name"], "test");
        assert_eq!(value["status"], "enabled");
        assert_eq!(value["conditions"], json!([]));
        assert_eq!(value["actions"][0]["address"], "/lights/1");
    }

    #[test]
    fn normalizes_minimal_schedule_without_panicking_on_missing_fields() {
        let value =
            normalize_schedule(json!({"name": "test", "command": {"address": "/lights/1"}}))
                .expect("schedule");
        assert_eq!(value["name"], "test");
        assert_eq!(value["status"], "enabled");
        assert!(value["created"].is_string());
    }
}
