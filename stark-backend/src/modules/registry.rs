//! Module registry — holds all available (compilable) modules

use super::discord_tipping::DiscordTippingModule;
use super::wallet_monitor::WalletMonitorModule;
use super::Module;
use std::collections::HashMap;

/// Registry of all available modules (compiled into the binary)
pub struct ModuleRegistry {
    modules: HashMap<String, Box<dyn Module>>,
}

impl ModuleRegistry {
    /// Create a new registry with all known modules
    pub fn new() -> Self {
        let mut reg = Self {
            modules: HashMap::new(),
        };
        reg.register(Box::new(WalletMonitorModule));
        reg.register(Box::new(DiscordTippingModule));
        // Future: reg.register(Box::new(CopyTradeModule));
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

    /// List all available module names
    pub fn available_modules(&self) -> Vec<&dyn Module> {
        self.modules.values().map(|m| m.as_ref()).collect()
    }
}
