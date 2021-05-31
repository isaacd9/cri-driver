#[macro_use]
extern crate log;
mod config;

use crate::config::*;
use rand::Rng;
use runtime::image_service_client::ImageServiceClient;
use runtime::runtime_service_client::RuntimeServiceClient;
use runtime::*;
use signal_hook::consts::signal::*;
use signal_hook::flag as signal_flag;
use std::collections::HashMap;
use std::env;
use std::fs::File;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

pub mod runtime {
    include!(concat!(env!("OUT_DIR"), "/runtime.v1alpha2.rs"));
}

const ADDR: &'static str = "http://localhost:3000";

struct PodManager {
    runtime_service_client: RuntimeServiceClient<tonic::transport::Channel>,
    image_service_client: ImageServiceClient<tonic::transport::Channel>,
}

impl PodManager {
    pub async fn get_and_print_version(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        info!("Connecting to peer at {}", ADDR);

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

    fn pod_sandbox_config(&self, name: &String, uid: &String) -> PodSandboxConfig {
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

    async fn create_container(
        &mut self,
        image_id: &String,
        container_name: &String,
        pod_sandbox_id: &String,
        pod_sandbox_config: &PodSandboxConfig,
    ) -> Result<String, Box<dyn std::error::Error>> {
        let mut container_config = ContainerConfig::default();
        container_config.metadata = Some(ContainerMetadata {
            name: String::from(container_name),
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

    async fn start_container(
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

    async fn poll_container_status(
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

    async fn stop_pod(
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

    async fn destroy_pod(
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

    async fn pull_image(
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

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config_file = env::args().nth(1).unwrap();
    let pod = read_yaml_config(File::open(config_file)?)?;

    simple_logger::SimpleLogger::new()
        .with_level(log::LevelFilter::Info)
        .init()
        .unwrap();

    info!("pod: {:?}", pod);

    let term = Arc::new(AtomicUsize::new(0));
    const SIGTERM_U: usize = SIGTERM as usize;
    const SIGINT_U: usize = SIGINT as usize;
    const SIGQUIT_U: usize = SIGQUIT as usize;
    signal_flag::register_usize(SIGTERM, Arc::clone(&term), SIGTERM_U)?;
    signal_flag::register_usize(SIGINT, Arc::clone(&term), SIGINT_U)?;
    signal_flag::register_usize(SIGQUIT, Arc::clone(&term), SIGQUIT_U)?;

    let runtime_service_client = RuntimeServiceClient::connect(ADDR).await?;
    let image_service_client = ImageServiceClient::connect(ADDR).await?;

    let mut m = PodManager {
        runtime_service_client: runtime_service_client,
        image_service_client: image_service_client,
    };

    m.get_and_print_version().await?;

    if m.pod_exists(&pod.name).await? {
        let e = format!("the pod \"{}\" already exists", &pod.name);
        return Err(e.into());
    }

    let mut rng = rand::thread_rng();
    let uid = format!("{}-{}", &pod.name, rng.gen::<u128>());
    //let config = read_yaml_config(stdin);
    //println!("{:?}", config);

    let pod_sandbox_config = m.pod_sandbox_config(&pod.name, &uid);
    let pod_sandbox_id = m.run_pod_sandbox(&pod_sandbox_config).await?;

    let mut container_ids = vec![];
    for container in pod.containers {
        // Per container config
        let image_ref = m.pull_image(&container.image, &pod_sandbox_config).await?;
        let container_id = m
            .create_container(
                // TODO: see if we can replace with image_ref
                &container.image,
                &container.name,
                &pod_sandbox_id,
                &pod_sandbox_config,
            )
            .await?;
        container_ids.push(container_id)
    }

    for container_id in &container_ids {
        m.start_container(&container_id).await?;
    }

    'outer: loop {
        match term.load(Ordering::Relaxed) {
            0 => {
                for container_id in &container_ids {
                    match m.poll_container_status(&container_id).await? {
                        ContainerState::ContainerExited => {
                            info!("container exited");
                            break 'outer;
                        }
                        _ => (),
                    }
                }
                std::thread::sleep(std::time::Duration::from_millis(1000));
            }
            SIGTERM_U => {
                warn!("Terminating on the TERM signal");
                break;
            }
            SIGINT_U => {
                warn!("Terminating on the INT signal");
                break;
            }
            SIGQUIT_U => {
                warn!("Terminating on the QUIT signal");
                break;
            }
            _ => unreachable!(),
        }
    }

    info!("cleaning up");
    m.stop_pod(&pod_sandbox_id).await?;
    m.destroy_pod(&pod_sandbox_id).await?;

    info!("cleaned up");
    Ok(())
}
