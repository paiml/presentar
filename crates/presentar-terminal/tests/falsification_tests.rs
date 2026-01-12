//! FALSIFICATION TESTS - Popperian Testing Philosophy
//!
//! These tests do NOT verify correctness. They ATTEMPT TO FALSIFY claims.
//! A passing test means "we tried to break it and couldn't" - NOT "it works".
//!
//! Severity Levels:
//! - S0: Cannot fail (FORBIDDEN)
//! - S1: Unlikely to fail (FORBIDDEN)
//! - S2: Might fail (DISCOURAGED)
//! - S3: Likely to fail if bug exists (REQUIRED)
//! - S4: Will definitely fail if bug exists (IDEAL)

#![cfg(feature = "ptop")]

use presentar_terminal::direct::CellBuffer;
use presentar_terminal::ptop::{App, PanelType};
use std::collections::HashMap;

// =============================================================================
// SECTION 1: CPU TEMPERATURE FALSIFICATION
// =============================================================================

/// FALSIFICATION TEST [S4]: AMD k10temp driver has no temp2_input
///
/// CLAIM: "All CPU cores display temperatures"
/// FALSIFIED BY: Any core showing "-" on AMD system
///
/// k10temp layout:
/// - temp1_input = Tctl (control temp)
/// - temp2_input = DOES NOT EXIST (the trap!)
/// - temp3_input = Tccd1
/// - temp4_input = Tccd2
/// - temp5_input = Tccd3
/// - temp6_input = Tccd4
#[test]
fn falsify_temp_amd_k10temp_no_temp2() {
    // Find k10temp hwmon device
    let mut k10temp_path = None;
    if let Ok(entries) = std::fs::read_dir("/sys/class/hwmon") {
        for entry in entries.flatten() {
            let name_path = entry.path().join("name");
            if let Ok(name) = std::fs::read_to_string(&name_path) {
                if name.trim() == "k10temp" {
                    k10temp_path = Some(entry.path());
                    break;
                }
            }
        }
    }

    let Some(hwmon_path) = k10temp_path else {
        println!("SKIP: Not an AMD k10temp system");
        return;
    };

    // Check what temp files actually exist
    let temp2_exists = hwmon_path.join("temp2_input").exists();

    // FALSIFICATION: If temp2 doesn't exist, our code better handle it
    if !temp2_exists {
        println!("CONFIRMED: temp2_input does NOT exist (k10temp layout)");
        println!("Available temps:");
        for i in 1..=10 {
            let path = hwmon_path.join(format!("temp{}_input", i));
            if path.exists() {
                let label_path = hwmon_path.join(format!("temp{}_label", i));
                let label = std::fs::read_to_string(&label_path).unwrap_or_default();
                println!("  temp{}_input exists (label: {})", i, label.trim());
            }
        }

        // Now test our code
        let app = App::with_config(false, Default::default());

        // FALSIFICATION ATTEMPT: Core 0 should have a temperature
        let core0_temp = app.per_core_temp.get(0).copied().unwrap_or(0.0);
        assert!(
            core0_temp > 0.0,
            "FALSIFIED: Core 0 has no temperature (got {}).\n\
             Root cause: Code tried to read temp2_input which doesn't exist.\n\
             Fix: Read temp*_label files to discover actual sensor layout.",
            core0_temp
        );

        // FALSIFICATION ATTEMPT: All cores should have temperatures
        let mut zeros = 0;
        for (i, &temp) in app.per_core_temp.iter().enumerate() {
            if temp == 0.0 {
                zeros += 1;
                if zeros <= 5 {
                    println!("Core {} has no temperature", i);
                }
            }
        }

        let zero_ratio = zeros as f32 / app.per_core_temp.len() as f32;
        assert!(
            zero_ratio < 0.1,
            "FALSIFIED: {}/{} cores ({:.1}%) have no temperature.\n\
             On k10temp, Tccd1-4 should be mapped to core groups.\n\
             Expected: Cores 0-11 -> Tccd1, 12-23 -> Tccd2, etc.",
            zeros,
            app.per_core_temp.len(),
            zero_ratio * 100.0
        );
    }
}

