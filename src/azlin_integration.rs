//! Integration with azlin for Azure VM discovery and bastion tunnels.
//!
//! Uses azlin-azure for VM listing and azlin-ssh for connections.
//! Bastion tunnels managed via `az network bastion tunnel` subprocess
//! with persistent registry at /tmp/azlin-tunnels/.

use anyhow::{Context, Result};
use azlin_azure::auth::AzureAuth;
use azlin_azure::vm::VmManager;
use azlin_core::models::{PowerState, VmInfo};
use azlin_ssh::{SshConfig, SshPool};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

use crate::source::ssh_tmux::RemoteConfig;
use crate::tmux::SessionInfo;

const TUNNEL_REGISTRY_PATH: &str = "/tmp/azlin-tunnels/registry.json";
const TUNNEL_PORT_START: u16 = 50200;

/// Config section for azlin integration.
#[derive(Debug, Default, Clone, Deserialize)]
#[serde(default)]
pub struct AzlinConfig {
    pub enabled: bool,
    pub resource_group: Option<String>,
}

/// Discover running Azure VMs via azlin-azure (synchronous — uses az CLI).
pub fn discover_vms(resource_group: Option<&str>) -> Result<Vec<VmInfo>> {
    let auth = AzureAuth::new().map_err(|e| anyhow::anyhow!("{}", e))?;
    let vm_manager = VmManager::new(&auth);

    let vms = if let Some(rg) = resource_group {
        vm_manager
            .list_vms(rg)
            .map_err(|e| anyhow::anyhow!("{}", e))?
    } else {
        vm_manager
            .list_all_vms()
            .map_err(|e| anyhow::anyhow!("{}", e))?
    };

    Ok(vms
        .into_iter()
        .filter(|vm| matches!(vm.power_state, PowerState::Running))
        .collect())
}

/// Convert a VmInfo to a RemoteConfig.
/// For private VMs, creates a bastion tunnel and routes through localhost.
pub fn vm_to_remote_config(vm: &VmInfo) -> Result<RemoteConfig> {
    let user = vm
        .admin_username
        .clone()
        .unwrap_or_else(|| "azureuser".to_string());

    let key = resolve_ssh_key();

    if let Some(ref public_ip) = vm.public_ip {
        Ok(RemoteConfig {
            name: vm.name.clone(),
            host: public_ip.clone(),
            user,
            key,
            port: 22,
            poll_interval_ms: 500,
        })
    } else if vm.private_ip.is_some() {
        let local_port =
            get_or_create_bastion_tunnel(vm).context(format!("Bastion tunnel for {}", vm.name))?;

        Ok(RemoteConfig {
            name: vm.name.clone(),
            host: "127.0.0.1".to_string(),
            user,
            key,
            port: local_port,
            poll_interval_ms: 500,
        })
    } else {
        anyhow::bail!("VM '{}' has no IP", vm.name);
    }
}

/// Discover tmux sessions on all reachable VMs.
pub async fn discover_remote_sessions(
    pool: &SshPool,
    resource_group: Option<&str>,
) -> Result<Vec<SessionInfo>> {
    // VM discovery is synchronous (az CLI calls)
    let vms = discover_vms(resource_group)?;
    let mut sessions = Vec::new();

    for vm in &vms {
        let remote = match vm_to_remote_config(vm) {
            Ok(r) => r,
            Err(_) => continue,
        };

        let ssh_config = match to_ssh_config(&remote) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let result = match pool.get_or_connect(&ssh_config).await {
            Ok(mut client) => {
                let res = client
                    .execute("tmux list-sessions -F '#{session_name}' 2>/dev/null || true")
                    .await;
                pool.release(client).await;
                res
            }
            Err(_) => continue,
        };

        if let Ok(result) = result {
            for name in result.stdout.lines().filter(|l| !l.is_empty()) {
                sessions.push(SessionInfo {
                    name: name.to_string(),
                    attached: false,
                    windows: 0,
                    host: Some(vm.name.clone()),
                });
            }
        }
    }

    Ok(sessions)
}

