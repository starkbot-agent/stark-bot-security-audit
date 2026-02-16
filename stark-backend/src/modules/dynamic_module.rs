//! DynamicModule — a Module implementation loaded from a `module.toml` manifest
//! at runtime. No compiled module-specific code needed.

use async_trait::async_trait;
use crate::tools::registry::Tool;
use std::path::PathBuf;
use std::sync::Arc;

use super::dynamic_tool::DynamicModuleTool;
use super::manifest::ModuleManifest;

/// A module loaded dynamically from a manifest file on disk.
pub struct DynamicModule {
    manifest: ModuleManifest,
    /// Directory containing the module (e.g. ~/.starkbot/modules/wallet_monitor/)
    module_dir: PathBuf,
    /// Cached skill content (loaded once from disk)
    skill_content: Option<String>,
}

impl DynamicModule {
    /// Create a DynamicModule from a parsed manifest and its containing directory.
    pub fn new(manifest: ModuleManifest, module_dir: PathBuf) -> Self {
        // Pre-load skill content if configured
        let skill_content = manifest.skill.as_ref().and_then(|skill_cfg| {
            let skill_path = module_dir.join(&skill_cfg.content_file);
            std::fs::read_to_string(&skill_path)
                .map_err(|e| {
                    log::warn!(
                        "[MODULE] Failed to read skill file {}: {}",
                        skill_path.display(),
                        e
                    );
                    e
                })
                .ok()
        });

        DynamicModule {
            manifest,
            module_dir,
            skill_content,
        }
    }

    /// Path to the service binary for this module.
    pub fn binary_path(&self) -> PathBuf {
        self.module_dir
            .join("bin")
            .join(format!("{}-service", self.manifest.module.name))
    }

    /// The directory this module was loaded from.
    pub fn module_dir(&self) -> &PathBuf {
        &self.module_dir
    }

    /// The manifest path.
    pub fn manifest_path(&self) -> PathBuf {
        self.module_dir.join("module.toml")
    }

    /// Author from manifest (if present).
    pub fn author(&self) -> Option<&str> {
        self.manifest.module.author.as_deref()
    }

    /// Port env var from manifest (if present).
    pub fn manifest_port_env_var(&self) -> Option<String> {
        self.manifest.service.port_env_var.clone()
    }
}

#[async_trait]
impl super::Module for DynamicModule {
    fn name(&self) -> &str {
        &self.manifest.module.name
    }

    fn description(&self) -> &str {
        &self.manifest.module.description
    }

    fn version(&self) -> &str {
        &self.manifest.module.version
    }

    fn default_port(&self) -> u16 {
        self.manifest.service.default_port
    }

    fn service_url(&self) -> String {
        self.manifest.service_url()
    }

    fn has_tools(&self) -> bool {
        !self.manifest.tools.is_empty()
    }

    fn has_dashboard(&self) -> bool {
        self.manifest.service.has_dashboard
    }

    fn create_tools(&self) -> Vec<Arc<dyn Tool>> {
        let base_url = self.service_url();
        self.manifest
            .tools
            .iter()
            .map(|tool_manifest| {
                Arc::new(DynamicModuleTool::from_manifest(tool_manifest, &base_url)) as Arc<dyn Tool>
            })
            .collect()
    }

    fn skill_content(&self) -> Option<&str> {
        self.skill_content.as_deref()
    }
}
