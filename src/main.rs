#[macro_use]
extern crate log;
mod client;
mod config;

use crate::client::*;
use crate::config::*;

use rand::Rng;
use signal_hook::consts::signal::*;
use signal_hook::flag as signal_flag;
use std::env;
use std::fs::File;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

const ADDR: &'static str = "http://localhost:3000";

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

    let mut m = CRIClient::connect(ADDR).await?;
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
                &image_ref,
                &container,
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
                        client::runtime::ContainerState::ContainerExited => {
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
