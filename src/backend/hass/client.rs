use std::collections::HashMap;
use std::time::Duration;

use futures::{SinkExt, StreamExt};
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use tokio::net::TcpStream;
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

#[derive(Clone, Debug)]
pub struct HassStateChangedEvent {
    pub entity_id: String,
    pub new_state: Option<HassState>,
    pub old_state: Option<HassState>,
}

#[derive(Debug, Deserialize)]
struct HassWsEventEnvelope {
    #[serde(default)]
    pub event_type: String,
    pub data: HassWsEventData,
}

#[derive(Debug, Deserialize)]
struct HassWsEventData {
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
}

impl HassWs {
    async fn recv_json(&mut self) -> ApiResult<Option<HassWsIncoming>> {
        let Some(msg) = self.socket.next().await else {
            return Ok(None);
        };
        let msg = msg.map_err(ApiError::from)?;
        let Message::Text(text) = msg else {
            return Ok(Some(HassWsIncoming::Other));
        };
        Ok(Some(serde_json::from_str::<HassWsIncoming>(&text)?))
    }

    pub async fn next_state_changed(&mut self) -> ApiResult<Option<HassStateChangedEvent>> {
        while let Some(msg) = self.recv_json().await? {
            if let HassWsIncoming::Event { event } = msg {
                if event.event_type == "state_changed" {
                    return Ok(Some(HassStateChangedEvent {
                        entity_id: event.data.entity_id,
                        new_state: event.data.new_state,
                        old_state: event.data.old_state,
                    }));
                }
            }
        }
        Ok(None)
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
        Ok(response.json().await?)
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
        let template = r#"
{% for s in states if s.entity_id.startswith('light.') or s.entity_id.startswith('switch.') or s.entity_id.startswith('binary_sensor.') %}
{{ s.entity_id }}|{{ area_name(s.entity_id) or '' }}
{% endfor %}
"#;
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
        name: &str,
        snapshot_entities: Vec<String>,
    ) -> ApiResult<()> {
        let mut data = Map::new();
        data.insert("scene_id".to_string(), Value::String(scene_id.to_string()));
        data.insert("name".to_string(), Value::String(name.to_string()));
        data.insert(
            "snapshot_entities".to_string(),
            Value::Array(snapshot_entities.into_iter().map(Value::String).collect()),
        );
        self.call_service("scene", "create", "", data).await
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

        // Subscribe to state_changed events.
        let sub = serde_json::json!({
            "id": 1,
            "type": "subscribe_events",
            "event_type": "state_changed",
        });
        socket.send(Message::Text(sub.to_string().into())).await?;

        // Wait for subscribe result.
        loop {
            let Some(msg) = socket.next().await else {
                return Err(ApiError::service_error(format!(
                    "[{}] Home Assistant websocket closed during subscribe",
                    self.backend_name
                )));
            };
            let msg = msg.map_err(ApiError::from)?;
            if let Message::Text(text) = msg {
                let value: HassWsIncoming = serde_json::from_str(&text)?;
                if let HassWsIncoming::Result { id, success, error } = value {
                    if id == 1 && success {
                        break;
                    }
                    if id == 1 && !success {
                        return Err(ApiError::service_error(format!(
                            "[{}] Home Assistant subscribe_events failed: {}",
                            self.backend_name,
                            error.unwrap_or(Value::Null)
                        )));
                    }
                }
            }
        }

        Ok(HassWs { socket })
    }

    pub async fn set_entity_registry_disabled(
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
