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
        reg.register(
            "weather",
            Box::new(|config| {
                let city = config
                    .get("city")
                    .and_then(|v| v.as_str())
                    .unwrap_or("London")
                    .to_string();
                let interval_ms = config
                    .get("interval_ms")
                    .and_then(|v| v.as_integer())
                    .unwrap_or(300_000) as u64;
                Box::new(super::weather::WeatherSource::new(city, interval_ms))
            }),
        );
        reg.register(
            "sysinfo",
            Box::new(|config| {
                let interval_ms = config
                    .get("interval_ms")
                    .and_then(|v| v.as_integer())
                    .unwrap_or(2000) as u64;
                Box::new(super::sysinfo::SysInfoSource::new(interval_ms))
            }),
        );
        reg.register(
            "snake",
            Box::new(|_config| Box::new(super::snake::SnakeSource::new())),
        );
        reg.register(
            "sparkline",
            Box::new(|config| {
                let command = config
                    .get("command")
                    .and_then(|v| v.as_str())
                    .unwrap_or("cat /proc/loadavg | cut -d' ' -f1")
                    .to_string();
                let interval_ms = config
                    .get("interval_ms")
                    .and_then(|v| v.as_integer())
                    .unwrap_or(2000) as u64;
                Box::new(super::sparkline_monitor::SparklineSource::new(
                    command,
                    interval_ms,
                ))
            }),
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
