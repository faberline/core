//! The [`stateful_instance`] renderer and the storage checks it runs first.

use std::collections::BTreeMap;

use serde_json::{json, Value};

use super::{
    StatefulInstanceError, StatefulInstancePlan, StatefulInstanceRender, StatefulStorageAttachment,
    VolumeClaimTemplate,
};

fn nonempty(value: &str, field: &'static str) -> Result<(), StatefulInstanceError> {
    if value.trim().is_empty() {
        Err(StatefulInstanceError::EmptyField(field))
    } else {
        Ok(())
    }
}

fn merge_extra_labels(
    target: &mut Value,
    extra: &BTreeMap<String, String>,
    core: &Value,
) -> Result<(), StatefulInstanceError> {
    let target = target.as_object_mut().expect("labels are an object");
    let core = core.as_object().expect("RenderCtx labels are an object");
    for (key, value) in extra {
        if let Some(expected) = core.get(key) {
            if expected != value {
                return Err(StatefulInstanceError::SelectorCoreIdentityOverride);
            }
            continue;
        }
        target.insert(key.clone(), json!(value));
    }
    Ok(())
}

fn merge_extra_selector(
    target: &mut Value,
    extra: &BTreeMap<String, String>,
    labels: &Value,
) -> Result<(), StatefulInstanceError> {
    let target = target.as_object_mut().expect("selector is an object");
    let labels = labels.as_object().expect("RenderCtx labels are an object");
    for (key, value) in extra {
        if let Some(expected) = labels.get(key) {
            if expected != value {
                return Err(StatefulInstanceError::SelectorCoreIdentityOverride);
            }
            continue;
        }
        target.insert(key.clone(), json!(value));
    }
    Ok(())
}

fn claim_template(
    mut template: Value,
    claim: &VolumeClaimTemplate,
    labels: &Value,
) -> Result<Value, StatefulInstanceError> {
    let object = template
        .as_object_mut()
        .ok_or(StatefulInstanceError::EmptyField("claim template"))?;
    let metadata = object.entry("metadata").or_insert_with(|| json!({}));
    let metadata = metadata
        .as_object_mut()
        .ok_or(StatefulInstanceError::EmptyField("claim template metadata"))?;
    metadata.insert("name".into(), json!(claim.name));
    let template_labels = metadata.entry("labels").or_insert_with(|| json!({}));
    if !template_labels.is_object() {
        return Err(StatefulInstanceError::EmptyField("claim template labels"));
    }
    merge_extra_labels(template_labels, &BTreeMap::new(), labels)?;
    for (key, value) in labels.as_object().expect("RenderCtx labels are an object") {
        match template_labels.get(key) {
            Some(existing) if existing != value => {
                return Err(StatefulInstanceError::SelectorCoreIdentityOverride)
            }
            Some(_) => {}
            None => {
                template_labels
                    .as_object_mut()
                    .expect("labels are an object")
                    .insert(key.clone(), value.clone());
            }
        }
    }
    Ok(template)
}

fn is_safe_basename(value: &str) -> bool {
    !value.is_empty()
        && value != "."
        && value != ".."
        && !value.contains('/')
        && !value.contains('\0')
}

fn is_exact_storage_child_mount(
    mount: &Value,
    volume_name: &str,
    parent_mount_path: &str,
    parent_read_only: bool,
) -> bool {
    let Some(object) = mount.as_object() else {
        return false;
    };
    if object.len() != 4
        || !object.contains_key("name")
        || !object.contains_key("mountPath")
        || !object.contains_key("subPath")
        || !object.contains_key("readOnly")
    {
        return false;
    }
    let Some(sub_path) = object.get("subPath").and_then(Value::as_str) else {
        return false;
    };
    is_safe_basename(sub_path)
        && object.get("name").and_then(Value::as_str) == Some(volume_name)
        && object.get("readOnly").and_then(Value::as_bool) == Some(parent_read_only)
        && object.get("mountPath").and_then(Value::as_str)
            == Some(format!("{parent_mount_path}/{sub_path}").as_str())
}

