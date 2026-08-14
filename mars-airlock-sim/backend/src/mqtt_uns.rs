use std::time::Duration;

use rumqttc::{AsyncClient, MqttOptions, QoS};
use serde_json::Value;
use tokio::time;
use tracing::{info, warn};

#[derive(Clone)]
pub struct MqttUnsPublisher {
    client: AsyncClient,
    qos: QoS,
    retain: bool,
}

impl MqttUnsPublisher {
    pub async fn from_env() -> Option<Self> {
        let broker = std::env::var("UNS_MQTT_BROKER").ok()?;
        if broker.trim().is_empty() {
            return None;
        }
        let (host, port) = parse_broker(&broker);
        let client_id = std::env::var("UNS_MQTT_CLIENT_ID")
            .unwrap_or_else(|_| "underhill-uns-publisher".to_string());
        let mut options = MqttOptions::new(client_id, host.clone(), port);
        options.set_keep_alive(Duration::from_secs(30));

        if let Ok(username) = std::env::var("UNS_MQTT_USERNAME") {
            let password = std::env::var("UNS_MQTT_PASSWORD").unwrap_or_default();
            options.set_credentials(username, password);
        }

        let (client, mut eventloop) = AsyncClient::new(options, 200);
        tokio::spawn(async move {
            loop {
                if let Err(err) = eventloop.poll().await {
                    warn!("MQTT UNS eventloop error: {err}");
                    time::sleep(Duration::from_secs(1)).await;
                }
            }
        });

        info!("MQTT UNS publisher connected to {}:{}", host, port);
        Some(Self {
            client,
            qos: QoS::AtLeastOnce,
            retain: false,
        })
    }

    pub async fn publish_json(&self, topic: &str, payload: &Value) -> anyhow::Result<()> {
        self.client
            .publish(topic, self.qos, self.retain, payload.to_string())
            .await
            .map_err(|err| anyhow::anyhow!("mqtt publish failed for {topic}: {err}"))
    }
}

fn parse_broker(input: &str) -> (String, u16) {
    let trimmed = input.trim();
    let stripped = trimmed.strip_prefix("mqtt://").unwrap_or(trimmed);
    if let Some((host, port_raw)) = stripped.rsplit_once(':') {
        if let Ok(port) = port_raw.parse::<u16>() {
            return (host.to_string(), port);
        }
    }
    (stripped.to_string(), 1883)
}
