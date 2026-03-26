use super::ContentSource;

type PluginConstructor = Box<dyn Fn(toml::Value) -> Box<dyn ContentSource> + Send + Sync>;

/// Metadata for a registered plugin/app.
pub struct PluginInfo {
    pub name: &'static str,
    pub description: &'static str,
    pub usage: &'static str,
    constructor: PluginConstructor,
}

/// A registry that maps plugin names to constructors and metadata.
pub struct PluginRegistry {
    plugins: Vec<PluginInfo>,
}

impl PluginRegistry {
    pub fn new() -> Self {
        let mut reg = Self {
            plugins: Vec::new(),
        };

        reg.add(
            "clock",
            "Live clock display",
            "clock:",
            Box::new(|_| Box::new(super::clock::ClockSource)),
        );
        reg.add(
            "weather",
            "Weather from wttr.in with color-coded temperature",
            "weather:City or weather:City:interval_ms",
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
        reg.add(
            "sysinfo",
            "CPU, memory, disk gauges with load average",
            "sysinfo: or sysinfo:interval_ms",
            Box::new(|config| {
                let interval_ms = config
                    .get("interval_ms")
                    .and_then(|v| v.as_integer())
                    .unwrap_or(2000) as u64;
                Box::new(super::sysinfo::SysInfoSource::new(interval_ms))
            }),
        );
        reg.add(
            "snake",
            "Playable snake game (arrow keys to steer)",
            "snake:",
            Box::new(|_| Box::new(super::snake::SnakeSource::new())),
        );
        reg.add(
            "sparkline",
            "Real-time sparkline chart from command output",
            "spark:command:interval_ms",
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

        reg.add(
            "settings",
            "Settings panel — bindings, remotes, theme, about",
            "settings:",
            Box::new(|_| {
                let config = crate::config::load().unwrap_or_default();
                Box::new(super::settings::SettingsSource::from_config(&config))
            }),
        );

        // Built-in non-widget sources (for documentation only)
        reg.add(
            "watch",
            "Run a command periodically and display output",
            "watch:command:interval_ms",
            Box::new(|_| Box::new(super::clock::ClockSource)), // placeholder
        );
        reg.add(
            "tail",
            "Follow a file with tail -f",
            "tail:/path/to/file",
            Box::new(|_| Box::new(super::clock::ClockSource)),
        );
        reg.add(
            "http",
            "Poll an HTTP URL periodically",
            "http:url:interval_ms",
            Box::new(|_| Box::new(super::clock::ClockSource)),
        );

        reg
    }

    fn add(
        &mut self,
        name: &'static str,
        description: &'static str,
        usage: &'static str,
        constructor: PluginConstructor,
    ) {
        self.plugins.push(PluginInfo {
            name,
            description,
            usage,
            constructor,
        });
    }

    pub fn create(&self, name: &str, config: toml::Value) -> Option<Box<dyn ContentSource>> {
        self.plugins
            .iter()
            .find(|p| p.name == name)
            .map(|p| (p.constructor)(config))
    }

    pub fn list(&self) -> &[PluginInfo] {
        &self.plugins
    }
}
