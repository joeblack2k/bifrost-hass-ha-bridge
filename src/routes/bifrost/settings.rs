use axum::Router;
use axum::extract::State;
use axum::routing::{get, post};
use serde_json::Value;

use bifrost_api::backend::BackendRequest;

use crate::model::bridge_settings::BridgeSettingsExport;
use crate::routes::bifrost::BifrostApiResult;
use crate::routes::extractor::Json;
use crate::server::appstate::AppState;

async fn export_settings(
    State(state): State<AppState>,
) -> BifrostApiResult<Json<BridgeSettingsExport>> {
    let ui = state.hass_ui();
    let config = ui.lock().await.config_normalized();
    Ok(Json(BridgeSettingsExport::from_config(config)))
}

async fn import_settings(
    State(state): State<AppState>,
    Json(value): Json<Value>,
) -> BifrostApiResult<Json<BridgeSettingsExport>> {
    let config = BridgeSettingsExport::parse_value(value)?;
    let export = BridgeSettingsExport::from_config(config.clone());

    {
        let ui = state.hass_ui();
        let mut lock = ui.lock().await;
        let previous = lock.config.clone();
        lock.set_config(config);
        if let Err(error) = lock.persist_and_log("Imported bridge settings") {
            lock.config = previous;
            return Err(error.into());
        }
    }

    {
        let res = state.res.lock().await;
        res.backend_request(BackendRequest::HassSync)?;
    }

    Ok(Json(export))
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/hass/settings/export", get(export_settings))
        .route("/hass/settings/import", post(import_settings))
}
