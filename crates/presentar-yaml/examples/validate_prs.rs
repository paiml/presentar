//! Validate .prs (Presentar Scene Format) files.
//!
//! This example demonstrates how to parse and validate .prs files,
//! showing detailed information about the scene structure.
//!
//! # Usage
//!
//! ```bash
//! # Validate a single file
//! cargo run -p presentar-yaml --example validate_prs examples/prs/minimal.prs
//!
//! # Validate multiple files
//! cargo run -p presentar-yaml --example validate_prs examples/prs/*.prs
//!
//! # Validate with verbose output
//! cargo run -p presentar-yaml --example validate_prs -- -v examples/prs/sentiment-demo.prs
//! ```

use presentar_yaml::{Scene, SceneError};
use std::env;
use std::fs;
use std::path::Path;
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        eprintln!("Usage: validate_prs [-v|--verbose] <file.prs> [file2.prs ...]");
        eprintln!();
        eprintln!("Options:");
        eprintln!("  -v, --verbose    Show detailed scene information");
        eprintln!();
        eprintln!("Examples:");
        eprintln!("  validate_prs examples/prs/minimal.prs");
        eprintln!("  validate_prs -v examples/prs/sentiment-demo.prs");
        eprintln!("  validate_prs examples/prs/*.prs");
        return ExitCode::from(1);
    }

    let mut verbose = false;
    let mut files: Vec<&str> = Vec::new();

    for arg in &args[1..] {
        match arg.as_str() {
            "-v" | "--verbose" => verbose = true,
            _ => files.push(arg),
        }
    }

    if files.is_empty() {
        eprintln!("Error: No .prs files specified");
        return ExitCode::from(1);
    }

    let mut passed = 0;
    let mut failed = 0;

    for file in &files {
        match validate_file(file, verbose) {
            Ok(()) => {
                passed += 1;
                println!("\x1b[32m✓\x1b[0m {file}");
            }
            Err(e) => {
                failed += 1;
                println!("\x1b[31m✗\x1b[0m {file}");
                eprintln!("  Error: {e}");
            }
        }
    }

    println!();
    println!(
        "Results: {} passed, {} failed, {} total",
        passed,
        failed,
        passed + failed
    );

    if failed > 0 {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    }
}

fn validate_file(path: &str, verbose: bool) -> Result<(), Box<dyn std::error::Error>> {
    let path = Path::new(path);

    if !path.exists() {
        return Err(format!("File not found: {}", path.display()).into());
    }

    if path.extension().map_or(true, |ext| ext != "prs") {
        return Err("File must have .prs extension".into());
    }

    let content = fs::read_to_string(path)?;
    let scene = Scene::from_yaml(&content).map_err(|e| format_scene_error(&e))?;

    if verbose {
        print_scene_info(&scene);
    }

    Ok(())
}

fn format_scene_error(e: &SceneError) -> String {
    match e {
        SceneError::Yaml(ye) => format!("YAML parse error: {ye}"),
        SceneError::InvalidVersion(v) => {
            format!("Invalid prs_version '{v}' (expected format: X.Y)")
        }
        SceneError::DuplicateWidgetId(id) => format!("Duplicate widget ID: '{id}'"),
        SceneError::InvalidBindingTarget { trigger, target } => {
            format!("Binding '{trigger}' references unknown target '{target}'")
        }
        SceneError::InvalidHashFormat { resource, hash } => {
            format!("Invalid hash for '{resource}': '{hash}' (expected blake3:<hex>)")
        }
        SceneError::MissingRemoteHash { resource } => {
            format!("Remote resource '{resource}' requires a hash for verification")
        }
        SceneError::InvalidExpression {
            context,
            expression,
            message,
        } => {
            format!("Invalid expression in {context}: '{expression}' - {message}")
        }
        SceneError::InvalidMetadataName(name) => {
            format!("Invalid metadata name '{name}' (must be kebab-case: lowercase, numbers, hyphens)")
        }
        SceneError::LayoutError(msg) => format!("Layout error: {msg}"),
    }
}

