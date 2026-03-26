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
}

/// Config section for azlin integration.
#[derive(Debug, Default, Clone, serde::Deserialize)]
#[serde(default)]
pub struct AzlinConfig {
    pub enabled: bool,
    pub resource_group: Option<String>,
}

/// Discover running Azure VMs via azlin-azure (synchronous -- uses az CLI).
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
        .filter(|vm| {
            // Filter to running VMs -- check power_state field
            format!("{:?}", vm.power_state).contains("Running")
        })
        .map(|vm| VmInfo {
            name: vm.name.clone(),
            admin_username: vm.admin_username.clone(),
            public_ip: vm.public_ip.clone(),
            private_ip: vm.private_ip.clone(),
        })
        .collect())
}

/// Convert a VmInfo to a RemoteConfig.
/// Prefers public IP, falls back to private IP (works on same vnet).
pub fn vm_to_remote_config(vm: &VmInfo) -> Result<RemoteConfig> {
    let user = vm
        .admin_username
        .clone()
        .unwrap_or_else(|| "azureuser".to_string());

    let key = resolve_ssh_key();

    let host = vm
        .public_ip
        .as_ref()
        .or(vm.private_ip.as_ref())
        .ok_or_else(|| anyhow::anyhow!("VM '{}' has no IP", vm.name))?
        .clone();

    Ok(RemoteConfig {
        name: vm.name.clone(),
        host,
        user,
        key,
        port: 22,
        poll_interval_ms: 500,
    })
}

/// Discover tmux sessions on all reachable VMs using SSH subprocess.
pub fn discover_remote_sessions_sync(resource_group: Option<&str>) -> Result<Vec<SessionInfo>> {
    let vms = discover_vms(resource_group)?;
    let mut sessions = Vec::new();

    for vm in &vms {
        let remote = match vm_to_remote_config(vm) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("  {}: {}", vm.name, e);
                continue;
            }
        };

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
        ssh_args.push("tmux list-sessions -F '#{session_name}' 2>/dev/null || true".to_string());

        let str_args: Vec<&str> = ssh_args.iter().map(|s| s.as_str()).collect();
        let output = std::process::Command::new("ssh").args(&str_args).output();

        match output {
            Ok(o) if o.status.success() => {
                let stdout = String::from_utf8_lossy(&o.stdout);
                for name in stdout.lines().filter(|l| !l.is_empty()) {
                    sessions.push(SessionInfo {
                        name: name.to_string(),
                        attached: false,
                        windows: 0,
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
