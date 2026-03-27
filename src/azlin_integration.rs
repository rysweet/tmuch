//! Integration with azlin for Azure VM discovery.
//!
//! Uses azlin-azure for VM listing and SSH subprocess for session discovery.

use anyhow::Result;
use azlin_azure::auth::AzureAuth;
use azlin_azure::vm::VmManager;
use std::path::PathBuf;

use crate::source::ssh_subprocess::RemoteConfig;
use crate::tmux::SessionInfo;

/// VM info extracted from azlin-azure (avoids needing azlin-core).
pub struct VmInfo {
    pub name: String,
    pub admin_username: Option<String>,
    pub public_ip: Option<String>,
    pub private_ip: Option<String>,
    pub resource_group: Option<String>,
    pub subscription_id: Option<String>,
}

/// Config section for azlin integration.
#[derive(Debug, Default, Clone, serde::Deserialize)]
#[serde(default)]
pub struct AzlinConfig {
    pub enabled: bool,
    pub resource_group: Option<String>,
    /// Override the default SSH user for azlin VMs (falls back to "azureuser").
    pub default_user: Option<String>,
    /// Auto-discover azlin VMs on startup (default: true when enabled).
    #[serde(default = "default_true")]
    pub auto_discover: bool,
}

fn default_true() -> bool {
    true
}

/// Discover running Azure VMs via azlin-azure (synchronous -- uses az CLI).
pub fn discover_vms(resource_group: Option<&str>) -> Result<Vec<VmInfo>> {
    // Resource group is required — listing all VMs across all subscriptions
    // takes 30+ seconds and often times out
    let resource_group = resource_group.ok_or_else(|| {
        anyhow::anyhow!("Resource group required for VM discovery. Set [azlin] resource_group in config or default_resource_group in ~/.azlin/config.toml")
    })?;

    let auth = AzureAuth::new().map_err(|e| anyhow::anyhow!("{}", e))?;
    let vm_manager = VmManager::new(&auth);

    let vms = vm_manager
        .list_vms(resource_group)
        .map_err(|e| anyhow::anyhow!("{}", e))?;

    let subscription_id = vm_manager.subscription_id().to_string();
    crate::dlog!("azlin: discovered {} VMs total", vms.len());

    Ok(vms
        .into_iter()
        .filter(|vm| {
            // Filter to running VMs -- check power_state field
            format!("{:?}", vm.power_state).contains("Running")
        })
        .map(|vm| VmInfo {
            name: vm.name.clone(),
            admin_username: vm.admin_username.clone(),
            public_ip: vm.public_ip.clone(),
            private_ip: vm.private_ip.clone(),
            resource_group: Some(vm.resource_group.clone()),
            subscription_id: Some(subscription_id.clone()),
        })
        .collect())
}

/// Convert a VmInfo to a RemoteConfig.
/// Prefers public IP, falls back to private IP (works on same vnet).
/// User resolution order: vm.admin_username -> azlin_config.default_user -> "azureuser".
pub fn vm_to_remote_config(vm: &VmInfo) -> Result<RemoteConfig> {
    vm_to_remote_config_with(vm, None)
}

/// Like vm_to_remote_config but accepts an optional AzlinConfig for default_user.
pub fn vm_to_remote_config_with(
    vm: &VmInfo,
    azlin_config: Option<&AzlinConfig>,
) -> Result<RemoteConfig> {
    let user = vm
        .admin_username
        .clone()
        .or_else(|| azlin_config.and_then(|c| c.default_user.clone()))
        .unwrap_or_else(|| "azureuser".to_string());

    let key = resolve_ssh_key();

    // If the VM has a public IP, use direct SSH.
    // Otherwise, try bastion SSH via `az network bastion ssh`.
    if let Some(ref public_ip) = vm.public_ip {
        Ok(RemoteConfig {
            name: vm.name.clone(),
            host: public_ip.clone(),
            user,
            key,
            port: 22,
            poll_interval_ms: 500,
            bastion: None,
        })
    } else if let Some(ref rg) = vm.resource_group {
        // No public IP — attempt bastion
        let bastion_name = detect_bastion(rg).unwrap_or_default();
        if bastion_name.is_empty() {
            // No bastion found either, fall back to private IP
            let host = vm
                .private_ip
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("VM '{}' has no IP", vm.name))?
                .clone();
            crate::dlog!(
                "azlin: VM '{}' has no public IP and no bastion, using private IP",
                vm.name
            );
            Ok(RemoteConfig {
                name: vm.name.clone(),
                host,
                user,
                key,
                port: 22,
                poll_interval_ms: 500,
                bastion: None,
            })
        } else {
            crate::dlog!(
                "azlin: VM '{}' using bastion '{}' in rg '{}'",
                vm.name,
                bastion_name,
                rg
            );
            let vm_rid = build_vm_resource_id(vm)?;
            // For bastion, host is the bastion_name (used for display), not an IP
            Ok(RemoteConfig {
                name: vm.name.clone(),
                host: vm.private_ip.clone().unwrap_or_default(),
                user,
                key,
                port: 22,
                poll_interval_ms: 500,
                bastion: Some(format!("{}:{}:{}", bastion_name, rg, vm_rid)),
            })
        }
    } else {
        // No resource group info, fall back to private IP
        let host = vm
            .private_ip
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("VM '{}' has no IP", vm.name))?
            .clone();
        Ok(RemoteConfig {
            name: vm.name.clone(),
            host,
            user,
            key,
            port: 22,
            poll_interval_ms: 500,
            bastion: None,
        })
    }
}

