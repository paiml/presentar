//! Benchmark tests for widget operations.

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use presentar_core::{Constraints, Size, Widget};
use presentar_widgets::{Button, Text};

fn bench_button_creation(c: &mut Criterion) {
    c.bench_function("button_new", |b| {
        b.iter(|| Button::new(black_box("Click me")))
    });
}

fn bench_text_creation(c: &mut Criterion) {
    c.bench_function("text_new", |b| {
        b.iter(|| Text::new(black_box("Hello, World!")))
    });
}

fn bench_button_measure(c: &mut Criterion) {
    let button = Button::new("Click me");
    let constraints = Constraints::new(0.0, 200.0, 0.0, 50.0);

    c.bench_function("button_measure", |b| {
        b.iter(|| button.measure(black_box(constraints)))
    });
}

fn bench_text_measure(c: &mut Criterion) {
    let text = Text::new("Hello, World!");
    let constraints = Constraints::new(0.0, 200.0, 0.0, 50.0);

    c.bench_function("text_measure", |b| {
        b.iter(|| text.measure(black_box(constraints)))
    });
}

fn bench_constraints_constrain(c: &mut Criterion) {
    let constraints = Constraints::new(0.0, 200.0, 0.0, 100.0);
    let size = Size::new(150.0, 75.0);

    c.bench_function("constraints_constrain", |b| {
        b.iter(|| {
            let s = black_box(size);
            constraints.constrain(s)
        })
    });
}

criterion_group!(
    benches,
    bench_button_creation,
    bench_text_creation,
    bench_button_measure,
    bench_text_measure,
    bench_constraints_constrain,
);
criterion_main!(benches);
