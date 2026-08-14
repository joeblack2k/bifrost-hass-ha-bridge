use std::collections::{HashMap, VecDeque};
use std::time::Duration;

use futures::{SinkExt, StreamExt};
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use tokio::net::TcpStream;
use tokio::time::timeout;
use tokio_tungstenite::MaybeTlsStream;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{WebSocketStream, connect_async};
use url::Url;

use bifrost_api::config::HassServer;

use crate::error::{ApiError, ApiResult};

#[derive(Clone, Debug, Deserialize)]
pub struct HassState {
    pub entity_id: String,
    pub state: String,
    #[serde(default)]
    pub attributes: Map<String, Value>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct HassCoreConfig {
    #[serde(default)]
    #[serde(alias = "time_zone")]
    pub timezone: Option<String>,
    #[serde(default)]
    pub latitude: Option<f64>,
    #[serde(default)]
    pub longitude: Option<f64>,
}

pub struct HassClient {
    backend_name: String,
    base_url: Url,
    http: reqwest::Client,
    token: Option<String>,
}

#[derive(Debug, Deserialize)]
struct HassWsEventEnvelope {
    #[serde(default)]
    pub event_type: String,
    pub data: Value,
}

#[derive(Clone, Debug)]
pub struct HassEvent {
    pub event_type: String,
    pub data: Value,
}

#[derive(Clone, Debug)]
pub struct HassStateChanged {
    pub entity_id: String,
    pub new_state: Option<HassState>,
}

impl HassEvent {
    pub fn state_changed(&self) -> Option<HassStateChanged> {
        if self.event_type != "state_changed" {
            return None;
        }
        let data = serde_json::from_value::<HassStateChangedData>(self.data.clone()).ok()?;
        let _ = data.old_state;
        Some(HassStateChanged {
            entity_id: data.entity_id,
            new_state: data.new_state,
        })
    }
}

#[derive(Debug, Deserialize)]
struct HassStateChangedData {
    pub entity_id: String,
    pub new_state: Option<HassState>,
    pub old_state: Option<HassState>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
enum HassWsIncoming {
    #[serde(rename = "auth_required")]
    AuthRequired,
    #[serde(rename = "auth_ok")]
    AuthOk,
    #[serde(rename = "auth_invalid")]
    AuthInvalid,
    #[serde(rename = "result")]
    Result {
        id: u64,
        success: bool,
        #[serde(default)]
        error: Option<Value>,
    },
    #[serde(rename = "event")]
    Event { event: HassWsEventEnvelope },
    #[serde(other)]
    Other,
}

pub struct HassWs {
    socket: WebSocketStream<MaybeTlsStream<TcpStream>>,
    pending: VecDeque<HassWsIncoming>,
}

impl HassWs {
    async fn recv_socket_json(&mut self) -> ApiResult<Option<HassWsIncoming>> {
        let Some(msg) = self.socket.next().await else {
            return Ok(None);
        };
        let msg = msg.map_err(ApiError::from)?;
        match msg {
            Message::Text(text) => match serde_json::from_str::<HassWsIncoming>(&text) {
                Ok(value) => Ok(Some(value)),
                Err(err) => {
                    log::debug!("Ignoring malformed Home Assistant websocket message: {err}");
                    Ok(Some(HassWsIncoming::Other))
                }
            },
            Message::Close(_) => Ok(None),
            Message::Ping(payload) => {
                self.socket.send(Message::Pong(payload)).await?;
                Ok(Some(HassWsIncoming::Other))
            }
            _ => Ok(Some(HassWsIncoming::Other)),
        }
    }

    async fn recv_json(&mut self) -> ApiResult<Option<HassWsIncoming>> {
        if let Some(msg) = self.pending.pop_front() {
            return Ok(Some(msg));
        }
        self.recv_socket_json().await
    }

