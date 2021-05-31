use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};

use runtime::image_service_client::ImageServiceClient;
use runtime::runtime_service_client::RuntimeServiceClient;
use runtime::*;

pub mod runtime {
    include!(concat!(env!("OUT_DIR"), "/runtime.v1alpha2.rs"));
}

pub struct CRIClient {
    addr: tonic::transport::Endpoint,
    runtime_service_client: RuntimeServiceClient<tonic::transport::Channel>,
    image_service_client: ImageServiceClient<tonic::transport::Channel>,
}

impl CRIClient {
    pub async fn connect<D>(dst: D) -> Result<Self, Box<dyn std::error::Error>>
    where
        D: std::convert::TryInto<tonic::transport::Endpoint>,
        D::Error: std::error::Error + Send + Sync + 'static,
    {
        let endpoint: tonic::transport::Endpoint = dst.try_into()?;
        let runtime_service_client = RuntimeServiceClient::connect(endpoint.clone()).await?;
        let image_service_client = ImageServiceClient::connect(endpoint.clone()).await?;

        Ok(Self {
            addr: endpoint,
            runtime_service_client: runtime_service_client,
            image_service_client: image_service_client,
        })
    }

    pub async fn get_and_print_version(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        info!("Connecting to peer at {:?}", self.addr);

        let version_req = VersionRequest {
            version: String::from("0.0.1-unnamed-rust-cri-client"),
        };
        let request = tonic::Request::new(version_req);
        let response = self.runtime_service_client.version(request).await?;

        let msg = response.into_inner();

        info!("Peer is {}@{}", msg.runtime_name, msg.runtime_version);
        Ok(())
    }

    pub async fn pod_exists(&mut self, name: &String) -> Result<bool, Box<dyn std::error::Error>> {
        let request = ListPodSandboxRequest::default();

        info!("Getting sandbox status {:?}", request);
        let response = self
            .runtime_service_client
            .list_pod_sandbox(request)
            .await?;
        let msg = response.into_inner();
        info!("peer responded {:?}", msg);
        let exists = msg.items.into_iter().any(|pod_sandbox| {
            pod_sandbox
                .metadata
                .filter(|metadata| metadata.name == *name)
                .is_some()
        });
        Ok(exists)
    }

    pub fn pod_sandbox_config(&self, name: &String, uid: &String) -> PodSandboxConfig {
        let mut security_context = LinuxSandboxSecurityContext::default();

        security_context.namespace_options = Some(NamespaceOption {
            network: NamespaceMode::Node as i32,
            pid: NamespaceMode::Container as i32,
            ipc: NamespaceMode::Pod as i32,
            target_id: String::from(""),
        });

        let mut pod_sandbox_config = PodSandboxConfig::default();
        pod_sandbox_config.metadata = Some(PodSandboxMetadata {
            name: name.clone(),
            uid: uid.clone(),
            namespace: name.clone(),
            attempt: 0,
        });
        pod_sandbox_config.linux = Some(LinuxPodSandboxConfig {
            cgroup_parent: String::from(""),
            security_context: Some(security_context),
            sysctls: HashMap::default(),
        });
        pod_sandbox_config
    }

    pub async fn run_pod_sandbox(
        &mut self,
        pod_sandbox_config: &PodSandboxConfig,
    ) -> Result<String, Box<dyn std::error::Error>> {
        let request = RunPodSandboxRequest {
            config: Some(pod_sandbox_config.clone()),
            runtime_handler: String::from(""),
        };

        info!("Creating pod sandbox {:?}", request);
        let response = self.runtime_service_client.run_pod_sandbox(request).await?;
        let msg = response.into_inner();
        info!("peer responded {:?}", msg);
        Ok(msg.pod_sandbox_id)
    }

    pub async fn create_container(
        &mut self,
        image_id: &String,
        ctr: &crate::config::types::Container,
        pod_sandbox_id: &String,
        pod_sandbox_config: &PodSandboxConfig,
    ) -> Result<String, Box<dyn std::error::Error>> {
        let mut container_config = ContainerConfig::default();
        container_config.args = ctr.args.clone();
        container_config.command = vec![ctr.command.clone()];
        container_config.metadata = Some(ContainerMetadata {
            name: ctr.name.clone(),
            attempt: 0,
        });
        container_config.image = Some(ImageSpec {
            image: image_id.clone(),
            annotations: HashMap::default(),
        });

        let request = CreateContainerRequest {
            pod_sandbox_id: pod_sandbox_id.clone(),
            config: Some(container_config),
            sandbox_config: Some(pod_sandbox_config.clone()),
        };
        info!("Creating container {:?}", request);
        let response = self
            .runtime_service_client
            .create_container(request)
            .await?;
        let msg = response.into_inner();
        info!("peer responded {:?}", msg);
        Ok(msg.container_id)
    }

    pub async fn start_container(
        &mut self,
        container_id: &String,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let request = StartContainerRequest {
            container_id: container_id.clone(),
        };
        info!("Starting container {:?}", request);
        let response = self.runtime_service_client.start_container(request).await?;
        let msg = response.into_inner();
        info!("peer responded {:?}", msg);
        Ok(())
    }

    pub async fn poll_container_status(
        &mut self,
        container_id: &String,
    ) -> Result<ContainerState, Box<dyn std::error::Error>> {
        let res = self
            .runtime_service_client
            .container_status(ContainerStatusRequest {
                container_id: container_id.clone(),
                verbose: true,
            })
            .await?;
        let msg = res.into_inner();
        info!("{:?}", msg);

        if let Some(status) = msg.status {
            if let Some(state) = ContainerState::from_i32(status.state) {
                return Ok(state);
            }
        }

        Ok(ContainerState::ContainerUnknown)
    }

    pub async fn stop_pod(
        &mut self,
        pod_sandbox_id: &String,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let res = self
            .runtime_service_client
            .stop_pod_sandbox(StopPodSandboxRequest {
                pod_sandbox_id: pod_sandbox_id.clone(),
            })
            .await?;
        let msg = res.into_inner();
        info!("{:?}", msg);

        Ok(())
    }

    pub async fn destroy_pod(
        &mut self,
        pod_sandbox_id: &String,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let res = self
            .runtime_service_client
            .remove_pod_sandbox(RemovePodSandboxRequest {
                pod_sandbox_id: pod_sandbox_id.clone(),
            })
            .await?;
        let msg = res.into_inner();
        info!("{:?}", msg);

        Ok(())
    }

    pub async fn pull_image(
        &mut self,
        image_id: &String,
        pod_sandbox_config: &PodSandboxConfig,
    ) -> Result<String, Box<dyn std::error::Error>> {
        let request = PullImageRequest {
            image: Some(ImageSpec {
                image: image_id.clone(),
                annotations: HashMap::default(),
            }),
            auth: None,
            sandbox_config: Some(pod_sandbox_config.clone()),
        };

        info!("Pulling image {:?}", request);
        let response = self.image_service_client.pull_image(request).await?;
        let msg = response.into_inner();
        info!("peer responded {:?}", msg);
        Ok(msg.image_ref)
    }
}
