//! Waybar integration for OpenUsage.
//!
//! When the binary is invoked with `--waybar`, this module loads all
//! configured plugins, runs their probes synchronously, and prints a
//! single-line JSON payload to stdout that is compatible with waybar's
//! `custom/script` module (return-type = "json").
//!
//! # Waybar config snippet
//!
//! ```jsonc
//! "custom/openusage": {
//!     "exec": "openusage --waybar",
//!     "interval": 300,
//!     "return-type": "json",
//!     "on-click": "openusage",
//!     "format": "󰄛 {}"
//! }
//! ```
//!
//! # Output format
//!
//! ```json
//! {"text":"Claude 42%  Cursor 78%","tooltip":"...","class":"warning","percentage":78}
//! ```
//!
//! CSS classes emitted:
//! - `ok`       — all providers below 80 %
//! - `warning`  — at least one provider at or above 80 %
//! - `critical` — at least one provider at or above 95 %

use crate::plugin_engine;
use crate::plugin_engine::runtime::{MetricLine, PluginOutput, ProgressFormat};
use std::path::PathBuf;

const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

// ── path helpers ──────────────────────────────────────────────────────────────

/// XDG-compliant app data directory: `$XDG_DATA_HOME/openusage`.
fn app_data_dir() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| {
            dirs::home_dir()
                .unwrap_or_else(|| PathBuf::from("/tmp"))
                .join(".local/share")
        })
        .join("openusage")
}

/// Try to locate the resource directory next to the installed binary.
/// Falls back to the current directory so that development runs still work.
fn resource_dir() -> PathBuf {
    if let Ok(exe) = std::env::current_exe() {
        let exe_dir = exe.parent().unwrap_or(&exe).to_path_buf();
        let candidates = [
            exe_dir.join("resources"),
            exe_dir
                .parent()
                .unwrap_or(&exe_dir)
                .join("share/openusage"),
            PathBuf::from("/usr/share/openusage"),
            PathBuf::from("/usr/local/share/openusage"),
            exe_dir,
        ];
        for candidate in &candidates {
            if candidate.exists() {
                return candidate.clone();
            }
        }
    }
    PathBuf::from(".")
}

// ── formatting helpers ────────────────────────────────────────────────────────

fn format_progress(used: f64, limit: f64, format: &ProgressFormat) -> String {
    let pct = if limit > 0.0 {
        (used / limit * 100.0).round() as u64
    } else {
        0
    };
    match format {
        ProgressFormat::Percent => format!("{}%", pct),
        ProgressFormat::Dollars => format!("${:.2}/${:.2}", used, limit),
        ProgressFormat::Count { suffix } => {
            format!("{}/{}{}", used as u64, limit as u64, suffix)
        }
    }
}

/// Highest usage percentage across all providers (Percent lines only).
fn max_percent(outputs: &[PluginOutput]) -> u64 {
    outputs
        .iter()
        .flat_map(|o| o.lines.iter())
        .filter_map(|line| {
            if let MetricLine::Progress { used, limit, format, .. } = line {
                if matches!(format, ProgressFormat::Percent) && *limit > 0.0 {
                    Some((used / limit * 100.0).round() as u64)
                } else {
                    None
                }
            } else {
                None
            }
        })
        .max()
        .unwrap_or(0)
}

fn css_class(pct: u64) -> &'static str {
    if pct >= 95 {
        "critical"
    } else if pct >= 80 {
        "warning"
    } else {
        "ok"
    }
}

/// Short bar text: "<Provider> <primary-metric>" for each provider, separated
/// by two spaces so waybar can render them inline.
fn build_text(outputs: &[PluginOutput]) -> String {
    let parts: Vec<String> = outputs
        .iter()
        .filter_map(|output| {
            // Pick the first progress-percent line (highest-priority metric).
            output
                .lines
                .iter()
                .find(|l| matches!(l, MetricLine::Progress { format: ProgressFormat::Percent, .. }))
                .map(|line| {
                    if let MetricLine::Progress { used, limit, format, .. } = line {
                        format!("{} {}", output.display_name, format_progress(*used, *limit, format))
                    } else {
                        output.display_name.clone()
                    }
                })
        })
        .collect();

    if parts.is_empty() {
        "OpenUsage".to_string()
    } else {
        parts.join("  ")
    }
}

/// Multi-line tooltip rendered on hover (Pango markup supported by waybar).
fn build_tooltip(outputs: &[PluginOutput]) -> String {
    let sections: Vec<String> = outputs
        .iter()
        .map(|output| {
            let mut lines = vec![format!("<b>{}</b>", output.display_name)];

            let has_error = output.lines.iter().any(|l| {
                matches!(l, MetricLine::Badge { label, .. } if label == "Error")
            });

            if has_error {
                lines.push("  ⚠ Error fetching data".to_string());
            } else {
                for line in &output.lines {
                    match line {
                        MetricLine::Progress { label, used, limit, format, .. } => {
                            lines.push(format!(
                                "  {}: {}",
                                label,
                                format_progress(*used, *limit, format)
                            ));
                        }
                        MetricLine::Text { label, value, .. } => {
                            lines.push(format!("  {}: {}", label, value));
                        }
                        MetricLine::Badge { label, text, .. } => {
                            lines.push(format!("  {}: {}", label, text));
                        }
                    }
                }
            }
            lines.join("\n")
        })
        .collect();

    if sections.is_empty() {
        "No providers configured.\nRun openusage to set up providers.".to_string()
    } else {
        sections.join("\n\n")
    }
}

// ── entry point ───────────────────────────────────────────────────────────────

/// Run in waybar mode: probe all plugins and print JSON to stdout.
pub fn run() {
    let data_dir = app_data_dir();
    let res_dir = resource_dir();

    let (_, plugins) = plugin_engine::initialize_plugins(&data_dir, &res_dir);

    if plugins.is_empty() {
        println!(
            r#"{{"text":"OpenUsage","tooltip":"No providers found.\nRun openusage to configure.","class":"ok","percentage":0}}"#
        );
        return;
    }

    let outputs: Vec<PluginOutput> = plugins
        .iter()
        .map(|p| plugin_engine::runtime::run_probe(p, &data_dir, APP_VERSION))
        .collect();

    let text = build_text(&outputs);
    let tooltip = build_tooltip(&outputs);
    let pct = max_percent(&outputs);
    let class = css_class(pct);

    // Minimal JSON escaping for the two string fields.
    let escape = |s: String| s.replace('\\', "\\\\").replace('"', "\\\"").replace('\n', "\\n");

    println!(
        r#"{{"text":"{}","tooltip":"{}","class":"{}","percentage":{}}}"#,
        escape(text),
        escape(tooltip),
        class,
        pct
    );
}