    pub async fn check_liveness(&mut self, wait: Duration) -> ApiResult<()> {
        self.socket.send(Message::Ping(Vec::new().into())).await?;
        let msg = timeout(wait, self.socket.next())
            .await
            .map_err(|_| ApiError::service_error("Home Assistant websocket liveness timeout"))?
            .ok_or_else(|| {
                ApiError::service_error("Home Assistant websocket closed during liveness check")
            })?
            .map_err(ApiError::from)?;

        match msg {
            Message::Text(text) => {
                if let Ok(value) = serde_json::from_str::<HassWsIncoming>(&text) {
                    self.pending.push_back(value);
                } else {
                    log::debug!("Ignoring malformed Home Assistant websocket liveness message");
                }
            }
            Message::Close(_) => {
                return Err(ApiError::service_error(
                    "Home Assistant websocket closed during liveness check",
                ));
            }
            Message::Ping(payload) => {
                self.socket.send(Message::Pong(payload)).await?;
            }
            _ => {}
        }
        Ok(())
    }

    pub async fn next_event(&mut self) -> ApiResult<Option<HassEvent>> {
        while let Some(msg) = self.recv_json().await? {
            if let HassWsIncoming::Event { event } = msg {
                return Ok(Some(HassEvent {
                    event_type: event.event_type,
                    data: event.data,
                }));
            }
        }
        Ok(None)
    }

    async fn subscribe_events(
        &mut self,
        id: u64,
        event_type: &str,
        required: bool,
        wait: Duration,
    ) -> ApiResult<bool> {
        let sub = serde_json::json!({
            "id": id,
            "type": "subscribe_events",
            "event_type": event_type,
        });
        self.socket
            .send(Message::Text(sub.to_string().into()))
            .await?;

        let result = timeout(wait, async {
            loop {
                let Some(msg) = self.recv_socket_json().await? else {
                    return Err(ApiError::service_error(
                        "Home Assistant websocket closed during subscribe",
                    ));
                };
                match msg {
                    HassWsIncoming::Result {
                        id: result_id,
                        success,
                        error,
                    } if result_id == id => {
                        if success {
                            return Ok(true);
                        }
                        if required {
                            return Err(ApiError::service_error(format!(
                                "Home Assistant subscribe_events failed: {}",
                                error.unwrap_or(Value::Null)
                            )));
                        }
                        return Ok(false);
                    }
                    HassWsIncoming::Event { event } => {
                        self.pending.push_back(HassWsIncoming::Event { event });
                    }
                    // A result for a timed-out optional subscription is stale. Do not put it
                    // back into pending or the next subscription would consume it forever.
                    HassWsIncoming::Result { .. }
                    | HassWsIncoming::AuthRequired
                    | HassWsIncoming::AuthOk
                    | HassWsIncoming::AuthInvalid
                    | HassWsIncoming::Other => {}
                }
            }
        })
        .await;

        match result {
            Ok(result) => result,
            Err(_) if required => Err(ApiError::service_error(
                "Home Assistant websocket state subscription timed out",
            )),
            Err(_) => Ok(false),
        }
    }
}

#[derive(Debug, Serialize)]
struct HassTemplateRequest<'a> {
    template: &'a str,
}

impl HassClient {
    const DEFAULT_TOKEN_ENV: &'static str = "HASS_TOKEN";
    const DEFAULT_TIMEOUT_SECS: u64 = 10;

    pub fn new(backend_name: &str, server: &HassServer) -> ApiResult<Self> {
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(Self::DEFAULT_TIMEOUT_SECS))
            .build()?;

        Ok(Self {
            backend_name: backend_name.to_string(),
            base_url: server.url.clone(),
            http,
            token: None,
        })
    }

    pub fn load_token_from_env(&mut self, server: &HassServer) -> ApiResult<()> {
        let token_env = server
            .token_env
            .as_deref()
            .unwrap_or(Self::DEFAULT_TOKEN_ENV);
        let token = std::env::var(token_env).map_err(|_| {
            ApiError::service_error(format!(
                "[{}] Missing Home Assistant token env var {}",
                self.backend_name, token_env
            ))
        })?;
        if token.trim().is_empty() {
            return Err(ApiError::service_error(format!(
                "[{}] Empty Home Assistant token in env var {}",
                self.backend_name, token_env
            )));
        }
        self.token = Some(token);
        Ok(())
    }