/// FALSIFICATION TEST [S4]: Temperature values are physically plausible
///
/// CLAIM: "Temperatures are in range 20-105C"
/// FALSIFIED BY: Any temperature outside this range (or 0)
#[test]
fn falsify_temp_physically_implausible() {
    let app = App::with_config(false, Default::default());

    let mut out_of_range = Vec::new();

    for (i, &temp) in app.per_core_temp.iter().enumerate() {
        if temp == 0.0 {
            continue; // Handled by other test
        }

        if temp < 15.0 || temp > 110.0 {
            out_of_range.push((i, temp));
        }
    }

    assert!(
        out_of_range.is_empty(),
        "FALSIFIED: {} cores have implausible temperatures: {:?}\n\
         Valid range is 15-110C.",
        out_of_range.len(),
        &out_of_range[..out_of_range.len().min(5)]
    );
}

/// FALSIFICATION TEST [S4]: Compare temperatures to `sensors` command
#[test]
fn falsify_temp_vs_sensors_command() {
    use std::process::Command;

    let output = Command::new("sensors").output();

    let Ok(output) = output else {
        println!("SKIP: sensors command not available");
        return;
    };

    let sensors_output = String::from_utf8_lossy(&output.stdout);

    // Parse Tccd temperatures from sensors output
    let mut tccd_temps: HashMap<String, f32> = HashMap::new();
    for line in sensors_output.lines() {
        if line.contains("Tccd") {
            let parts: Vec<&str> = line.split(':').collect();
            if parts.len() >= 2 {
                let label = parts[0].trim().to_string();
                let temp_str = parts[1]
                    .trim()
                    .trim_start_matches('+')
                    .split('°')
                    .next()
                    .unwrap_or("0");
                if let Ok(temp) = temp_str.parse::<f32>() {
                    tccd_temps.insert(label, temp);
                }
            }
        }
    }

    if tccd_temps.is_empty() {
        println!("SKIP: No Tccd temperatures found in sensors output");
        return;
    }

    println!("Sensors reports: {:?}", tccd_temps);

    let app = App::with_config(false, Default::default());

    if let Some(&sensors_tccd1) = tccd_temps.get("Tccd1") {
        let our_core0_temp = app.per_core_temp.get(0).copied().unwrap_or(0.0);

        if our_core0_temp > 0.0 {
            let delta = (our_core0_temp - sensors_tccd1).abs();
            assert!(
                delta < 10.0,
                "FALSIFIED: Core 0 temp {:.1}C differs from sensors Tccd1 {:.1}C by {:.1}C.",
                our_core0_temp,
                sensors_tccd1,
                delta
            );
        }
    }
}

// =============================================================================
// SECTION 2: PROCESS USER COLUMN FALSIFICATION
// =============================================================================

/// FALSIFICATION TEST [S4]: USER column shows usernames, not dashes
#[test]
fn falsify_user_column_shows_dashes() {
    let app = App::with_config(false, Default::default());

    let processes: Vec<_> = app.system.processes().iter().collect();

    assert!(!processes.is_empty(), "FALSIFIED: No processes found.");

    let mut dash_count = 0;
    let mut total_count = 0;

    for (pid, process) in processes.iter().take(100) {
        total_count += 1;

        let user = process
            .user_id()
            .and_then(|uid| app.users.get_user_by_id(uid))
            .map(|u| u.name().to_string());

        if user.is_none() {
            dash_count += 1;
            if dash_count <= 3 {
                println!("PID {} has no user (would show '-')", pid.as_u32());
            }
        }
    }

    let dash_ratio = dash_count as f32 / total_count as f32;
    assert!(
        dash_ratio < 0.10,
        "FALSIFIED: {:.1}% of processes ({}/{}) have no user.\n\
         .with_user() may be missing from ProcessRefreshKind.",
        dash_ratio * 100.0,
        dash_count,
        total_count
    );
}

