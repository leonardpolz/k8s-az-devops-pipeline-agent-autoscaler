use env_logger::{Builder, Env};
use futures_util::stream::StreamExt;
use kube::runtime::watcher::{self, Event};
use kube::{Api, Client, Config};
use log::{error, info, warn};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::signal::unix::{signal, SignalKind};

mod services;
use services::kubernetes_action_service::KubernetesActionService;
use services::kubernetes_status_service::KubernetesStatusService;

mod status_enum;
use status_enum::Status;

mod crd_spec;
use crd_spec::DevopsReplicaController;

#[tokio::main]
async fn main() {
    Builder::from_env(Env::default().default_filter_or("info")).init();
    info!("Devops replica controller started!");

    let pod_status_table: Arc<Mutex<HashMap<String, Status>>> =
        Arc::new(Mutex::new(HashMap::new()));

    let kube_config = Config::infer().await.unwrap();
    let kube_client = Client::try_from(kube_config).unwrap();

    let drcreplicacontrollers: Api<DevopsReplicaController> =
        Api::namespaced(kube_client.clone(), "default");

    let watcher_config = watcher::Config::default();
    let watcher = watcher::watcher(drcreplicacontrollers, watcher_config);

    let token = Arc::new(Mutex::new(false));
    let status_service =
        KubernetesStatusService::new(Arc::clone(&pod_status_table), kube_client.clone());
    let action_service =
        KubernetesActionService::new(Arc::clone(&pod_status_table), kube_client.clone());

    tokio::pin!(watcher);

    info!("Watching for events...");
    while let Some(event) = watcher.next().await {
        match event {
            Ok(Event::Applied(deployment)) => {
                let status_service = status_service.clone();
                let action_service = action_service.clone();

                *token.lock().unwrap() = true;
                *token.lock().unwrap() = false;
                status_service
                    .update_pod_table_thread(Arc::clone(&token))
                    .await;

                let delay_time_for_state = Duration::from_secs(5);
                tokio::time::sleep(delay_time_for_state).await;

                let deployment_spec = deployment.spec.template.spec.clone();
                action_service
                    .controller_thread(Arc::clone(&token), deployment_spec.clone())
                    .await;
                info!(
                    "Applied Deployment: {:?}",
                    deployment_spec.config.get(0).unwrap().value
                );
            }
            Ok(Event::Deleted(deployment)) => {
                *token.lock().unwrap() = true;
                *token.lock().unwrap() = true;
                info!("Deployment deletion received, deleting all pods...");
                action_service.delete_all_pods().await;
                info!(
                    "Deleted Deployement: {:?}",
                    deployment.spec.template.spec.config.get(0).unwrap().value
                );
            }
            Ok(Event::Restarted(_deployments)) => {
                info!("Restarted event received!");
            }
            Err(e) => {
                if format!("{}", e).contains("os error 54") {
                    warn!("Connection reset by peer (os error 54), will try to reconnect.");
                } else {
                    error!("Error: {}", e);
                }
            }
        }
    }

    signal(SignalKind::terminate())
        .unwrap()
        .recv()
        .await
        .unwrap();

    *token.lock().unwrap() = true;
}