    pub fn set_runtime(&mut self, base_url: Url, token: Option<String>) -> ApiResult<()> {
        self.base_url = base_url;
        self.token = token
            .map(|x| x.trim().to_string())
            .filter(|x| !x.is_empty());
        if self.token.is_none() {
            return Err(ApiError::service_error(format!(
                "[{}] Home Assistant token not configured",
                self.backend_name
            )));
        }
        Ok(())
    }

    pub fn set_base_url(&mut self, base_url: Url) {
        self.base_url = base_url;
    }

    fn endpoint_url(&self, endpoint: &str) -> ApiResult<Url> {
        let base = if self.base_url.path().is_empty() {
            format!("{}/", self.base_url)
        } else {
            self.base_url.to_string()
        };
        let base = Url::parse(&base)?;
        Ok(base.join(endpoint.trim_start_matches('/'))?)
    }

    fn token(&self) -> ApiResult<&str> {
        self.token.as_deref().ok_or_else(|| {
            ApiError::service_error(format!(
                "[{}] Home Assistant token not initialized",
                self.backend_name
            ))
        })
    }

    async fn check_status(
        &self,
        response: reqwest::Response,
        action: &str,
    ) -> ApiResult<reqwest::Response> {
        if response.status().is_success() {
            return Ok(response);
        }

        let status = response.status();
        let body = response.text().await.unwrap_or_else(|_| String::new());

        let details = if body.is_empty() {
            format!("{status}")
        } else {
            format!("{status}: {body}")
        };

        let err = if status == StatusCode::UNAUTHORIZED {
            format!(
                "[{}] Home Assistant unauthorized during {}. Verify HASS_TOKEN",
                self.backend_name, action
            )
        } else {
            format!(
                "[{}] Home Assistant error during {}: {}",
                self.backend_name, action, details
            )
        };

        Err(ApiError::service_error(err))
    }

    pub async fn get_states(&self) -> ApiResult<Vec<HassState>> {
        let url = self.endpoint_url("/api/states")?;
        let response = self.http.get(url).bearer_auth(self.token()?).send().await?;
        let response = self.check_status(response, "GET /api/states").await?;
        let value = response.json::<Value>().await?;
        parse_states(value, &self.backend_name)
    }

    pub async fn get_core_config(&self) -> ApiResult<HassCoreConfig> {
        let url = self.endpoint_url("/api/config")?;
        let response = self.http.get(url).bearer_auth(self.token()?).send().await?;
        let response = self.check_status(response, "GET /api/config").await?;
        Ok(response.json().await?)
    }

    pub async fn get_state(&self, entity_id: &str) -> ApiResult<HassState> {
        let url = self.endpoint_url(&format!("/api/states/{entity_id}"))?;
        let response = self.http.get(url).bearer_auth(self.token()?).send().await?;
        let response = self
            .check_status(response, &format!("GET /api/states/{entity_id}"))
            .await?;
        Ok(response.json().await?)
    }

    pub async fn get_entity_area(&self, entity_id: &str) -> ApiResult<Option<String>> {
        // Keep this lightweight (single-entity). Full `get_entity_areas()` is used on full sync.
        let template = format!("{{{{ area_name('{entity_id}') or '' }}}}");
        let url = self.endpoint_url("/api/template")?;
        let response = self
            .http
            .post(url)
            .bearer_auth(self.token()?)
            .json(&HassTemplateRequest {
                template: &template,
            })
            .send()
            .await?;
        let response = self
            .check_status(response, "POST /api/template (single entity area)")
            .await?;
        let body = response.text().await?;
        let area = body.trim();
        if area.is_empty() {
            return Ok(None);
        }
        Ok(Some(area.to_string()))
    }