fn to_ssh_config(remote: &RemoteConfig) -> Result<SshConfig> {
    let key_path = remote
        .key
        .as_ref()
        .map(|p| PathBuf::from(shellexpand::tilde(p).as_ref()))
        .or_else(resolve_ssh_key_path)
        .context("No SSH key found")?;

    let mut config = SshConfig::new(&remote.host, &remote.user, key_path);
    config.port = remote.port;
    Ok(config)
}

fn resolve_ssh_key() -> Option<String> {
    resolve_ssh_key_path().map(|p| p.to_string_lossy().to_string())
}

fn resolve_ssh_key_path() -> Option<PathBuf> {
    let home = dirs::home_dir()?;
    for name in ["azlin_key", "id_rsa", "id_ed25519"] {
        let path = home.join(".ssh").join(name);
        if path.exists() {
            return Some(path);
        }
    }
    None
}

// ---- Bastion Tunnel Management ----

#[derive(Debug, Serialize, Deserialize)]
struct TunnelRegistryEntry {
    vm_resource_id: String,
    bastion_name: String,
    resource_group: String,
    local_port: u16,
    pid: u32,
    created_at: u64,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct TunnelRegistry {
    tunnels: HashMap<String, TunnelRegistryEntry>,
}

fn load_registry() -> TunnelRegistry {
    std::fs::read_to_string(TUNNEL_REGISTRY_PATH)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn save_registry(registry: &TunnelRegistry) {
    if let Some(parent) = std::path::Path::new(TUNNEL_REGISTRY_PATH).parent() {
        std::fs::create_dir_all(parent).ok();
    }
    if let Ok(json) = serde_json::to_string_pretty(registry) {
        std::fs::write(TUNNEL_REGISTRY_PATH, json).ok();
    }
}

fn is_process_alive(pid: u32) -> bool {
    unsafe { libc::kill(pid as i32, 0) == 0 }
}

fn build_vm_resource_id(vm: &VmInfo) -> Result<String> {
    let output = std::process::Command::new("az")
        .args(["account", "show", "--query", "id", "-o", "tsv"])
        .output()
        .context("Failed to get subscription ID")?;

    let sub_id = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if sub_id.is_empty() {
        anyhow::bail!("No Azure subscription found — run `az login`");
    }

    Ok(format!(
        "/subscriptions/{}/resourceGroups/{}/providers/Microsoft.Compute/virtualMachines/{}",
        sub_id, vm.resource_group, vm.name
    ))
}

fn detect_bastion(resource_group: &str) -> Option<String> {
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
        .output()
        .ok()?;

    let name = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if name.is_empty() {
        None
    } else {
        Some(name)
    }
}

fn get_or_create_bastion_tunnel(vm: &VmInfo) -> Result<u16> {
    let vm_resource_id = build_vm_resource_id(vm)?;

    let mut registry = load_registry();

    // Prune dead tunnels
    registry
        .tunnels
        .retain(|_, entry| is_process_alive(entry.pid));

    // Return existing if alive
    if let Some(entry) = registry.tunnels.get(&vm_resource_id) {
        save_registry(&registry);
        return Ok(entry.local_port);
    }

    let bastion_name = detect_bastion(&vm.resource_group)
        .ok_or_else(|| anyhow::anyhow!("No bastion host in RG {}", vm.resource_group))?;

    let local_port = TUNNEL_PORT_START + registry.tunnels.len() as u16;

    let child = std::process::Command::new("az")
        .args([
            "network",
            "bastion",
            "tunnel",
            "--name",
            &bastion_name,
            "--resource-group",
            &vm.resource_group,
            "--target-resource-id",
            &vm_resource_id,
            "--resource-port",
            "22",
            "--port",
            &local_port.to_string(),
        ])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .context("Failed to spawn bastion tunnel")?;

    let pid = child.id();

    // Wait for tunnel to establish
    std::thread::sleep(std::time::Duration::from_secs(3));

    if !is_process_alive(pid) {
        anyhow::bail!("Bastion tunnel process died");
    }

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    registry.tunnels.insert(
        vm_resource_id.clone(),
        TunnelRegistryEntry {
            vm_resource_id,
            bastion_name,
            resource_group: vm.resource_group.clone(),
            local_port,
            pid,
            created_at: now,
        },
    );

    save_registry(&registry);
    Ok(local_port)
}
