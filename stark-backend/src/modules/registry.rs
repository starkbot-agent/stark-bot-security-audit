//! Module registry — holds all available modules (built-in + dynamic)

use super::discord_tipping::DiscordTippingModule;
use super::loader;
use super::social_monitor::SocialMonitorModule;
use super::wallet_monitor::WalletMonitorModule;
use super::Module;
use std::collections::HashMap;

/// Registry of all available modules (compiled + dynamically loaded)
pub struct ModuleRegistry {
    modules: HashMap<String, Box<dyn Module>>,
}

impl ModuleRegistry {
    /// Create a new registry with built-in modules + dynamically loaded modules
    /// from `~/.starkbot/modules/`.
    pub fn new() -> Self {
        let mut reg = Self {
            modules: HashMap::new(),
        };

        // Built-in (compiled) modules
        reg.register(Box::new(WalletMonitorModule));
        reg.register(Box::new(DiscordTippingModule));
        reg.register(Box::new(SocialMonitorModule));

        // Dynamic modules from ~/.starkbot/modules/
        let dynamic = loader::load_dynamic_modules();
        for module in dynamic {
            let name = module.name().to_string();
            if reg.modules.contains_key(&name) {
                log::warn!(
                    "[MODULE] Dynamic module '{}' conflicts with built-in module — skipping",
                    name
                );
                continue;
            }
            log::info!("[MODULE] Registered dynamic module: {}", name);
            reg.modules.insert(name, Box::new(module));
        }

        reg
    }

    fn register(&mut self, module: Box<dyn Module>) {
        let name = module.name().to_string();
        self.modules.insert(name, module);
    }

    /// Get a module by name
    pub fn get(&self, name: &str) -> Option<&dyn Module> {
        self.modules.get(name).map(|m| m.as_ref())
    }

    /// List all available modules
    pub fn available_modules(&self) -> Vec<&dyn Module> {
        self.modules.values().map(|m| m.as_ref()).collect()
    }
}