    pub async fn get_entity_areas(&self) -> ApiResult<HashMap<String, String>> {
        // Returns one line per entity in format: entity_id|area_name
        let template = r"
{% for s in states if s.entity_id.startswith('light.') or s.entity_id.startswith('switch.') or s.entity_id.startswith('binary_sensor.') or s.entity_id.startswith('sensor.') or s.entity_id.startswith('event.') or s.entity_id.startswith('scene.') %}
{{ s.entity_id }}|{{ area_name(s.entity_id) or '' }}
{% endfor %}
";
        let url = self.endpoint_url("/api/template")?;
        let response = self
            .http
            .post(url)
            .bearer_auth(self.token()?)
            .json(&HassTemplateRequest { template })
            .send()
            .await?;
        let response = self
            .check_status(response, "POST /api/template (entity area sync)")
            .await?;
        let body = response.text().await?;
        let mut map = HashMap::new();
        for line in body.lines().map(str::trim).filter(|x| !x.is_empty()) {
            let Some((entity_id, area_name)) = line.split_once('|') else {
                continue;
            };
            let entity_id = entity_id.trim();
            let area_name = area_name.trim();
            if entity_id.is_empty() {
                continue;
            }
            if !area_name.is_empty() {
                map.insert(entity_id.to_string(), area_name.to_string());
            }
        }
        Ok(map)
    }

    pub async fn call_service(
        &self,
        domain: &str,
        service: &str,
        entity_id: &str,
        mut data: Map<String, Value>,
    ) -> ApiResult<()> {
        let url = self.endpoint_url(&format!("/api/services/{domain}/{service}"))?;
        if !entity_id.trim().is_empty() {
            data.insert(
                "entity_id".to_string(),
                Value::String(entity_id.to_string()),
            );
        }
        let payload = Value::Object(data);

        let response = self
            .http
            .post(url)
            .bearer_auth(self.token()?)
            .json(&payload)
            .send()
            .await?;
        let _response = self
            .check_status(response, &format!("POST /api/services/{domain}/{service}"))
            .await?;
        Ok(())
    }

    pub async fn create_scene_snapshot(
        &self,
        scene_id: &str,
        snapshot_entities: Vec<String>,
    ) -> ApiResult<()> {
        let mut data = Map::new();
        data.insert("scene_id".to_string(), Value::String(scene_id.to_string()));
        data.insert(
            "snapshot_entities".to_string(),
            Value::Array(snapshot_entities.into_iter().map(Value::String).collect()),
        );
        self.call_service("scene", "create", "", data).await
    }

    pub async fn delete_scene_snapshot(&self, entity_id: &str) -> ApiResult<()> {
        self.call_service("scene", "delete", entity_id, Map::new())
            .await
    }

    pub async fn turn_on_scene(&self, entity_id: &str) -> ApiResult<()> {
        self.call_service("scene", "turn_on", entity_id, Map::new())
            .await
    }

    fn ws_endpoint_url(&self) -> ApiResult<Url> {
        let mut url = self.endpoint_url("/api/websocket")?;
        let scheme = match url.scheme() {
            "https" => "wss",
            _ => "ws",
        };
        url.set_scheme(scheme).map_err(|_| {
            ApiError::service_error(format!(
                "[{}] Failed to convert HA url scheme for websocket",
                self.backend_name
            ))
        })?;
        Ok(url)
    }

    pub async fn subscribe_state_changed(&self) -> ApiResult<HassWs> {
        timeout(
            Duration::from_secs(Self::DEFAULT_TIMEOUT_SECS),
            self.subscribe_state_changed_inner(),
        )
        .await
        .map_err(|_| {
            ApiError::service_error(format!(
                "[{}] Home Assistant websocket handshake timed out",
                self.backend_name
            ))
        })?
    }

    async fn subscribe_state_changed_inner(&self) -> ApiResult<HassWs> {
        let ws_url = self.ws_endpoint_url()?;
        let (mut socket, _response) = connect_async(ws_url.as_str()).await?;

        // Consume initial auth challenge.
        let _ = socket.next().await;

        let auth = serde_json::json!({
            "type": "auth",
            "access_token": self.token()?,
        });
        socket.send(Message::Text(auth.to_string().into())).await?;

        // Wait for auth_ok.
        loop {
            let Some(msg) = socket.next().await else {
                return Err(ApiError::service_error(format!(
                    "[{}] Home Assistant websocket closed during auth",
                    self.backend_name
                )));
            };
            let msg = msg.map_err(ApiError::from)?;
            if let Message::Text(text) = msg {
                let value: HassWsIncoming = serde_json::from_str(&text)?;
                match value {
                    HassWsIncoming::AuthOk => break,
                    HassWsIncoming::AuthInvalid => {
                        return Err(ApiError::service_error(format!(
                            "[{}] Home Assistant websocket auth failed (check token)",
                            self.backend_name
                        )));
                    }
                    _ => {}
                }
            }
        }

        // Keep state synchronization strict. Home Assistant event entities surface button
        // updates through state_changed; generic integration buses do not carry a deterministic
        // entity-to-button mapping and must not make the bridge handshake fragile.
        let mut ws = HassWs {
            socket,
            pending: VecDeque::new(),
        };
        ws.subscribe_events(1, "state_changed", true, Duration::from_secs(5))
            .await?;
        Ok(ws)
    }

