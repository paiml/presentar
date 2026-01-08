//! Brick Computer Demo - SIMD Visualization
//!
//! A polished TUI demonstration of Brick Architecture showing:
//! - SIMD lanes as visual "bricks" that light up during operations
//! - Real-time performance metrics (throughput, latency, utilization)
//! - Budget tracking with visual meters
//! - Inspired by trueno-viz and btop polish
//!
//! Run with: cargo run --example brick_computer -p presentar

use std::io::{self, Write};
use std::thread;
use std::time::{Duration, Instant};

// ============================================================================
// ANSI ESCAPE CODES (Polish: Color System)
// ============================================================================

const RESET: &str = "\x1b[0m";
const BOLD: &str = "\x1b[1m";
const DIM: &str = "\x1b[2m";

// Status colors (perceptually distinct)
const GREEN: &str = "\x1b[38;2;74;222;128m"; // #4ade80 - success
const YELLOW: &str = "\x1b[38;2;250;204;21m"; // #facc15 - warning
const RED: &str = "\x1b[38;2;248;113;113m"; // #f87171 - error
const CYAN: &str = "\x1b[38;2;34;211;238m"; // #22d3ee - info
const MAGENTA: &str = "\x1b[38;2;232;121;249m"; // #e879f9 - accent
const BLUE: &str = "\x1b[38;2;96;165;250m"; // #60a5fa - primary
const GRAY: &str = "\x1b[38;2;107;114;128m"; // #6b7280 - muted

// Background colors
const BG_GREEN: &str = "\x1b[48;2;22;163;74m"; // #16a34a
const BG_YELLOW: &str = "\x1b[48;2;202;138;4m"; // #ca8a04
const BG_RED: &str = "\x1b[48;2;220;38;38m"; // #dc2626
const BG_BLUE: &str = "\x1b[48;2;37;99;235m"; // #2563eb
const BG_DARK: &str = "\x1b[48;2;17;24;39m"; // #111827
const WHITE: &str = "\x1b[38;2;255;255;255m";

// ============================================================================
// UNICODE BOX DRAWING (Polish: Professional TUI)
// ============================================================================

const BOX_TL: &str = "╭";
const BOX_TR: &str = "╮";
const BOX_BL: &str = "╰";
const BOX_BR: &str = "╯";
const BOX_H: &str = "─";
const BOX_V: &str = "│";

// Block characters for meters (keeping full set for reference)
const BLOCK_FULL: &str = "█";
const BLOCK_EMPTY: &str = "░";
#[allow(dead_code)]
const BLOCK_CHARS: [&str; 8] = ["▏", "▎", "▍", "▌", "▋", "▊", "▉", "█"];

// Braille for sparklines
const BRAILLE_DOTS: [char; 8] = ['⣀', '⣄', '⣤', '⣦', '⣶', '⣷', '⣿', '⡿'];

// ============================================================================
// SIMD BRICK REPRESENTATION
// ============================================================================

#[derive(Clone, Copy, PartialEq)]
enum BrickState {
    Idle,
    Computing,
    Success,
    Warning,
    Error,
}

struct SimdLane {
    id: usize,
    state: BrickState,
    value: f32,
    latency_us: u32,
    operations: u64,
}

impl SimdLane {
    fn new(id: usize) -> Self {
        Self {
            id,
            state: BrickState::Idle,
            value: 0.0,
            latency_us: 0,
            operations: 0,
        }
    }

    fn compute(&mut self, input: f32) {
        self.state = BrickState::Computing;
        // Simulate SIMD operation
        self.value = input * 2.0 + (self.id as f32);
        self.latency_us = 50 + (self.id as u32 * 10);
        self.operations += 1;
        self.state = if self.latency_us < 100 {
            BrickState::Success
        } else if self.latency_us < 150 {
            BrickState::Warning
        } else {
            BrickState::Error
        };
    }
}