#[allow(clippy::too_many_lines)]
fn print_scene_info(scene: &Scene) {
    println!();
    println!("  \x1b[1mScene Information\x1b[0m");
    println!("  ─────────────────");
    println!("  Version:     {}", scene.prs_version);
    println!("  Name:        {}", scene.metadata.name);

    if let Some(title) = &scene.metadata.title {
        println!("  Title:       {title}");
    }

    if let Some(author) = &scene.metadata.author {
        println!("  Author:      {author}");
    }

    if let Some(license) = &scene.metadata.license {
        println!("  License:     {license}");
    }

    if !scene.metadata.tags.is_empty() {
        println!("  Tags:        {}", scene.metadata.tags.join(", "));
    }

    println!();
    println!("  \x1b[1mLayout\x1b[0m");
    println!("  ──────");
    println!("  Type:        {:?}", scene.layout.layout_type);
    if let Some(cols) = scene.layout.columns {
        println!("  Columns:     {cols}");
    }
    if let Some(rows) = scene.layout.rows {
        println!("  Rows:        {rows}");
    }
    println!("  Gap:         {}px", scene.layout.gap);

    println!();
    println!("  \x1b[1mWidgets ({} total)\x1b[0m", scene.widgets.len());
    println!("  ─────────────────");
    for widget in &scene.widgets {
        let pos = widget
            .position
            .as_ref()
            .map(|p| format!(" @ ({}, {})", p.row, p.col))
            .unwrap_or_default();
        println!("  • {} ({:?}){}", widget.id, widget.widget_type, pos);
    }

    if !scene.resources.models.is_empty() {
        println!();
        println!(
            "  \x1b[1mModels ({} total)\x1b[0m",
            scene.resources.models.len()
        );
        println!("  ─────────────────");
        for (name, model) in &scene.resources.models {
            let hash_status = if model.hash.is_some() { "✓" } else { "○" };
            println!(
                "  • {name} ({:?}) [{hash_status}]",
                model.resource_type
            );
            println!("    Source: {}", model.source.primary());
        }
    }

    if !scene.resources.datasets.is_empty() {
        println!();
        println!(
            "  \x1b[1mDatasets ({} total)\x1b[0m",
            scene.resources.datasets.len()
        );
        println!("  ───────────────────");
        for (name, dataset) in &scene.resources.datasets {
            let hash_status = if dataset.hash.is_some() { "✓" } else { "○" };
            println!(
                "  • {name} ({:?}) [{hash_status}]",
                dataset.resource_type
            );
            println!("    Source: {}", dataset.source.primary());
        }
    }

    if !scene.bindings.is_empty() {
        println!();
        println!(
            "  \x1b[1mBindings ({} total)\x1b[0m",
            scene.bindings.len()
        );
        println!("  ──────────────────");
        for binding in &scene.bindings {
            let debounce = binding
                .debounce_ms
                .map(|ms| format!(" (debounce: {ms}ms)"))
                .unwrap_or_default();
            println!("  • {}{debounce}", binding.trigger);
            for action in &binding.actions {
                let act = action.action.as_deref().unwrap_or("update");
                println!("    → {} [{}]", action.target, act);
            }
        }
    }

    if let Some(theme) = &scene.theme {
        println!();
        println!("  \x1b[1mTheme\x1b[0m");
        println!("  ─────");
        if let Some(preset) = &theme.preset {
            println!("  Preset:      {preset}");
        }
        if !theme.custom.is_empty() {
            println!("  Custom:      {} properties", theme.custom.len());
        }
    }

    if !scene.permissions.network.is_empty()
        || !scene.permissions.filesystem.is_empty()
        || scene.permissions.clipboard
        || scene.permissions.camera
    {
        println!();
        println!("  \x1b[1mPermissions\x1b[0m");
        println!("  ───────────");
        if !scene.permissions.network.is_empty() {
            println!(
                "  Network:     {} pattern(s)",
                scene.permissions.network.len()
            );
        }
        if !scene.permissions.filesystem.is_empty() {
            println!(
                "  Filesystem:  {} pattern(s)",
                scene.permissions.filesystem.len()
            );
        }
        if scene.permissions.clipboard {
            println!("  Clipboard:   allowed");
        }
        if scene.permissions.camera {
            println!("  Camera:      allowed");
        }
    }

    println!();
}