    pub async fn set_entity_registry_disabled(
        &self,
        entity_id: &str,
        disabled: bool,
    ) -> ApiResult<()> {
        timeout(
            Duration::from_secs(Self::DEFAULT_TIMEOUT_SECS),
            self.set_entity_registry_disabled_inner(entity_id, disabled),
        )
        .await
        .map_err(|_| {
            ApiError::service_error(format!(
                "[{}] Home Assistant entity registry websocket timed out",
                self.backend_name
            ))
        })?
    }

    async fn set_entity_registry_disabled_inner(
        &self,
        entity_id: &str,
        disabled: bool,
    ) -> ApiResult<()> {
        let ws_url = self.ws_endpoint_url()?;
        let (mut socket, _response) = connect_async(ws_url.as_str()).await?;

        let first = socket
            .next()
            .await
            .ok_or_else(|| {
                ApiError::service_error(format!(
                    "[{}] Missing websocket auth challenge",
                    self.backend_name
                ))
            })?
            .map_err(ApiError::from)?;
        let _ = first;

        let auth = serde_json::json!({
            "type": "auth",
            "access_token": self.token()?,
        });
        socket.send(Message::Text(auth.to_string().into())).await?;

        let auth_reply = socket
            .next()
            .await
            .ok_or_else(|| {
                ApiError::service_error(format!(
                    "[{}] Missing websocket auth reply",
                    self.backend_name
                ))
            })?
            .map_err(ApiError::from)?;
        if let Message::Text(text) = auth_reply {
            let value: Value = serde_json::from_str(&text)?;
            if value.get("type").and_then(Value::as_str) != Some("auth_ok") {
                return Err(ApiError::service_error(format!(
                    "[{}] Home Assistant websocket auth failed: {}",
                    self.backend_name, value
                )));
            }
        }

        let req = serde_json::json!({
            "id": 1,
            "type": "config/entity_registry/update",
            "entity_id": entity_id,
            "disabled_by": if disabled { Value::String("user".to_string()) } else { Value::Null },
        });
        socket.send(Message::Text(req.to_string().into())).await?;

        while let Some(msg) = socket.next().await {
            let msg = msg?;
            if let Message::Text(text) = msg {
                let value: Value = serde_json::from_str(&text)?;
                if value.get("id").and_then(Value::as_u64) == Some(1) {
                    if value.get("success").and_then(Value::as_bool) == Some(true) {
                        return Ok(());
                    }
                    return Err(ApiError::service_error(format!(
                        "[{}] HA entity registry update failed: {}",
                        self.backend_name, value
                    )));
                }
            }
        }

        Err(ApiError::service_error(format!(
            "[{}] No websocket response for entity registry update",
            self.backend_name
        )))
    }
}

fn parse_states(value: Value, backend_name: &str) -> ApiResult<Vec<HassState>> {
    let Some(values) = value.as_array() else {
        return Err(ApiError::service_error(format!(
            "[{backend_name}] Home Assistant /api/states response was not an array"
        )));
    };

    let mut states = Vec::with_capacity(values.len());
    for (index, value) in values.iter().enumerate() {
        match serde_json::from_value::<HassState>(value.clone()) {
            Ok(state) => states.push(state),
            Err(err) => log::warn!(
                "[{backend_name}] Skipping malformed Home Assistant state at index {index}: {err}"
            ),
        }
    }
    Ok(states)
}