struct BrickComputer {
    lanes: Vec<SimdLane>,
    total_ops: u64,
    start_time: Instant,
    history: Vec<f64>,
    budget_ms: u32,
    used_ms: u32,
}

impl BrickComputer {
    fn new(lane_count: usize) -> Self {
        Self {
            lanes: (0..lane_count).map(SimdLane::new).collect(),
            total_ops: 0,
            start_time: Instant::now(),
            history: Vec::with_capacity(60),
            budget_ms: 16, // 60fps
            used_ms: 0,
        }
    }

    fn step(&mut self, inputs: &[f32]) {
        let step_start = Instant::now();

        for (lane, &input) in self.lanes.iter_mut().zip(inputs.iter()) {
            lane.compute(input);
        }

        self.total_ops += self.lanes.len() as u64;
        self.used_ms = step_start.elapsed().as_millis() as u32;

        // Track utilization history
        let utilization = (self.used_ms as f64 / self.budget_ms as f64).min(1.0);
        self.history.push(utilization);
        if self.history.len() > 60 {
            self.history.remove(0);
        }
    }

    fn throughput(&self) -> f64 {
        let elapsed = self.start_time.elapsed().as_secs_f64();
        if elapsed > 0.0 {
            self.total_ops as f64 / elapsed
        } else {
            0.0
        }
    }

    fn avg_latency_us(&self) -> u32 {
        if self.lanes.is_empty() {
            return 0;
        }
        self.lanes.iter().map(|l| l.latency_us).sum::<u32>() / self.lanes.len() as u32
    }
}

// ============================================================================
// RENDERING FUNCTIONS
// ============================================================================

fn clear_screen() {
    print!("\x1b[2J\x1b[H");
    io::stdout().flush().ok();
}

fn draw_box(title: &str, x: usize, y: usize, width: usize, height: usize, color: &str) {
    // Move to position
    print!("\x1b[{};{}H", y, x);

    // Top border
    print!("{}{}{}", color, BOX_TL, RESET);
    print!("{}{}", color, BOX_H.repeat(width - 2));
    print!("{}{}", color, BOX_TR);
    print!("{}", RESET);

    // Title
    if !title.is_empty() {
        print!("\x1b[{};{}H", y, x + 2);
        print!("{}{} {} {}", BOLD, color, title, RESET);
    }

    // Sides
    for row in 1..height - 1 {
        print!("\x1b[{};{}H", y + row, x);
        print!("{}{}{}", color, BOX_V, RESET);
        print!("\x1b[{};{}H", y + row, x + width - 1);
        print!("{}{}{}", color, BOX_V, RESET);
    }

    // Bottom border
    print!("\x1b[{};{}H", y + height - 1, x);
    print!("{}{}{}", color, BOX_BL, RESET);
    print!("{}{}", color, BOX_H.repeat(width - 2));
    print!("{}{}", color, BOX_BR);
    print!("{}", RESET);
}

fn draw_brick(lane: &SimdLane, x: usize, y: usize) {
    let (bg, fg, symbol) = match lane.state {
        BrickState::Idle => (BG_DARK, GRAY, "░░"),
        BrickState::Computing => (BG_BLUE, WHITE, "▓▓"),
        BrickState::Success => (BG_GREEN, WHITE, "██"),
        BrickState::Warning => (BG_YELLOW, WHITE, "▓▓"),
        BrickState::Error => (BG_RED, WHITE, "XX"),
    };

    // Draw 3x2 brick
    print!("\x1b[{};{}H", y, x);
    print!("{}{}{}{}{}", bg, fg, symbol, symbol, RESET);
    print!("\x1b[{};{}H", y + 1, x);
    print!("{}{} {:2} {}", bg, fg, lane.id, RESET);
    print!("\x1b[{};{}H", y + 2, x);
    print!("{}{}{}{}{}", bg, fg, symbol, symbol, RESET);
}

