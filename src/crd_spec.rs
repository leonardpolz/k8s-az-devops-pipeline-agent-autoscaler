use kube::CustomResource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Clone, Debug, CustomResource, Deserialize, Serialize, JsonSchema)]
#[kube(
    group = "example.com",
    version = "v1",
    kind = "DevopsReplicaController",
    namespaced
)]
pub struct DevopsReplicaControllerSpec {
    pub selector: Selector,
    pub template: Template,
}

#[allow(non_snake_case)]
#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
pub struct Selector {
    pub matchLabels: HashMap<String, String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
pub struct Template {
    pub metadata: TemplateMetadata,
    pub spec: TemplateSpec,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
pub struct TemplateMetadata {
    pub labels: HashMap<String, String>,
}

#[allow(non_snake_case)]
#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
pub struct TemplateSpec {
    pub image: String,
    pub pullSecretName: String,
    pub config: Vec<ConfigItem>,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
pub struct ConfigItem {
    pub name: String,
    pub value: String,
}