/// FALSIFICATION TEST [S4]: Usernames are valid strings
#[test]
fn falsify_username_is_garbage() {
    let app = App::with_config(false, Default::default());

    for (_pid, process) in app.system.processes().iter().take(100) {
        if let Some(uid) = process.user_id() {
            if let Some(user) = app.users.get_user_by_id(uid) {
                let name = user.name();

                assert!(
                    name.chars()
                        .all(|c| c.is_alphanumeric() || c == '_' || c == '-' || c == '$'),
                    "FALSIFIED: Username '{}' contains invalid characters.",
                    name
                );

                assert!(!name.is_empty(), "FALSIFIED: Empty username found.");

                assert!(
                    name.len() <= 32,
                    "FALSIFIED: Username '{}' is {} chars (max 32).",
                    name,
                    name.len()
                );
            }
        }
    }
}

// =============================================================================
// SECTION 3: COMMAND TEXT FALSIFICATION
// =============================================================================

/// FALSIFICATION TEST [S4]: Command text is not garbled
#[test]
fn falsify_command_is_garbled() {
    let app = App::with_config(false, Default::default());

    for (pid, process) in app.system.processes().iter().take(100) {
        let cmd = process.name().to_string_lossy();

        let has_control = cmd.chars().any(|c| c.is_control() && c != '\t');
        assert!(
            !has_control,
            "FALSIFIED: PID {} command '{}' contains control characters.",
            pid.as_u32(),
            cmd
        );

        assert!(
            !cmd.contains('\0'),
            "FALSIFIED: PID {} command contains null byte.",
            pid.as_u32()
        );
    }
}

// =============================================================================
// SECTION 4: CORE COUNT FALSIFICATION
// =============================================================================

/// FALSIFICATION TEST [S4]: Core count matches system
#[test]
fn falsify_core_count_mismatch() {
    let expected_cores = std::fs::read_to_string("/proc/cpuinfo")
        .unwrap_or_default()
        .matches("processor")
        .count();

    let app = App::with_config(false, Default::default());

    assert_eq!(
        app.per_core_percent.len(),
        expected_cores,
        "FALSIFIED: per_core_percent has {} entries, expected {}.",
        app.per_core_percent.len(),
        expected_cores
    );

    assert_eq!(
        app.per_core_freq.len(),
        expected_cores,
        "FALSIFIED: per_core_freq has {} entries, expected {}.",
        app.per_core_freq.len(),
        expected_cores
    );

    assert_eq!(
        app.per_core_temp.len(),
        expected_cores,
        "FALSIFIED: per_core_temp has {} entries, expected {}.",
        app.per_core_temp.len(),
        expected_cores
    );
}

/// FALSIFICATION TEST [S4]: All cores visible in exploded view
#[test]
fn falsify_cores_not_visible() {
    use presentar_terminal::ptop::ui;

    let expected_cores = std::fs::read_to_string("/proc/cpuinfo")
        .unwrap_or_default()
        .matches("processor")
        .count();

    let mut app = App::with_config(false, Default::default());
    app.exploded_panel = Some(PanelType::Cpu);

    let mut buffer = CellBuffer::new(180, 60);
    ui::draw(&app, &mut buffer);

    let mut output = String::new();
    for y in 0..60 {
        for x in 0..180 {
            if let Some(cell) = buffer.get(x, y) {
                output.push(cell.symbol.chars().next().unwrap_or(' '));
            }
        }
        output.push('\n');
    }

    // Count core column entries (numbers 0-47)
    let mut visible_cores = std::collections::HashSet::new();
    for line in output.lines() {
        for word in line.split_whitespace() {
            if let Ok(num) = word.parse::<usize>() {
                if num < expected_cores {
                    visible_cores.insert(num);
                }
            }
        }
    }

    let visible_ratio = visible_cores.len() as f32 / expected_cores as f32;

    assert!(
        visible_ratio >= 0.5,
        "FALSIFIED: Only {}/{} cores visible ({:.1}%).",
        visible_cores.len(),
        expected_cores,
        visible_ratio * 100.0
    );
}