fn overlaps_storage_mount(mount: &Value, volume_name: &str, parent_mount_path: &str) -> bool {
    mount.get("name").and_then(Value::as_str) == Some(volume_name)
        || mount
            .get("mountPath")
            .and_then(Value::as_str)
            .is_some_and(|path| {
                path == parent_mount_path || path.starts_with(&format!("{parent_mount_path}/"))
            })
}

/// Render one deterministic StatefulSet and, for an independent claim, its
/// PVC.  The returned independent PVC is ready before the StatefulSet so a
/// caller can apply storage before the workload.
pub fn stateful_instance(
    plan: StatefulInstancePlan<'_>,
) -> Result<StatefulInstanceRender, StatefulInstanceError> {
    if plan.replicas == 0 {
        return Err(StatefulInstanceError::ZeroReplicas);
    }
    if plan.cx.app().trim().is_empty() {
        return Err(StatefulInstanceError::EmptyIdentity);
    }
    nonempty(plan.cx.name(), "name")?;
    nonempty(plan.cx.ns(), "namespace")?;
    nonempty(plan.cx.app(), "identity")?;
    nonempty(&plan.service_name, "service_name")?;

    nonempty(&plan.name, "statefulset name")?;
    if let Some(storage) = &plan.storage {
        match storage {
            StatefulStorageAttachment::VolumeClaimTemplate(claim) => {
                nonempty(&claim.name, "claim template name")?;
                nonempty(&claim.mount_path, "mount path")?;
                if !claim.template.is_object() {
                    return Err(StatefulInstanceError::EmptyField("claim template"));
                }
            }
            StatefulStorageAttachment::ExistingClaim(claim) => {
                nonempty(&claim.claim_name, "claim name")?;
                nonempty(&claim.volume_name, "volume name")?;
                nonempty(&claim.mount_path, "mount path")?;
                if !claim.template.is_object() {
                    return Err(StatefulInstanceError::EmptyField("claim template"));
                }
            }
        }
    }

    let mut labels = plan.cx.labels(plan.pod.component);
    merge_extra_labels(
        &mut labels,
        &plan.labels,
        &plan.cx.labels(plan.pod.component),
    )?;
    let mut selector = plan.cx.selector(plan.pod.component);
    merge_extra_selector(
        &mut selector,
        &plan.selector,
        &plan.cx.labels(plan.pod.component),
    )?;
    let core_labels = plan.cx.labels(plan.pod.component);
    for (key, value) in &plan.selector {
        if core_labels.get(key).is_none() {
            labels[key] = json!(value);
        }
    }
    let component = plan.pod.component;
    let topology_spread_constraints = if plan.topology_spread_constraints.is_empty() {
        plan.pod.topology_spread_constraints.clone()
    } else {
        plan.topology_spread_constraints.clone()
    };
    let mut template = plan.pod.render();
    let template_obj = template.as_object_mut().unwrap();
    let metadata = template_obj.entry("metadata").or_insert_with(|| json!({}));
    let metadata = metadata
        .as_object_mut()
        .ok_or(StatefulInstanceError::InvalidPodTemplate)?;
    metadata.insert("labels".into(), labels.clone());
    let spec = template_obj.entry("spec").or_insert_with(|| json!({}));
    let spec = spec
        .as_object_mut()
        .ok_or(StatefulInstanceError::InvalidPodTemplate)?;
    // `ServicePodTemplate` can carry a spread constraint for Deployment. Move
    // it to the StatefulSet ordering point after placement fields so the
    // source-compatible adapter keeps its historical serialized order.
    spec.remove("topologySpreadConstraints");
    let containers = spec.entry("containers").or_insert_with(|| json!([]));
    let containers = containers
        .as_array_mut()
        .ok_or(StatefulInstanceError::InvalidPodTemplate)?;
    if containers.is_empty() {
        return Err(StatefulInstanceError::EmptyField("containers"));
    }
    let container = containers[0]
        .as_object_mut()
        .ok_or(StatefulInstanceError::InvalidPodTemplate)?;
    if let Some(storage) = &plan.storage {
        let (volume_name, mount_path, read_only) = match storage {
            StatefulStorageAttachment::VolumeClaimTemplate(claim) => {
                (&claim.name, &claim.mount_path, claim.read_only)
            }
            StatefulStorageAttachment::ExistingClaim(claim) => {
                (&claim.volume_name, &claim.mount_path, claim.read_only)
            }
        };
        let mounts = container.entry("volumeMounts").or_insert_with(|| json!([]));
        let mounts = mounts
            .as_array_mut()
            .ok_or(StatefulInstanceError::InvalidPodTemplate)?;
        let mut child = None;
        let mut unrelated = Vec::with_capacity(mounts.len());
        for mount in std::mem::take(mounts) {
            if overlaps_storage_mount(&mount, volume_name, mount_path) {
                if child.is_some()
                    || !is_exact_storage_child_mount(&mount, volume_name, mount_path, read_only)
                {
                    return Err(StatefulInstanceError::VolumeMountCollision);
                }
                child = Some(mount);
            } else {
                unrelated.push(mount);
            }
        }
        unrelated
            .push(json!({ "name": volume_name, "mountPath": mount_path, "readOnly": read_only }));
        if let Some(child) = child {
            unrelated.push(child);
        }
        *mounts = unrelated;
    }

    let mut sts_spec = json!({
        "replicas": plan.replicas,
        "serviceName": plan.service_name,
        "podManagementPolicy": "Parallel",
        "selector": { "matchLabels": selector },
        "template": template,
    });
    if let Some(policy) = plan.pod_management_policy {
        sts_spec["podManagementPolicy"] = json!(policy);
    }
    if let Some(value) = plan.affinity {
        sts_spec["template"]["spec"]["affinity"] = value;
    }
    if let Some(value) = plan.node_selector {
        sts_spec["template"]["spec"]["nodeSelector"] = value;
    }
    if !plan.tolerations.is_empty() {
        sts_spec["template"]["spec"]["tolerations"] = json!(plan.tolerations);
    }
    if !topology_spread_constraints.is_empty() {
        sts_spec["template"]["spec"]["topologySpreadConstraints"] =
            json!(topology_spread_constraints);
    }
    if let Some(value) = plan.enable_service_links {
        sts_spec["template"]["spec"]["enableServiceLinks"] = json!(value);
    }
    if let Some(value) = plan.revision_history_limit {
        sts_spec["revisionHistoryLimit"] = json!(value);
    }
    if let Some(value) = plan.update_strategy {
        sts_spec["updateStrategy"] = value;
    }
    let mut pvc = None;
    match &plan.storage {
        Some(StatefulStorageAttachment::VolumeClaimTemplate(claim)) => {
            sts_spec["volumeClaimTemplates"] =
                json!([claim_template(claim.template.clone(), claim, &labels,)?]);
        }
        Some(StatefulStorageAttachment::ExistingClaim(claim)) => {
            let volumes = sts_spec["template"]["spec"]
                .get_mut("volumes")
                .and_then(Value::as_array_mut);
            let mut volumes = volumes.cloned().unwrap_or_default();
            if volumes.iter().any(|volume| {
                volume.get("name").and_then(Value::as_str) == Some(claim.volume_name.as_str())
            }) {
                return Err(StatefulInstanceError::VolumeMountCollision);
            }
            volumes.push(json!({ "name": claim.volume_name, "persistentVolumeClaim": { "claimName": claim.claim_name } }));
            sts_spec["template"]["spec"]["volumes"] = json!(volumes);
            let mut p = claim.template.clone();
            p["metadata"] = plan.cx.meta(&claim.claim_name, component);
            p["metadata"]["labels"] = labels.clone();
            pvc = Some(
                json!({"apiVersion":"v1","kind":"PersistentVolumeClaim","metadata":p["metadata"].clone(),"spec":p["spec"].clone()}),
            );
        }
        None => {}
    }
    let mut metadata = plan.cx.meta(&plan.name, component);
    metadata["labels"] = labels;
    Ok(StatefulInstanceRender {
        storage: pvc,
        workload: json!({"apiVersion": "apps/v1", "kind": "StatefulSet", "metadata": metadata, "spec": sts_spec}),
    })
}
