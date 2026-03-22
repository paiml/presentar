// build.rs — Read provable-contracts binding.yaml and emit CONTRACT_* env vars
//
// Reads contracts/*.yaml in this crate and sets env vars consumed by
// the #[contract] proc macro at compile time.

use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::Path;

/// Minimal subset of the binding.yaml schema.
#[derive(Deserialize)]
struct BindingFile {
    #[allow(dead_code)]
    version: String,
    #[allow(dead_code)]
    target_crate: String,
    bindings: Vec<Binding>,
}

#[derive(Deserialize)]
struct Binding {
    contract: String,
    equation: String,
    status: String,
    #[serde(default)]
    #[allow(dead_code)]
    notes: Option<String>,
}

/// Convert a contract filename + equation into a canonical env var name.
fn env_var_name(contract: &str, equation: &str) -> String {
    let stem = contract
        .trim_end_matches(".yaml")
        .trim_end_matches(".yml")
        .to_uppercase()
        .replace('-', "_");
    let eq = equation.to_uppercase().replace('-', "_");
    format!("CONTRACT_{stem}_{eq}")
}

fn main() {
    // Phase 1: Read binding.yaml from provable-contracts registry
    let binding_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("..")
        .join("provable-contracts")
        .join("contracts")
        .join("presentar")
        .join("binding.yaml");

    println!("cargo:rerun-if-changed={}", binding_path.display());

    if binding_path.exists() {
        if let Ok(yaml_content) = std::fs::read_to_string(&binding_path) {
            if let Ok(bindings) = serde_yaml_ng::from_str::<BindingFile>(&yaml_content) {
                let mut implemented = 0u32;
                let total = bindings.bindings.len() as u32;

                for binding in &bindings.bindings {
                    let var_name = env_var_name(&binding.contract, &binding.equation);
                    println!("cargo:rustc-env={var_name}={}", binding.status);
                    if binding.status == "implemented" {
                        implemented += 1;
                    }
                }

                println!(
                    "cargo:warning=[contract] presentar-core: {implemented}/{total} bindings implemented"
                );
                println!("cargo:rustc-env=CONTRACT_BINDING_SOURCE=binding.yaml");
            }
        }
    } else {
        println!(
            "cargo:warning=provable-contracts binding.yaml not found at {}; \
             CONTRACT_* env vars will not be set (CI/crates.io build)",
            binding_path.display()
        );
        println!("cargo:rustc-env=CONTRACT_BINDING_SOURCE=none");
    }

    // Phase 2: Read contract YAMLs and emit PRE/POST env vars for the
    // #[contract] proc macro to inject as debug_assert!() calls.
    emit_contract_assertions();
}

/// Minimal YAML contract schema for PRE/POST extraction.
#[derive(Deserialize, Default)]
struct ContractYaml {
    #[serde(default)]
    equations: BTreeMap<String, EquationYaml>,
}

#[derive(Deserialize, Default)]
struct EquationYaml {
    #[serde(default)]
    preconditions: Vec<String>,
    #[serde(default)]
    postconditions: Vec<String>,
    #[allow(dead_code)]
    #[serde(default)]
    lean_theorem: Option<String>,
}

fn emit_contract_assertions() {
    let contracts_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("contracts");
    if !contracts_dir.exists() {
        return;
    }

    let Ok(entries) = std::fs::read_dir(&contracts_dir) else {
        return;
    };

    let mut total_pre = 0usize;
    let mut total_post = 0usize;

    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("yaml") {
            continue;
        }
        println!("cargo:rerun-if-changed={}", path.display());

        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown");

        let Ok(content) = std::fs::read_to_string(&path) else {
            continue;
        };

        let contract: ContractYaml = match serde_yaml_ng::from_str(&content) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let stem_upper = stem.to_uppercase().replace('-', "_");

        for (eq_name, equation) in &contract.equations {
            let eq_upper = eq_name.to_uppercase().replace('-', "_");
            let key = format!("CONTRACT_{stem_upper}_{eq_upper}");

            let pre_count = equation.preconditions.len();
            if pre_count > 0 {
                println!("cargo:rustc-env={key}_PRE_COUNT={pre_count}");
                for (i, pre) in equation.preconditions.iter().enumerate() {
                    println!("cargo:rustc-env={key}_PRE_{i}={pre}");
                }
                total_pre += pre_count;
            }

            let post_count = equation.postconditions.len();
            if post_count > 0 {
                println!("cargo:rustc-env={key}_POST_COUNT={post_count}");
                for (i, post) in equation.postconditions.iter().enumerate() {
                    println!("cargo:rustc-env={key}_POST_{i}={post}");
                }
                total_post += post_count;
            }
        }
    }

    println!(
        "cargo:warning=[contract] Assertions: {total_pre} preconditions, \
         {total_post} postconditions from YAML"
    );
}
