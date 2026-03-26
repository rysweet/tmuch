use std::collections::HashMap;

use super::ContentSource;

type PluginConstructor = Box<dyn Fn(toml::Value) -> Box<dyn ContentSource> + Send + Sync>;

/// A registry that maps plugin names to constructors for creating content sources.
pub struct PluginRegistry {
    plugins: HashMap<String, PluginConstructor>,
}

impl PluginRegistry {
    pub fn new() -> Self {
        let mut reg = Self {
            plugins: HashMap::new(),
        };
        // Register built-in plugins
        reg.register(
            "clock",
            Box::new(|_config| Box::new(super::clock::ClockSource)),
        );
        reg
    }

    pub fn register(&mut self, name: &str, constructor: PluginConstructor) {
        self.plugins.insert(name.to_string(), constructor);
    }

    pub fn create(&self, name: &str, config: toml::Value) -> Option<Box<dyn ContentSource>> {
        self.plugins.get(name).map(|ctor| ctor(config))
    }
}
