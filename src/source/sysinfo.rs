use super::{ContentSource, PaneSpec};
use anyhow::Result;
use ratatui::buffer::Buffer;
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Gauge, Paragraph, Widget};
use std::time::{Duration, Instant};

/// System stats widget showing CPU, memory, disk usage with gauge bars.
pub struct SysInfoSource {
    interval: Duration,
    last_run: Option<Instant>,
    cpu_percent: f64,
    mem_used_gb: f64,
    mem_total_gb: f64,
    disk_used_pct: f64,
    disk_label: String,
    load_avg: String,
    uptime: String,
    prev_cpu_idle: u64,
    prev_cpu_total: u64,
}

impl SysInfoSource {
    pub fn new(interval_ms: u64) -> Self {
        Self {
            interval: Duration::from_millis(interval_ms),
            last_run: None,
            cpu_percent: 0.0,
            mem_used_gb: 0.0,
            mem_total_gb: 0.0,
            disk_used_pct: 0.0,
            disk_label: String::new(),
            load_avg: String::new(),
            uptime: String::new(),
            prev_cpu_idle: 0,
            prev_cpu_total: 0,
        }
    }

    fn should_refresh(&self) -> bool {
        match self.last_run {
            None => true,
            Some(t) => t.elapsed() >= self.interval,
        }
    }

    fn refresh(&mut self) {
        self.read_cpu();
        self.read_memory();
        self.read_disk();
        self.read_loadavg();
        self.read_uptime();
        self.last_run = Some(Instant::now());
    }

    fn read_cpu(&mut self) {
        if let Ok(contents) = std::fs::read_to_string("/proc/stat") {
            if let Some(line) = contents.lines().next() {
                let parts: Vec<u64> = line
                    .split_whitespace()
                    .skip(1) // skip "cpu"
                    .filter_map(|s| s.parse().ok())
                    .collect();
                if parts.len() >= 4 {
                    let idle = parts[3];
                    let total: u64 = parts.iter().sum();
                    if self.prev_cpu_total > 0 {
                        let d_total = total.saturating_sub(self.prev_cpu_total);
                        let d_idle = idle.saturating_sub(self.prev_cpu_idle);
                        if d_total > 0 {
                            self.cpu_percent = ((d_total - d_idle) as f64 / d_total as f64) * 100.0;
                        }
                    }
                    self.prev_cpu_idle = idle;
                    self.prev_cpu_total = total;
                }
            }
        }
    }

    fn read_memory(&mut self) {
        if let Ok(contents) = std::fs::read_to_string("/proc/meminfo") {
            let mut total_kb: u64 = 0;
            let mut available_kb: u64 = 0;
            for line in contents.lines() {
                if let Some(rest) = line.strip_prefix("MemTotal:") {
                    total_kb = rest
                        .split_whitespace()
                        .next()
                        .and_then(|s| s.parse().ok())
                        .unwrap_or(0);
                } else if let Some(rest) = line.strip_prefix("MemAvailable:") {
                    available_kb = rest
                        .split_whitespace()
                        .next()
                        .and_then(|s| s.parse().ok())
                        .unwrap_or(0);
                }
            }
            if total_kb > 0 {
                self.mem_total_gb = total_kb as f64 / 1_048_576.0;
                self.mem_used_gb = (total_kb - available_kb) as f64 / 1_048_576.0;
            }
        }
    }

    fn read_disk(&mut self) {
        if let Ok(output) = std::process::Command::new("df").args(["-h", "/"]).output() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            // Second line has the data
            if let Some(line) = stdout.lines().nth(1) {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 5 {
                    // parts[4] is like "42%"
                    if let Some(pct_str) = parts[4].strip_suffix('%') {
                        self.disk_used_pct = pct_str.parse().unwrap_or(0.0);
                    }
                    self.disk_label = format!("{} / {}", parts[2], parts[1]);
                }
            }
        }
    }

    fn read_loadavg(&mut self) {
        if let Ok(contents) = std::fs::read_to_string("/proc/loadavg") {
            let parts: Vec<&str> = contents.split_whitespace().collect();
            if parts.len() >= 3 {
                self.load_avg = format!("{} {} {}", parts[0], parts[1], parts[2]);
            }
        }
    }

    fn read_uptime(&mut self) {
        if let Ok(contents) = std::fs::read_to_string("/proc/uptime") {
            if let Some(secs_str) = contents.split_whitespace().next() {
                if let Ok(secs) = secs_str.parse::<f64>() {
                    let total = secs as u64;
                    let days = total / 86400;
                    let hours = (total % 86400) / 3600;
                    let mins = (total % 3600) / 60;
                    self.uptime = if days > 0 {
                        format!("{}d {}h {}m", days, hours, mins)
                    } else {
                        format!("{}h {}m", hours, mins)
                    };
                }
            }
        }
    }

    fn gauge_color(pct: f64) -> Color {
        if pct < 50.0 {
            Color::Green
        } else if pct < 80.0 {
            Color::Yellow
        } else {
            Color::Red
        }
    }
}

