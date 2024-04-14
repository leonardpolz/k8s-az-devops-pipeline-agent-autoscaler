use k8s_openapi::api::core::v1::{
    Container, EnvVar, EnvVarSource, LocalObjectReference, ObjectFieldSelector, Pod, PodSpec,
    SecretKeySelector,
};
use kube::api::{DeleteParams, PostParams};
use kube::core::ObjectMeta;
use kube::{Api, Client};
use log::{error, info};
use rand::Rng;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::time::Duration;

use crate::crd_spec::TemplateSpec;
use crate::status_enum::Status;

#[derive(Clone)]
pub struct KubernetesActionService {
    pod_status_table: Arc<Mutex<HashMap<String, Status>>>,
    kube_client: Client,
}

impl KubernetesActionService {
    pub fn new(pod_status_table: Arc<Mutex<HashMap<String, Status>>>, kube_client: Client) -> Self {
        Self {
            pod_status_table,
            kube_client,
        }
    }

    pub async fn controller_thread(self, token: Arc<Mutex<bool>>, deployment_spec: TemplateSpec) {
        tokio::spawn(async move {
            loop {
                if *token.lock().unwrap() {
                    break;
                }

                let mut total = 0;
                let mut running = 0;
                let mut waiting = 0;

                {
                    let guard = self.pod_status_table.lock().unwrap();

                    for (_, pod_status) in guard.iter() {
                        total += 1;
                        if *pod_status == Status::Running {
                            running += 1;
                        } else {
                            waiting += 1;
                        }
                    }
                }

                info!(
                    "Total: {}, Waiting: {}, Running: {}",
                    total, waiting, running
                );

                if total == 0 || total == running {
                    self.create_pod(deployment_spec.clone()).await;
                } else if total - running > 1 {
                    let mut waiting_pod_id: String = "".to_string();
                    {
                        let guard = self.pod_status_table.lock().unwrap();

                        for (pod_id, pod_status) in guard.iter() {
                            if *pod_status == Status::Waiting {
                                waiting_pod_id = pod_id.to_string();
                                break;
                            }
                        }
                    }

                    self.delete_pod(waiting_pod_id).await;
                }

                tokio::time::sleep(Duration::from_secs(10)).await;
            }
        });
    }

    async fn create_pod(&self, deployment_spec: TemplateSpec) {
        let pods: Api<Pod> = Api::namespaced(self.kube_client.clone(), "default");
        let pod = Pod {
            metadata: ObjectMeta {
                name: Some(format!(
                    "{}-{}-{}",
                    "drc3937",
                    deployment_spec.config.get(0).unwrap().value,
                    self.generate_id().await
                )),
                labels: Some(
                    [("app".to_string(), "devopsagent".to_string())]
                        .iter()
                        .cloned()
                        .collect(),
                ),
                ..Default::default()
            },
            spec: Some(PodSpec {
                containers: vec![Container {
                    name: "devopsagent".to_string(),
                    image: Some(deployment_spec.image.clone()),
                    image_pull_policy: Some("Always".to_string()),
                    env: Some(vec![
                        EnvVar {
                            name: "AZP_AGENT_NAME".to_string(),
                            value_from: Some(EnvVarSource {
                                field_ref: Some(ObjectFieldSelector {
                                    api_version: Some("v1".to_string()),
                                    field_path: "metadata.name".to_string(),
                                }),
                                ..Default::default()
                            }),
                            ..Default::default()
                        },
                        EnvVar {
                            name: "AZP_URL".to_string(),
                            value: Some(deployment_spec.config.get(1).unwrap().value.clone()),
                            ..Default::default()
                        },
                        EnvVar {
                            name: "AZP_POOL".to_string(),
                            value: Some(deployment_spec.config.get(2).unwrap().value.clone()),
                            ..Default::default()
                        },
                        EnvVar {
                            name: "AZP_WORK".to_string(),
                            value: Some(deployment_spec.config.get(3).unwrap().value.clone()),
                            ..Default::default()
                        },
                        EnvVar {
                            name: "AZP_TOKEN".to_string(),
                            value_from: Some(EnvVarSource {
                                secret_key_ref: Some(SecretKeySelector {
                                    name: Some(
                                        deployment_spec.config.get(4).unwrap().value.clone(),
                                    ),
                                    key: deployment_spec.config.get(5).unwrap().value.clone(),
                                    ..Default::default()
                                }),
                                ..Default::default()
                            }),
                            ..Default::default()
                        },
                    ]),
                    ..Default::default()
                }],
                image_pull_secrets: Some(vec![LocalObjectReference {
                    name: Some(deployment_spec.pullSecretName.clone()),
                }]),
                ..Default::default()
            }),
            ..Default::default()
        };

        let pp = PostParams::default();

        match pods.create(&pp, &pod).await {
            Ok(o) => {
                self.pod_status_table
                    .lock()
                    .unwrap()
                    .insert(o.metadata.name.clone().unwrap(), Status::Waiting);
                info!("Created {:?}", o.metadata.name.unwrap())
            }
            Err(e) => error!("Error: {}", e),
        }
    }

    async fn generate_id(&self) -> i32 {
        let mut rng = rand::thread_rng();
        rng.gen_range(10000000..100000000)
    }

    async fn delete_pod(&self, pod_id: String) {
        let pods: Api<Pod> = Api::namespaced(self.kube_client.clone(), "default");
        let dp = DeleteParams::default();

        match pods.delete(&pod_id, &dp).await {
            Ok(_) => {
                info!("Successfully deleted pod {:?}", pod_id)
            }
            Err(e) => error!("Failed to delete pod: {:?}", e),
        };

        self.pod_status_table.lock().unwrap().remove(&pod_id);
    }

    pub async fn delete_all_pods(&self) {
        let guard = self.pod_status_table.lock().unwrap();
        let keys: Vec<String> = guard.keys().cloned().collect();
        drop(guard);

        for pod_id in keys {
            self.delete_pod(pod_id).await;
        }
    }
}
