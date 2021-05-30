#[macro_use]
extern crate log;
mod config;

use crate::config::read_yaml_config;
use rand::Rng;
use runtime::image_service_client::ImageServiceClient;
use runtime::runtime_service_client::RuntimeServiceClient;
use runtime::*;
use signal_hook::consts::signal::*;
use signal_hook::flag as signal_flag;
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

pub mod runtime {
    include!(concat!(env!("OUT_DIR"), "/runtime.v1alpha2.rs"));
}

async fn get_and_print_version(
    client: &mut RuntimeServiceClient<tonic::transport::Channel>,
) -> Result<(), Box<dyn std::error::Error>> {
    info!("Connecting to peer at {}", ADDR);

    let version_req = VersionRequest {
        version: String::from("0.0.1-unnamed-rust-cri-client"),
    };
    let request = tonic::Request::new(version_req);
    let response = client.version(request).await?;

    let msg = response.into_inner();

    info!("Peer is {}@{}", msg.runtime_name, msg.runtime_version);
    Ok(())
}

async fn pod_exists(
    name: &String,
    client: &mut RuntimeServiceClient<tonic::transport::Channel>,
) -> Result<bool, Box<dyn std::error::Error>> {
    let request = ListPodSandboxRequest::default();

    info!("Getting sandbox status {:?}", request);
    let response = client.list_pod_sandbox(request).await?;
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

fn pod_sandbox_config(name: &String, uid: &String) -> PodSandboxConfig {
    let mut security_context = LinuxSandboxSecurityContext::default();

    security_context.namespace_options = Some(NamespaceOption {
        network: NamespaceMode::Node as i32,
        pid: NamespaceMode::Container as i32,
        ipc: NamespaceMode::Pod as i32,
        target_id: String::from(""),
    });

    let mut pod_sandbox_config = PodSandboxConfig::default();
    pod_sandbox_config.metadata = Some(PodSandboxMetadata {
        name: String::from(name),
        uid: uid.clone(),
        namespace: String::from("my-test-name-namespace"),
        attempt: 0,
    });
    pod_sandbox_config.linux = Some(LinuxPodSandboxConfig {
        cgroup_parent: String::from(""),
        security_context: Some(security_context),
        sysctls: HashMap::default(),
    });
    pod_sandbox_config
}

async fn run_pod_sandbox(
    pod_sandbox_config: &PodSandboxConfig,
    client: &mut RuntimeServiceClient<tonic::transport::Channel>,
) -> Result<String, Box<dyn std::error::Error>> {
    let request = RunPodSandboxRequest {
        config: Some(pod_sandbox_config.clone()),
        runtime_handler: String::from(""),
    };

    info!("Creating pod sandbox {:?}", request);
    let response = client.run_pod_sandbox(request).await?;
    let msg = response.into_inner();
    info!("peer responded {:?}", msg);
    Ok(msg.pod_sandbox_id)
}

async fn create_containers(
    image_id: &String,
    pod_sandbox_id: &String,
    pod_sandbox_config: &PodSandboxConfig,
    client: &mut RuntimeServiceClient<tonic::transport::Channel>,
) -> Result<String, Box<dyn std::error::Error>> {
    let mut container_config = ContainerConfig::default();
    container_config.metadata = Some(ContainerMetadata {
        name: String::from("my-container"),
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
    let response = client.create_container(request).await?;
    let msg = response.into_inner();
    info!("peer responded {:?}", msg);
    Ok(msg.container_id)
}

async fn start_containers(
    container_id: &String,
    client: &mut RuntimeServiceClient<tonic::transport::Channel>,
) -> Result<(), Box<dyn std::error::Error>> {
    let request = StartContainerRequest {
        container_id: container_id.clone(),
    };
    info!("Starting container {:?}", request);
    let response = client.start_container(request).await?;
    let msg = response.into_inner();
    info!("peer responded {:?}", msg);
    Ok(())
}

async fn poll_container_status(
    container_id: &String,
    client: &mut RuntimeServiceClient<tonic::transport::Channel>,
) -> Result<ContainerState, Box<dyn std::error::Error>> {
    let res = client
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
    pod_sandbox_id: &String,
    client: &mut RuntimeServiceClient<tonic::transport::Channel>,
) -> Result<(), Box<dyn std::error::Error>> {
    let res = client
        .stop_pod_sandbox(StopPodSandboxRequest {
            pod_sandbox_id: pod_sandbox_id.clone(),
        })
        .await?;
    let msg = res.into_inner();
    info!("{:?}", msg);

    Ok(())
}

async fn destroy_pod(
    pod_sandbox_id: &String,
    client: &mut RuntimeServiceClient<tonic::transport::Channel>,
) -> Result<(), Box<dyn std::error::Error>> {
    let res = client
        .remove_pod_sandbox(RemovePodSandboxRequest {
            pod_sandbox_id: pod_sandbox_id.clone(),
        })
        .await?;
    let msg = res.into_inner();
    info!("{:?}", msg);

    Ok(())
}

async fn pull_image(
    image_id: &String,
    pod_sandbox_config: &PodSandboxConfig,
    client: &mut ImageServiceClient<tonic::transport::Channel>,
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
    let response = client.pull_image(request).await?;
    let msg = response.into_inner();
    info!("peer responded {:?}", msg);
    Ok(msg.image_ref)
}

const ADDR: &'static str = "http://localhost:3000";
const IMAGE: &'static str = "gcr.io/google_containers/pause-amd64:3.0";
const NAME: &'static str = "a nice pod 2";

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    simple_logger::SimpleLogger::new()
        .with_level(log::LevelFilter::Info)
        .init()
        .unwrap();

    let term = Arc::new(AtomicUsize::new(0));
    const SIGTERM_U: usize = SIGTERM as usize;
    const SIGINT_U: usize = SIGINT as usize;
    const SIGQUIT_U: usize = SIGQUIT as usize;
    signal_flag::register_usize(SIGTERM, Arc::clone(&term), SIGTERM_U)?;
    signal_flag::register_usize(SIGINT, Arc::clone(&term), SIGINT_U)?;
    signal_flag::register_usize(SIGQUIT, Arc::clone(&term), SIGQUIT_U)?;

    let mut runtime_service_client = RuntimeServiceClient::connect(ADDR).await?;
    let mut image_service_client = ImageServiceClient::connect(ADDR).await?;
    get_and_print_version(&mut runtime_service_client).await?;

    if pod_exists(&String::from(NAME), &mut runtime_service_client).await? {
        let e = format!("the pod \"{}\" already exists", &String::from(NAME));
        return Err(e.into());
    }

    let mut rng = rand::thread_rng();
    let uid = format!("{}-{}", NAME, rng.gen::<u128>());
    let stdin = std::io::stdin();
    //let config = read_yaml_config(stdin);
    //println!("{:?}", config);

    let pod_sandbox_config = pod_sandbox_config(&String::from(NAME), &uid);
    let pod_sandbox_id = run_pod_sandbox(&pod_sandbox_config, &mut runtime_service_client).await?;
    let image_ref = pull_image(
        &IMAGE.to_string(),
        &pod_sandbox_config,
        &mut image_service_client,
    )
    .await?;
    let container = create_containers(
        &IMAGE.to_string(),
        &pod_sandbox_id,
        &pod_sandbox_config,
        &mut runtime_service_client,
    )
    .await?;

    start_containers(&container, &mut runtime_service_client).await?;

    loop {
        match term.load(Ordering::Relaxed) {
            0 => {
                let status = poll_container_status(&container, &mut runtime_service_client).await?;
                match status {
                    ContainerState::ContainerExited => {
                        info!("container exited");
                        break;
                    }
                    _ => (),
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
    stop_pod(&pod_sandbox_id, &mut runtime_service_client).await?;
    destroy_pod(&pod_sandbox_id, &mut runtime_service_client).await?;

    info!("cleaned up");
    Ok(())
}