fn draw_meter(label: &str, value: f64, x: usize, y: usize, width: usize) {
    let bar_width = width - label.len() - 8;
    let filled = ((value * bar_width as f64) as usize).min(bar_width);

    let color = if value > 0.9 {
        RED
    } else if value > 0.7 {
        YELLOW
    } else {
        GREEN
    };

    print!("\x1b[{};{}H", y, x);
    print!("{}{:>6}{} ", BOLD, label, RESET);
    print!("{}", color);

    for i in 0..bar_width {
        if i < filled {
            print!("{}", BLOCK_FULL);
        } else {
            print!("{}{}", GRAY, BLOCK_EMPTY);
            print!("{}", color);
        }
    }

    print!("{} {:>5.1}%{}", RESET, value * 100.0, RESET);
}

fn draw_sparkline(data: &[f64], x: usize, y: usize, width: usize) {
    print!("\x1b[{};{}H", y, x);

    let step = data.len().max(1) / width.max(1);
    for i in 0..width {
        let idx = (i * step).min(data.len().saturating_sub(1));
        let val = data.get(idx).copied().unwrap_or(0.0);
        let level = ((val * 7.0) as usize).min(7);

        let color = if val > 0.9 {
            RED
        } else if val > 0.7 {
            YELLOW
        } else {
            GREEN
        };

        print!("{}{}{}", color, BRAILLE_DOTS[level], RESET);
    }
}

fn draw_stats(computer: &BrickComputer, x: usize, y: usize) {
    let success = computer
        .lanes
        .iter()
        .filter(|l| l.state == BrickState::Success)
        .count();
    let warning = computer
        .lanes
        .iter()
        .filter(|l| l.state == BrickState::Warning)
        .count();
    let error = computer
        .lanes
        .iter()
        .filter(|l| l.state == BrickState::Error)
        .count();

    print!("\x1b[{};{}H", y, x);
    print!(
        "{}Throughput:{} {:>12.0} ops/s",
        BOLD,
        RESET,
        computer.throughput()
    );

    print!("\x1b[{};{}H", y + 1, x);
    print!(
        "{}Avg Latency:{} {:>10}μs",
        BOLD,
        RESET,
        computer.avg_latency_us()
    );

    print!("\x1b[{};{}H", y + 2, x);
    print!("{}Total Ops:{} {:>13}", BOLD, RESET, computer.total_ops);

    print!("\x1b[{};{}H", y + 4, x);
    print!("{}Lanes: {}", BOLD, RESET);
    print!("{}{}{} ", GREEN, BLOCK_FULL, RESET);
    print!("{}{} ", success, RESET);
    print!("{}{}{} ", YELLOW, BLOCK_FULL, RESET);
    print!("{}{} ", warning, RESET);
    print!("{}{}{} ", RED, BLOCK_FULL, RESET);
    print!("{}", error);
}

fn draw_header() {
    print!("\x1b[1;1H");
    print!("{}{}  BRICK COMPUTER  {}", BOLD, MAGENTA, RESET);
    print!("{}  SIMD Visualization Demo  {}", DIM, RESET);
    print!("{}  presentar v0.2  {}", CYAN, RESET);
}

fn draw_legend(y: usize) {
    print!("\x1b[{};2H", y);
    print!("{}Legend:{} ", BOLD, RESET);
    print!("{}{}{} Idle  ", BG_DARK, GRAY, BLOCK_EMPTY.repeat(2));
    print!("{}", RESET);
    print!("{}{}{} Computing  ", BG_BLUE, WHITE, "▓▓");
    print!("{}", RESET);
    print!("{}{}{} Success  ", BG_GREEN, WHITE, BLOCK_FULL.repeat(2));
    print!("{}", RESET);
    print!("{}{}{} Warning  ", BG_YELLOW, WHITE, "▓▓");
    print!("{}", RESET);
    print!("{}{}{} Error", BG_RED, WHITE, "XX");
    print!("{}", RESET);
}

fn draw_footer(y: usize) {
    print!("\x1b[{};2H", y);
    print!("{}Press Ctrl+C to exit{}", DIM, RESET);
}

