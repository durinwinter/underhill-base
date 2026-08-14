use std::{collections::HashMap, sync::Arc};

use tokio::{sync::RwLock, task::JoinHandle};
use tracing::{info, warn};

#[derive(Debug, Clone)]
pub struct PeaEndpointSpec {
    pub pea_id: String,
    pub pea_name: String,
    pub endpoint_url: String,
    pub namespace_uri: String,
    pub root_path: String,
}

pub trait PeaAddressSpaceBuilder: Send + Sync + 'static {
    fn build_manifest(&self, spec: &PeaEndpointSpec) -> anyhow::Result<Vec<String>>;
}

#[derive(Debug)]
pub struct PeaEndpointInstance {
    pub spec: PeaEndpointSpec,
    pub node_manifest: Vec<String>,
    server_task: JoinHandle<anyhow::Result<()>>,
}

#[derive(Clone, Default)]
pub struct PeaEndpointHost {
    instances: Arc<RwLock<HashMap<String, PeaEndpointInstance>>>,
}

impl PeaEndpointHost {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn spawn_endpoint(
        &self,
        spec: PeaEndpointSpec,
        builder: Arc<dyn PeaAddressSpaceBuilder>,
    ) -> anyhow::Result<()> {
        let manifest = builder.build_manifest(&spec)?;
        let pea_id = spec.pea_id.clone();
        let endpoint_url = spec.endpoint_url.clone();

        let server_task = tokio::spawn(async move {
            // Skeleton only: wire concrete OPC UA server startup here.
            // Intended implementation:
            // 1) create per-PEA OPC UA server instance
            // 2) register InformationLabel/Diagnostics/DataAssemblies/Services/Simulation
            // 3) map runtime values into node updates
            info!(
                "PEA OPC UA skeleton server active: {} at {}",
                pea_id, endpoint_url
            );
            futures_util::future::pending::<()>().await;
            #[allow(unreachable_code)]
            Ok(())
        });

        let mut instances = self.instances.write().await;
        if instances.contains_key(&spec.pea_id) {
            return Err(anyhow::anyhow!(
                "PEA endpoint already registered: {}",
                spec.pea_id
            ));
        }
        instances.insert(
            spec.pea_id.clone(),
            PeaEndpointInstance {
                spec,
                node_manifest: manifest,
                server_task,
            },
        );
        Ok(())
    }

    pub async fn stop_endpoint(&self, pea_id: &str) -> anyhow::Result<()> {
        let instance = {
            let mut instances = self.instances.write().await;
            instances.remove(pea_id)
        };
        let Some(instance) = instance else {
            return Err(anyhow::anyhow!("PEA endpoint not found: {pea_id}"));
        };
        instance.server_task.abort();
        let _ = instance.server_task.await;
        info!("Stopped PEA endpoint skeleton: {}", pea_id);
        Ok(())
    }

    pub async fn list_endpoints(&self) -> Vec<PeaEndpointSpec> {
        let instances = self.instances.read().await;
        instances
            .values()
            .map(|instance| instance.spec.clone())
            .collect()
    }

    pub async fn shutdown_all(&self) {
        let keys = {
            let instances = self.instances.read().await;
            instances.keys().cloned().collect::<Vec<_>>()
        };
        for pea_id in keys {
            if let Err(err) = self.stop_endpoint(&pea_id).await {
                warn!("Failed stopping endpoint {}: {}", pea_id, err);
            }
        }
    }
}