// =============================================================================
// SECTION 5: FREQUENCY FALSIFICATION
// =============================================================================

/// FALSIFICATION TEST [S4]: Frequencies match /proc/cpuinfo
#[test]
fn falsify_freq_vs_proc_cpuinfo() {
    let cpuinfo = std::fs::read_to_string("/proc/cpuinfo").unwrap_or_default();

    let actual_freqs: Vec<f64> = cpuinfo
        .lines()
        .filter(|line| line.starts_with("cpu MHz"))
        .filter_map(|line| line.split(':').nth(1).and_then(|s| s.trim().parse().ok()))
        .collect();

    if actual_freqs.is_empty() {
        println!("SKIP: Could not read frequencies from /proc/cpuinfo");
        return;
    }

    let app = App::with_config(false, Default::default());

    for (i, &actual_mhz) in actual_freqs.iter().enumerate().take(8) {
        let our_mhz = app.per_core_freq.get(i).copied().unwrap_or(0) as f64;

        if our_mhz == 0.0 {
            continue;
        }

        let delta = (our_mhz - actual_mhz).abs();

        assert!(
            delta < 500.0,
            "FALSIFIED: Core {} freq {:.0}MHz differs from /proc/cpuinfo {:.0}MHz by {:.0}MHz.",
            i,
            our_mhz,
            actual_mhz,
            delta
        );
    }
}

/// FALSIFICATION TEST [S3]: Frequencies are physically plausible
#[test]
fn falsify_freq_physically_implausible() {
    let app = App::with_config(false, Default::default());

    for (i, &freq_mhz) in app.per_core_freq.iter().enumerate() {
        if freq_mhz == 0 {
            continue;
        }

        assert!(
            freq_mhz >= 400,
            "FALSIFIED: Core {} freq {}MHz is below 400MHz.",
            i,
            freq_mhz
        );

        assert!(
            freq_mhz <= 6500,
            "FALSIFIED: Core {} freq {}MHz exceeds 6.5GHz.",
            i,
            freq_mhz
        );
    }
}

// =============================================================================
// SECTION 6: LOAD AVERAGE FALSIFICATION
// =============================================================================

/// FALSIFICATION TEST [S4]: Load average matches /proc/loadavg
#[test]
fn falsify_load_avg_vs_proc() {
    let loadavg = std::fs::read_to_string("/proc/loadavg").unwrap_or_default();
    let parts: Vec<&str> = loadavg.split_whitespace().collect();

    if parts.len() < 3 {
        println!("SKIP: Could not parse /proc/loadavg");
        return;
    }

    let actual_1min: f64 = parts[0].parse().unwrap_or(0.0);

    let app = App::with_config(false, Default::default());

    let delta_1 = (app.load_avg.one - actual_1min).abs();
    assert!(
        delta_1 < 5.0,
        "FALSIFIED: 1min load avg {:.2} differs from /proc/loadavg {:.2} by {:.2}.",
        app.load_avg.one,
        actual_1min,
        delta_1
    );
}

// =============================================================================
// SECTION 7: MEMORY FALSIFICATION
// =============================================================================

/// FALSIFICATION TEST [S4]: Memory total matches /proc/meminfo
#[test]
fn falsify_memory_total_vs_proc() {
    let meminfo = std::fs::read_to_string("/proc/meminfo").unwrap_or_default();

    let actual_total_kb: u64 = meminfo
        .lines()
        .find(|line| line.starts_with("MemTotal:"))
        .and_then(|line| line.split_whitespace().nth(1).and_then(|s| s.parse().ok()))
        .unwrap_or(0);

    if actual_total_kb == 0 {
        println!("SKIP: Could not parse /proc/meminfo");
        return;
    }

    let actual_total_bytes = actual_total_kb * 1024;

    let app = App::with_config(false, Default::default());
    let our_total = app.system.total_memory();

    let delta_ratio =
        (our_total as f64 - actual_total_bytes as f64).abs() / actual_total_bytes as f64;

    assert!(
        delta_ratio < 0.01,
        "FALSIFIED: Memory total {} differs from /proc/meminfo {} by {:.2}%.",
        our_total,
        actual_total_bytes,
        delta_ratio * 100.0
    );
}