// ============================================================================
// MAIN
// ============================================================================

fn main() {
    // 8-wide SIMD (AVX-256 style)
    let lane_count = 8;
    let mut computer = BrickComputer::new(lane_count);

    // Random-ish inputs
    let inputs: Vec<f32> = (0..lane_count).map(|i| (i as f32) * 0.1 + 0.5).collect();

    clear_screen();

    // Hide cursor
    print!("\x1b[?25l");
    io::stdout().flush().ok();

    let frame_count = 100;

    for frame in 0..frame_count {
        clear_screen();

        // Header
        draw_header();

        // SIMD Lanes panel
        draw_box("SIMD LANES (AVX-256)", 2, 3, 50, 8, CYAN);

        // Draw bricks
        for (i, lane) in computer.lanes.iter().enumerate() {
            let bx = 4 + (i * 6);
            draw_brick(lane, bx, 5);
        }

        // Stats panel
        draw_box("PERFORMANCE", 54, 3, 28, 8, MAGENTA);
        draw_stats(&computer, 56, 5);

        // Budget panel
        draw_box("BUDGET UTILIZATION", 2, 12, 80, 6, BLUE);
        let utilization = computer.used_ms as f64 / computer.budget_ms as f64;
        draw_meter("Frame", utilization, 4, 14, 40);

        print!("\x1b[14;50H");
        print!(
            "{}Budget:{} {}ms  {}Used:{} {}ms",
            BOLD, RESET, computer.budget_ms, BOLD, RESET, computer.used_ms
        );

        // Sparkline history
        print!("\x1b[16;4H");
        print!("{}History: {}", DIM, RESET);
        draw_sparkline(&computer.history, 14, 16, 60);

        // Brick verification panel
        draw_box("BRICK VERIFICATION", 2, 19, 80, 5, GREEN);

        let all_pass = computer
            .lanes
            .iter()
            .all(|l| l.state == BrickState::Success);
        print!("\x1b[21;4H");
        if all_pass {
            print!(
                "{}{}ALL BRICKS LIT{} - JIDOKA: Render allowed",
                BOLD, GREEN, RESET
            );
        } else {
            print!(
                "{}{}SOME BRICKS DARK{} - JIDOKA: Investigating...",
                BOLD, YELLOW, RESET
            );
        }

        print!("\x1b[22;4H");
        print!(
            "{}Assertions:{} TextVisible ✓  ContrastRatio ✓  MaxLatency ",
            DIM, RESET
        );
        if computer.avg_latency_us() < 100 {
            print!("{}✓{}", GREEN, RESET);
        } else {
            print!("{}✗{}", RED, RESET);
        }

        // Legend and footer
        draw_legend(25);
        draw_footer(27);

        // Frame counter
        print!("\x1b[27;70H");
        print!("{}Frame {}/{}{}", DIM, frame + 1, frame_count, RESET);

        io::stdout().flush().ok();

        // Step the simulation
        computer.step(&inputs);

        // Animate
        thread::sleep(Duration::from_millis(100));
    }

    // Show cursor
    print!("\x1b[?25h");

    // Final summary
    println!("\n\n{}{}=== SIMULATION COMPLETE ==={}", BOLD, CYAN, RESET);
    println!();
    println!("{}Total Operations:{} {}", BOLD, RESET, computer.total_ops);
    println!(
        "{}Final Throughput:{} {:.0} ops/s",
        BOLD,
        RESET,
        computer.throughput()
    );
    println!(
        "{}Average Latency:{} {}μs",
        BOLD,
        RESET,
        computer.avg_latency_us()
    );
    println!();

    let success_rate = computer
        .lanes
        .iter()
        .filter(|l| l.state == BrickState::Success)
        .count() as f64
        / computer.lanes.len() as f64
        * 100.0;

    println!("{}Brick Success Rate:{} {:.1}%", BOLD, RESET, success_rate);
    println!();
}
