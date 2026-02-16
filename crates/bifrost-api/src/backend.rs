use serde::{Deserialize, Serialize};
use uuid::Uuid;

use hue::api::{
    GroupedLightUpdate, LightUpdate, ResourceLink, RoomUpdate, Scene, SceneUpdate,
    ZigbeeDeviceDiscoveryUpdate,
};
use hue::stream::HueStreamLightsV2;

use crate::Client;
use crate::config::{HassServer, Z2mServer};
use crate::error::BifrostResult;

#[allow(clippy::large_enum_variant)]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum BackendRequest {
    LightUpdate(ResourceLink, LightUpdate),
    SensorEnabledUpdate(ResourceLink, bool),
    HassSync,
    /// Upsert a single entity from Home Assistant into the Hue resource DB (fetches HA state).
    HassUpsertEntity(String),
    /// Remove a single entity from the Hue resource DB (no HA call).
    HassRemoveEntity(String),
    /// Rebuild room metadata/assignments from the current UI config without HA requests.
    HassUpdateRooms,
    HassConnect,
    HassDisconnect,

    SceneCreate(ResourceLink, u32, Scene),
    SceneUpdate(ResourceLink, SceneUpdate),

    GroupedLightUpdate(ResourceLink, GroupedLightUpdate),

    RoomUpdate(ResourceLink, RoomUpdate),

    Delete(ResourceLink),

    EntertainmentStart(Uuid),
    EntertainmentFrame(HueStreamLightsV2),
    EntertainmentStop(),

    ZigbeeDeviceDiscovery(ResourceLink, ZigbeeDeviceDiscoveryUpdate),
}

impl Client {
    pub async fn post_backend(&self, name: &str, backend: Z2mServer) -> BifrostResult<()> {
        self.post(&format!("backend/z2m/{name}"), backend).await
    }

    pub async fn post_backend_hass(&self, name: &str, backend: HassServer) -> BifrostResult<()> {
        self.post(&format!("backend/hass/{name}"), backend).await
    }
}