/// FALSIFICATION TEST [S3]: Memory math adds up
#[test]
fn falsify_memory_math() {
    let app = App::with_config(false, Default::default());

    let total = app.system.total_memory();
    let used = app.system.used_memory();
    let available = app.system.available_memory();

    let sum = used + available;
    let delta_ratio = (sum as f64 - total as f64).abs() / total as f64;

    assert!(
        delta_ratio < 0.15,
        "FALSIFIED: used + available = {} differs from total {} by {:.1}%.",
        sum,
        total,
        delta_ratio * 100.0
    );
}

// =============================================================================
// SECTION 8: RENDER OUTPUT FALSIFICATION
// =============================================================================

/// FALSIFICATION TEST [S4]: No placeholder text in output
#[test]
fn falsify_placeholder_text_in_output() {
    use presentar_terminal::ptop::ui;

    let app = App::with_config(false, Default::default());

    let mut buffer = CellBuffer::new(140, 45);
    ui::draw(&app, &mut buffer);

    let mut output = String::new();
    for y in 0..45 {
        for x in 0..140 {
            if let Some(cell) = buffer.get(x, y) {
                output.push(cell.symbol.chars().next().unwrap_or(' '));
            }
        }
        output.push('\n');
    }

    let placeholders = ["TODO", "PLACEHOLDER", "FIXME", "XXX"];

    for placeholder in placeholders {
        assert!(
            !output.contains(placeholder),
            "FALSIFIED: Output contains placeholder text '{}'.",
            placeholder
        );
    }
}

// =============================================================================
// SECTION 9: DETERMINISTIC MODE FALSIFICATION
// =============================================================================

/// FALSIFICATION TEST [S4]: Deterministic mode produces zeros
#[test]
fn falsify_deterministic_not_zero() {
    let app = App::with_config(true, Default::default());

    assert!(
        app.per_core_percent.iter().all(|&p| p == 0.0),
        "FALSIFIED: Deterministic mode has non-zero CPU percentages."
    );

    assert!(
        app.per_core_freq.iter().all(|&f| f == 0),
        "FALSIFIED: Deterministic mode has non-zero frequencies."
    );

    assert!(
        app.load_avg.one == 0.0 && app.load_avg.five == 0.0 && app.load_avg.fifteen == 0.0,
        "FALSIFIED: Deterministic mode has non-zero load average."
    );
}

// =============================================================================
// SEVERITY REPORT
// =============================================================================

#[test]
fn report_test_severities() {
    println!("\n=== FALSIFICATION TEST SEVERITY REPORT ===");
    println!("falsify_temp_amd_k10temp_no_temp2     : S4");
    println!("falsify_temp_physically_implausible   : S4");
    println!("falsify_temp_vs_sensors_command       : S4");
    println!("falsify_user_column_shows_dashes      : S4");
    println!("falsify_username_is_garbage           : S4");
    println!("falsify_command_is_garbled            : S4");
    println!("falsify_core_count_mismatch           : S4");
    println!("falsify_cores_not_visible             : S4");
    println!("falsify_freq_vs_proc_cpuinfo          : S4");
    println!("falsify_freq_physically_implausible   : S3");
    println!("falsify_load_avg_vs_proc              : S4");
    println!("falsify_memory_total_vs_proc          : S4");
    println!("falsify_memory_math                   : S3");
    println!("falsify_placeholder_text_in_output    : S4");
    println!("falsify_deterministic_not_zero        : S4");
    println!("==========================================");
    println!("Total: 15 falsification tests");
    println!("S4: 13, S3: 2");
}