#[cfg(test)]
mod tests {
    use super::{HassEvent, HassWs, parse_states};
    use futures::{SinkExt, StreamExt};
    use serde_json::json;
    use std::collections::VecDeque;
    use std::time::Duration;
    use tokio::net::TcpListener;
    use tokio_tungstenite::tungstenite::Message;

    #[test]
    fn full_state_sync_skips_one_malformed_entity() {
        let states = parse_states(
            json!([
                {"entity_id": "light.good", "state": "on", "attributes": {}},
                {"entity_id": 42, "state": "broken"},
                {"entity_id": "scene.good", "state": "unknown", "attributes": {}}
            ]),
            "test",
        )
        .expect("array response should be accepted");
        assert_eq!(
            states
                .iter()
                .map(|state| state.entity_id.as_str())
                .collect::<Vec<_>>(),
            ["light.good", "scene.good"]
        );
    }

    #[test]
    fn state_changed_keeps_entity_id_for_deletions() {
        let event = HassEvent {
            event_type: "state_changed".to_string(),
            data: json!({
                "entity_id": "scene.deleted",
                "old_state": {"entity_id": "scene.deleted", "state": "unknown"},
                "new_state": null
            }),
        };
        let change = event.state_changed().expect("state change");
        assert_eq!(change.entity_id, "scene.deleted");
        assert!(change.new_state.is_none());
    }

    #[test]
    fn optional_subscription_timeout_preserves_interleaved_events() {
        tokio::runtime::Runtime::new()
            .expect("runtime")
            .block_on(async {
                let listener = TcpListener::bind("127.0.0.1:0").await.expect("listener");
                let address = listener.local_addr().expect("address");
                let server = tokio::spawn(async move {
                    let (stream, _) = listener.accept().await.expect("accept");
                    let mut socket = tokio_tungstenite::accept_async(stream)
                        .await
                        .expect("websocket");
                    let _ = socket.next().await;
                    socket
                        .send(Message::Text(
                            serde_json::json!({
                                "type": "event",
                                "event": {
                                    "event_type": "zha_event",
                                    "data": {"entity_id": "event.remote", "command": "press"}
                                }
                            })
                            .to_string()
                            .into(),
                        ))
                        .await
                        .expect("event");
                    tokio::time::sleep(Duration::from_millis(100)).await;
                });

                let (socket, _) = tokio_tungstenite::connect_async(format!("ws://{address}"))
                    .await
                    .expect("connect");
                let mut ws = HassWs {
                    socket,
                    pending: VecDeque::new(),
                };

                assert!(
                    !ws.subscribe_events(2, "zha_event", false, Duration::from_millis(20))
                        .await
                        .expect("optional timeout")
                );
                let event = ws
                    .next_event()
                    .await
                    .expect("event read")
                    .expect("queued event");
                assert_eq!(event.event_type, "zha_event");
                server.await.expect("server");
            });
    }

    #[test]
    fn required_subscription_ignores_malformed_messages_and_surfaces_rejection() {
        tokio::runtime::Runtime::new()
            .expect("runtime")
            .block_on(async {
                let listener = TcpListener::bind("127.0.0.1:0").await.expect("listener");
                let address = listener.local_addr().expect("address");
                let server = tokio::spawn(async move {
                    let (stream, _) = listener.accept().await.expect("accept");
                    let mut socket = tokio_tungstenite::accept_async(stream)
                        .await
                        .expect("websocket");
                    let _ = socket.next().await;
                    socket
                        .send(Message::Text("not-json".to_string().into()))
                        .await
                        .expect("malformed message");
                    socket
                        .send(Message::Text(
                            serde_json::json!({
                                "type": "result",
                                "id": 1,
                                "success": false,
                                "error": {"code": "unsupported"}
                            })
                            .to_string()
                            .into(),
                        ))
                        .await
                        .expect("rejection");
                });

                let (socket, _) = tokio_tungstenite::connect_async(format!("ws://{address}"))
                    .await
                    .expect("connect");
                let mut ws = HassWs {
                    socket,
                    pending: VecDeque::new(),
                };

                assert!(
                    ws.subscribe_events(1, "state_changed", true, Duration::from_secs(1))
                        .await
                        .is_err()
                );
                server.await.expect("server");
            });
    }
}