impl ContentSource for SysInfoSource {
    fn capture(&mut self, _width: u16, _height: u16) -> Result<String> {
        if self.should_refresh() {
            self.refresh();
        }
        Ok(format!(
            "CPU: {:.1}% | Mem: {:.1}/{:.1} GB | Disk: {:.0}% | Load: {}",
            self.cpu_percent,
            self.mem_used_gb,
            self.mem_total_gb,
            self.disk_used_pct,
            self.load_avg
        ))
    }

    fn send_keys(&mut self, _keys: &str) -> Result<()> {
        Ok(())
    }

    fn name(&self) -> &str {
        "sysinfo"
    }

    fn source_label(&self) -> &str {
        "widget"
    }

    fn is_interactive(&self) -> bool {
        false
    }

    fn to_spec(&self) -> PaneSpec {
        let mut config = toml::map::Map::new();
        config.insert(
            "interval_ms".to_string(),
            toml::Value::Integer(self.interval.as_millis() as i64),
        );
        PaneSpec::Plugin {
            plugin_name: "sysinfo".to_string(),
            config: toml::Value::Table(config),
        }
    }

    fn has_custom_render(&self) -> bool {
        true
    }

    fn render(&self, area: Rect, buf: &mut Buffer) {
        if area.height < 5 || area.width < 20 {
            return;
        }

        // Layout: title, cpu gauge, mem gauge, disk gauge, load+uptime
        let chunks = Layout::vertical([
            Constraint::Length(2), // title + spacing
            Constraint::Length(2), // CPU
            Constraint::Length(2), // Memory
            Constraint::Length(2), // Disk
            Constraint::Length(1), // Load average
            Constraint::Length(1), // Uptime
            Constraint::Min(0),    // padding
        ])
        .split(area);

        // Title
        let title = Line::from(vec![Span::styled(
            "System Stats",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )]);
        let title_para = Paragraph::new(title).alignment(Alignment::Center);
        Widget::render(title_para, chunks[0], buf);

        // CPU gauge
        let cpu_pct = self.cpu_percent.clamp(0.0, 100.0) as u16;
        let cpu_label = format!("CPU  {:.1}%", self.cpu_percent);
        let cpu_gauge = Gauge::default()
            .gauge_style(Style::default().fg(Self::gauge_color(self.cpu_percent)))
            .percent(cpu_pct)
            .label(cpu_label);
        Widget::render(cpu_gauge, chunks[1], buf);

        // Memory gauge
        let mem_pct = if self.mem_total_gb > 0.0 {
            ((self.mem_used_gb / self.mem_total_gb) * 100.0).clamp(0.0, 100.0)
        } else {
            0.0
        };
        let mem_label = format!(
            "Mem  {:.1} / {:.1} GB ({:.0}%)",
            self.mem_used_gb, self.mem_total_gb, mem_pct
        );
        let mem_gauge = Gauge::default()
            .gauge_style(Style::default().fg(Self::gauge_color(mem_pct)))
            .percent(mem_pct as u16)
            .label(mem_label);
        Widget::render(mem_gauge, chunks[2], buf);

        // Disk gauge
        let disk_pct = self.disk_used_pct.clamp(0.0, 100.0) as u16;
        let disk_label = format!("Disk {} ({:.0}%)", self.disk_label, self.disk_used_pct);
        let disk_gauge = Gauge::default()
            .gauge_style(Style::default().fg(Self::gauge_color(self.disk_used_pct)))
            .percent(disk_pct)
            .label(disk_label);
        Widget::render(disk_gauge, chunks[3], buf);

        // Load average
        let load_line = Line::from(vec![
            Span::styled("Load: ", Style::default().fg(Color::DarkGray)),
            Span::styled(self.load_avg.clone(), Style::default().fg(Color::White)),
        ]);
        let load_para = Paragraph::new(load_line).alignment(Alignment::Center);
        Widget::render(load_para, chunks[4], buf);

        // Uptime
        let up_line = Line::from(vec![
            Span::styled("Uptime: ", Style::default().fg(Color::DarkGray)),
            Span::styled(self.uptime.clone(), Style::default().fg(Color::White)),
        ]);
        let up_para = Paragraph::new(up_line).alignment(Alignment::Center);
        Widget::render(up_para, chunks[5], buf);
    }
}
