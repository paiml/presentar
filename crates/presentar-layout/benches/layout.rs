//! Benchmark tests for layout engine operations.

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use presentar_core::widget::{AccessibleRole, LayoutResult};
use presentar_core::{Canvas, Constraints, Event, Rect, Size, TypeId, Widget};
use presentar_layout::LayoutEngine;
use std::any::Any;

/// Test widget for benchmarking
struct BenchWidget {
    size: Size,
    children: Vec<Box<dyn Widget>>,
}

impl BenchWidget {
    fn new(width: f32, height: f32) -> Self {
        Self {
            size: Size::new(width, height),
            children: Vec::new(),
        }
    }

    fn with_child(mut self, child: BenchWidget) -> Self {
        self.children.push(Box::new(child));
        self
    }

    fn with_n_children(mut self, n: usize, width: f32, height: f32) -> Self {
        for _ in 0..n {
            self.children
                .push(Box::new(BenchWidget::new(width, height)));
        }
        self
    }
}

impl Widget for BenchWidget {
    fn type_id(&self) -> TypeId {
        TypeId::of::<Self>()
    }

    fn measure(&self, constraints: Constraints) -> Size {
        constraints.constrain(self.size)
    }

    fn layout(&mut self, bounds: Rect) -> LayoutResult {
        LayoutResult {
            size: bounds.size(),
        }
    }

    fn paint(&self, _canvas: &mut dyn Canvas) {}

    fn event(&mut self, _event: &Event) -> Option<Box<dyn Any + Send>> {
        None
    }

    fn children(&self) -> &[Box<dyn Widget>] {
        &self.children
    }

    fn children_mut(&mut self) -> &mut [Box<dyn Widget>] {
        &mut self.children
    }

    fn accessible_role(&self) -> AccessibleRole {
        AccessibleRole::Generic
    }
}

fn bench_layout_single_widget(c: &mut Criterion) {
    let mut engine = LayoutEngine::new();
    let viewport = Size::new(800.0, 600.0);

    c.bench_function("layout_single_widget", |b| {
        b.iter(|| {
            let mut widget = BenchWidget::new(100.0, 50.0);
            engine.compute(black_box(&mut widget), black_box(viewport))
        })
    });
}

fn bench_layout_10_children(c: &mut Criterion) {
    let mut engine = LayoutEngine::new();
    let viewport = Size::new(800.0, 600.0);

    c.bench_function("layout_10_children", |b| {
        b.iter(|| {
            let mut widget = BenchWidget::new(400.0, 300.0).with_n_children(10, 50.0, 50.0);
            engine.compute(black_box(&mut widget), black_box(viewport))
        })
    });
}

fn bench_layout_100_children(c: &mut Criterion) {
    let mut engine = LayoutEngine::new();
    let viewport = Size::new(800.0, 600.0);

    c.bench_function("layout_100_children", |b| {
        b.iter(|| {
            let mut widget = BenchWidget::new(800.0, 600.0).with_n_children(100, 30.0, 30.0);
            engine.compute(black_box(&mut widget), black_box(viewport))
        })
    });
}

fn bench_layout_nested_3_levels(c: &mut Criterion) {
    let mut engine = LayoutEngine::new();
    let viewport = Size::new(800.0, 600.0);

    c.bench_function("layout_nested_3_levels", |b| {
        b.iter(|| {
            let mut widget = BenchWidget::new(400.0, 300.0)
                .with_child(
                    BenchWidget::new(200.0, 150.0).with_child(BenchWidget::new(100.0, 75.0)),
                )
                .with_child(
                    BenchWidget::new(200.0, 150.0).with_child(BenchWidget::new(100.0, 75.0)),
                );
            engine.compute(black_box(&mut widget), black_box(viewport))
        })
    });
}

fn bench_layout_readonly(c: &mut Criterion) {
    let mut engine = LayoutEngine::new();
    let viewport = Size::new(800.0, 600.0);

    c.bench_function("layout_readonly_10_children", |b| {
        b.iter(|| {
            let widget = BenchWidget::new(400.0, 300.0).with_n_children(10, 50.0, 50.0);
            engine.compute_readonly(black_box(&widget), black_box(viewport))
        })
    });
}

fn bench_constraints_operations(c: &mut Criterion) {
    let constraints = Constraints::new(0.0, 800.0, 0.0, 600.0);
    let size = Size::new(500.0, 400.0);

    c.bench_function("constraints_constrain", |b| {
        b.iter(|| constraints.constrain(black_box(size)))
    });

    c.bench_function("constraints_deflate", |b| {
        b.iter(|| constraints.deflate(black_box(20.0), black_box(20.0)))
    });

    c.bench_function("constraints_tight", |b| {
        b.iter(|| Constraints::tight(black_box(size)))
    });

    c.bench_function("constraints_loose", |b| {
        b.iter(|| Constraints::loose(black_box(size)))
    });
}

criterion_group!(
    benches,
    bench_layout_single_widget,
    bench_layout_10_children,
    bench_layout_100_children,
    bench_layout_nested_3_levels,
    bench_layout_readonly,
    bench_constraints_operations,
);
criterion_main!(benches);
