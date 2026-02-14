//! Module loader — scans `~/.starkbot/modules/` for installed dynamic modules.
//!
//! Each subdirectory is expected to contain a `module.toml` manifest.
//! The loader parses each manifest and creates a `DynamicModule`.

use super::dynamic_module::DynamicModule;
use super::manifest::ModuleManifest;
use super::Module;
use std::path::PathBuf;

/// Default base directory for dynamically installed modules.
fn modules_base_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("STARKBOT_MODULES_DIR") {
        return PathBuf::from(dir);
    }
    dirs_or_home().join(".starkbot").join("modules")
}

/// Get the user's home directory.
fn dirs_or_home() -> PathBuf {
    std::env::var("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."))
}

/// Scan the modules directory and load all valid dynamic modules.
///
/// Returns a Vec of successfully loaded modules. Invalid/broken manifests
/// are logged as warnings and skipped.
pub fn load_dynamic_modules() -> Vec<DynamicModule> {
    let base = modules_base_dir();

    if !base.exists() {
        log::debug!(
            "[MODULE] Dynamic modules directory does not exist: {}",
            base.display()
        );
        return Vec::new();
    }

    let entries = match std::fs::read_dir(&base) {
        Ok(entries) => entries,
        Err(e) => {
            log::warn!(
                "[MODULE] Failed to read modules directory {}: {}",
                base.display(),
                e
            );
            return Vec::new();
        }
    };

    let mut modules = Vec::new();

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        let manifest_path = path.join("module.toml");
        if !manifest_path.exists() {
            log::debug!(
                "[MODULE] Skipping {} — no module.toml found",
                path.display()
            );
            continue;
        }

        match ModuleManifest::from_file(&manifest_path) {
            Ok(manifest) => {
                let name = manifest.module.name.clone();
                let version = manifest.module.version.clone();
                modules.push(DynamicModule::new(manifest, path));
                log::info!(
                    "[MODULE] Loaded dynamic module: {} v{} from {}",
                    name,
                    version,
                    manifest_path.display()
                );
            }
            Err(e) => {
                log::warn!(
                    "[MODULE] Failed to parse {}: {}",
                    manifest_path.display(),
                    e
                );
            }
        }
    }

    modules
}

/// Get the service binary paths for all dynamic modules.
/// Used by `start_module_services()` to spawn module processes.
pub fn get_dynamic_service_binaries() -> Vec<DynamicServiceInfo> {
    let modules = load_dynamic_modules();
    modules
        .into_iter()
        .map(|m| {
            let name = m.name().to_string();
            let port = m.default_port();
            let binary = m.binary_path();
            let port_env = m.manifest_port_env_var();
            DynamicServiceInfo {
                name,
                default_port: port,
                binary_path: binary,
                port_env_var: port_env,
            }
        })
        .collect()
}

/// Info about a dynamic module service binary for process management.
pub struct DynamicServiceInfo {
    pub name: String,
    pub default_port: u16,
    pub binary_path: PathBuf,
    pub port_env_var: Option<String>,
}