/// Discover tmux sessions on all reachable VMs using SSH subprocess.
pub fn discover_remote_sessions_sync(resource_group: Option<&str>) -> Result<Vec<SessionInfo>> {
    let vms = discover_vms(resource_group)?;
    let mut sessions = Vec::new();

    for vm in &vms {
        let remote = match vm_to_remote_config_with(vm, None) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("  {}: {}", vm.name, e);
                continue;
            }
        };

        let list_cmd = "tmux list-sessions -F '#{session_name}' 2>/dev/null || true".to_string();

        let output = if let Some(ref bastion_str) = remote.bastion {
            // Bastion SSH path
            let parts: Vec<&str> = bastion_str.splitn(3, ':').collect();
            if parts.len() == 3 {
                std::process::Command::new("az")
                    .args([
                        "network",
                        "bastion",
                        "ssh",
                        "--name",
                        parts[0],
                        "--resource-group",
                        parts[1],
                        "--target-resource-id",
                        parts[2],
                        "--auth-type",
                        "AAD",
                        "--username",
                        &remote.user,
                        "--",
                        "-o",
                        "StrictHostKeyChecking=accept-new",
                        "-o",
                        "BatchMode=yes",
                        &list_cmd,
                    ])
                    .output()
            } else {
                continue;
            }
        } else {
            // Direct SSH path
            let mut ssh_args = vec![
                "-o".to_string(),
                "StrictHostKeyChecking=accept-new".to_string(),
                "-o".to_string(),
                "ConnectTimeout=5".to_string(),
                "-o".to_string(),
                "BatchMode=yes".to_string(),
            ];
            if let Some(ref key) = remote.key {
                ssh_args.push("-i".to_string());
                ssh_args.push(key.clone());
            } else if let Some(key_path) = resolve_ssh_key_path() {
                ssh_args.push("-i".to_string());
                ssh_args.push(key_path.to_string_lossy().to_string());
            }
            ssh_args.push("-p".to_string());
            ssh_args.push(remote.port.to_string());
            ssh_args.push(format!("{}@{}", remote.user, remote.host));
            ssh_args.push(list_cmd);

            let str_args: Vec<&str> = ssh_args.iter().map(|s| s.as_str()).collect();
            std::process::Command::new("ssh").args(&str_args).output()
        };

        match output {
            Ok(o) if o.status.success() => {
                let stdout = String::from_utf8_lossy(&o.stdout);
                for name in stdout.lines().filter(|l| !l.is_empty()) {
                    sessions.push(SessionInfo {
                        name: name.to_string(),
                        attached: false,
                        host: Some(vm.name.clone()),
                    });
                }
            }
            Ok(o) => {
                let stderr = String::from_utf8_lossy(&o.stderr);
                eprintln!("  {}: SSH failed: {}", vm.name, stderr.trim());
            }
            Err(e) => {
                eprintln!("  {}: {}", vm.name, e);
            }
        }
    }

    Ok(sessions)
}

/// Detect a bastion host in the given resource group.
pub fn detect_bastion(resource_group: &str) -> Result<String> {
    let output = std::process::Command::new("az")
        .args([
            "network",
            "bastion",
            "list",
            "--resource-group",
            resource_group,
            "--query",
            "[0].name",
            "-o",
            "tsv",
        ])
        .output()?;

    if !output.status.success() {
        anyhow::bail!(
            "Failed to list bastions: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }

    let name = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if name.is_empty() {
        anyhow::bail!("No bastion found in resource group '{}'", resource_group);
    }
    Ok(name)
}

/// Build the full Azure resource ID for a VM.
pub fn build_vm_resource_id(vm: &VmInfo) -> Result<String> {
    let sub = vm
        .subscription_id
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("VM '{}' missing subscription_id", vm.name))?;
    let rg = vm
        .resource_group
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("VM '{}' missing resource_group", vm.name))?;
    Ok(format!(
        "/subscriptions/{}/resourceGroups/{}/providers/Microsoft.Compute/virtualMachines/{}",
        sub, rg, vm.name
    ))
}

fn resolve_ssh_key() -> Option<String> {
    resolve_ssh_key_path().map(|p| p.to_string_lossy().to_string())
}

fn resolve_ssh_key_path() -> Option<PathBuf> {
    let home = dirs::home_dir()?;
    for name in ["azlin_key", "id_ed25519", "id_rsa"] {
        let path = home.join(".ssh").join(name);
        if path.exists() {
            return Some(path);
        }
    }
    None
}
