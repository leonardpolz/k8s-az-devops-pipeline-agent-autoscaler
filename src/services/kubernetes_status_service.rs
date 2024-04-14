use k8s_openapi::api::core::v1::Pod;
use kube::{
    api::{Api, ListParams},
    Client,
};
use log::info;
use std::collections::HashMap;
use std::process::Command;
use std::sync::{Arc, Mutex};
use tokio::time::Duration;

use crate::status_enum::Status;

#[derive(Clone)]
pub struct KubernetesStatusService {
    pod_status_table: Arc<Mutex<HashMap<String, Status>>>,
    kube_client: Client,
}

impl KubernetesStatusService {
    pub fn new(pod_status_table: Arc<Mutex<HashMap<String, Status>>>, kube_client: Client) -> Self {
        Self {
            pod_status_table,
            kube_client,
        }
    }

    pub async fn update_pod_table_thread(self, token: Arc<Mutex<bool>>) {
        tokio::spawn(async move {
            loop {
                if *token.lock().unwrap() {
                    break;
                }

                let pods: Api<Pod> = Api::namespaced(self.kube_client.clone(), "default");
                let pod_list = match pods.list(&ListParams::default()).await {
                    Ok(list) => list,
                    Err(err) => {
                        eprintln!("Error listing pods: {}", err);
                        continue;
                    }
                };

                let filtered_pods = pod_list
                    .items
                    .into_iter()
                    .filter(|pod| pod.metadata.name.as_ref().map(|n| n.starts_with("drc3937")).unwrap_or(false))
                    .collect::<Vec<_>>();

                let mut pod_ids = vec![];

                for pod in filtered_pods {
                    if let Some(name) = pod.metadata.name {
                        pod_ids.push(name);
                    }
                }

                for pod_id in pod_ids {
                    let output = Command::new("sh")
                        .arg("-c")
                        .arg(format!("kubectl logs {} | tail -n 1 | grep -q \"Running job:\" && echo 1 || echo 0", pod_id))
                        .output()
                        .expect("Failed to execute command");

                    let output_str = String::from_utf8(output.stdout).unwrap();
                    let output_num: i32 = output_str
                        .trim()
                        .parse()
                        .expect("Failed to parse output to integer");

                    let mut status_table = self.pod_status_table.lock().unwrap();
                    if output_num == 1 {
                        status_table.insert(pod_id, Status::Running);
                    } else {
                        status_table.insert(pod_id, Status::Waiting);
                    }
                }

                info!("Updated Table.");
                tokio::time::sleep(Duration::from_secs(10)).await;
            }
        });
    }
}
