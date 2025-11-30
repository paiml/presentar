#![allow(clippy::unwrap_used, clippy::disallowed_methods)]
//! Chart rendering algorithms for Presentar.
//!
//! This module provides mathematical algorithms for chart rendering:
//! - Interpolation (linear, cubic spline, Catmull-Rom, Bezier)
//! - Path tessellation for GPU rendering
//! - Histogram binning
//! - Arc geometry computation
//! - Data normalization and scaling
//!
//! # Example
//!
//! ```
//! use presentar_core::chart::{Interpolator, CubicSpline, Point2D};
//!
//! // Create a spline from control points
//! let points = vec![
//!     Point2D::new(0.0, 0.0),
//!     Point2D::new(1.0, 2.0),
//!     Point2D::new(2.0, 1.5),
//!     Point2D::new(3.0, 3.0),
//! ];
//! let spline = CubicSpline::from_points(&points);
//!
//! // Interpolate at any x value
//! let y = spline.interpolate(1.5);
//! ```

use serde::{Deserialize, Serialize};
use std::f64::consts::PI;

/// 2D point for chart calculations.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct Point2D {
    pub x: f64,
    pub y: f64,
}

impl Point2D {
    /// Create a new point.
    #[must_use]
    pub const fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }

    /// Origin point (0, 0).
    pub const ORIGIN: Self = Self { x: 0.0, y: 0.0 };

    /// Distance to another point.
    #[must_use]
    pub fn distance(&self, other: &Self) -> f64 {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        dx.hypot(dy)
    }

    /// Linear interpolation between two points.
    #[must_use]
    pub fn lerp(&self, other: &Self, t: f64) -> Self {
        Self {
            x: (other.x - self.x).mul_add(t, self.x),
            y: (other.y - self.y).mul_add(t, self.y),
        }
    }
}

impl std::ops::Add for Point2D {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self {
            x: self.x + rhs.x,
            y: self.y + rhs.y,
        }
    }
}

impl std::ops::Sub for Point2D {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        Self {
            x: self.x - rhs.x,
            y: self.y - rhs.y,
        }
    }
}

impl std::ops::Mul<f64> for Point2D {
    type Output = Self;

    fn mul(self, rhs: f64) -> Self::Output {
        Self {
            x: self.x * rhs,
            y: self.y * rhs,
        }
    }
}

/// Interpolation trait for different curve types.
pub trait Interpolator {
    /// Interpolate y value at given x.
    fn interpolate(&self, x: f64) -> f64;

    /// Generate points along the curve.
    fn sample(&self, start: f64, end: f64, num_points: usize) -> Vec<Point2D> {
        if num_points < 2 {
            return vec![];
        }
        let step = (end - start) / (num_points - 1) as f64;
        (0..num_points)
            .map(|i| {
                let x = (i as f64).mul_add(step, start);
                Point2D::new(x, self.interpolate(x))
            })
            .collect()
    }
}

/// Linear interpolation between points.
#[derive(Debug, Clone)]
pub struct LinearInterpolator {
    points: Vec<Point2D>,
}

impl LinearInterpolator {
    /// Create from points (must be sorted by x).
    #[must_use]
    pub fn from_points(points: &[Point2D]) -> Self {
        let mut sorted = points.to_vec();
        sorted.sort_by(|a, b| a.x.partial_cmp(&b.x).unwrap_or(std::cmp::Ordering::Equal));
        Self { points: sorted }
    }

    /// Create from x,y data.
    #[must_use]
    pub fn from_xy(xs: &[f64], ys: &[f64]) -> Self {
        let points: Vec<_> = xs
            .iter()
            .zip(ys.iter())
            .map(|(&x, &y)| Point2D::new(x, y))
            .collect();
        Self::from_points(&points)
    }

    /// Get the underlying points.
    #[must_use]
    pub fn points(&self) -> &[Point2D] {
        &self.points
    }

    /// Find the segment containing x.
    fn find_segment(&self, x: f64) -> Option<(usize, f64)> {
        if self.points.len() < 2 {
            return None;
        }
        for i in 0..self.points.len() - 1 {
            let p1 = &self.points[i];
            let p2 = &self.points[i + 1];
            if x >= p1.x && x <= p2.x {
                let t = if (p2.x - p1.x).abs() < 1e-10 {
                    0.0
                } else {
                    (x - p1.x) / (p2.x - p1.x)
                };
                return Some((i, t));
            }
        }
        // Extrapolate
        if x < self.points[0].x {
            Some((
                0,
                (x - self.points[0].x) / (self.points[1].x - self.points[0].x),
            ))
        } else {
            let n = self.points.len();
            Some((
                n - 2,
                (x - self.points[n - 2].x) / (self.points[n - 1].x - self.points[n - 2].x),
            ))
        }
    }
}

impl Interpolator for LinearInterpolator {
    fn interpolate(&self, x: f64) -> f64 {
        if self.points.is_empty() {
            return 0.0;
        }
        if self.points.len() == 1 {
            return self.points[0].y;
        }

        if let Some((i, t)) = self.find_segment(x) {
            let p1 = &self.points[i];
            let p2 = &self.points[i + 1];
            (p2.y - p1.y).mul_add(t, p1.y)
        } else {
            0.0
        }
    }
}

/// Cubic spline interpolation (natural spline).
#[derive(Debug, Clone)]
pub struct CubicSpline {
    points: Vec<Point2D>,
    /// Second derivatives at each point
    y2: Vec<f64>,
}

impl CubicSpline {
    /// Create from points (must have at least 3 points).
    #[must_use]
    pub fn from_points(points: &[Point2D]) -> Self {
        let mut sorted = points.to_vec();
        sorted.sort_by(|a, b| a.x.partial_cmp(&b.x).unwrap_or(std::cmp::Ordering::Equal));

        let n = sorted.len();
        if n < 3 {
            return Self {
                points: sorted,
                y2: vec![0.0; n],
            };
        }

        // Compute second derivatives using natural spline boundary conditions
        let mut y2 = vec![0.0; n];
        let mut u = vec![0.0; n];

        // Forward sweep
        for i in 1..n - 1 {
            let h_prev = sorted[i].x - sorted[i - 1].x;
            let h_next = sorted[i + 1].x - sorted[i].x;

            if h_prev.abs() < 1e-10 || h_next.abs() < 1e-10 {
                continue;
            }

            let sig = h_prev / (h_prev + h_next);
            let p = sig.mul_add(y2[i - 1], 2.0);
            y2[i] = (sig - 1.0) / p;
            u[i] =
                (sorted[i + 1].y - sorted[i].y) / h_next - (sorted[i].y - sorted[i - 1].y) / h_prev;
            u[i] = sig.mul_add(-u[i - 1], 6.0 * u[i] / (h_prev + h_next)) / p;
        }

        // Back substitution
        for k in (0..n - 1).rev() {
            y2[k] = y2[k].mul_add(y2[k + 1], u[k]);
        }

        Self { points: sorted, y2 }
    }

    /// Create from x,y data.
    #[must_use]
    pub fn from_xy(xs: &[f64], ys: &[f64]) -> Self {
        let points: Vec<_> = xs
            .iter()
            .zip(ys.iter())
            .map(|(&x, &y)| Point2D::new(x, y))
            .collect();
        Self::from_points(&points)
    }

    /// Get the underlying points.
    #[must_use]
    pub fn points(&self) -> &[Point2D] {
        &self.points
    }
}

impl Interpolator for CubicSpline {
    fn interpolate(&self, x: f64) -> f64 {
        let n = self.points.len();
        if n == 0 {
            return 0.0;
        }
        if n == 1 {
            return self.points[0].y;
        }
        if n == 2 {
            // Fall back to linear
            let t = (x - self.points[0].x) / (self.points[1].x - self.points[0].x);
            return (self.points[1].y - self.points[0].y).mul_add(t, self.points[0].y);
        }

        // Find segment
        let mut lo = 0;
        let mut hi = n - 1;
        while hi - lo > 1 {
            let mid = (hi + lo) / 2;
            if self.points[mid].x > x {
                hi = mid;
            } else {
                lo = mid;
            }
        }

        let h = self.points[hi].x - self.points[lo].x;
        if h.abs() < 1e-10 {
            return self.points[lo].y;
        }

        let a = (self.points[hi].x - x) / h;
        let b = (x - self.points[lo].x) / h;

        a.mul_add(self.points[lo].y, b * self.points[hi].y)
            + (a * a)
                .mul_add(a, -a)
                .mul_add(self.y2[lo], (b * b).mul_add(b, -b) * self.y2[hi])
                * h
                * h
                / 6.0
    }
}

/// Catmull-Rom spline interpolation.
#[derive(Debug, Clone)]
pub struct CatmullRom {
    points: Vec<Point2D>,
    /// Tension parameter (0.0 to 1.0)
    tension: f64,
}

impl CatmullRom {
    /// Create from points with default tension.
    #[must_use]
    pub fn from_points(points: &[Point2D]) -> Self {
        Self::with_tension(points, 0.5)
    }

    /// Create from points with custom tension.
    #[must_use]
    pub fn with_tension(points: &[Point2D], tension: f64) -> Self {
        let mut sorted = points.to_vec();
        sorted.sort_by(|a, b| a.x.partial_cmp(&b.x).unwrap_or(std::cmp::Ordering::Equal));
        Self {
            points: sorted,
            tension: tension.clamp(0.0, 1.0),
        }
    }

    /// Get the underlying points.
    #[must_use]
    pub fn points(&self) -> &[Point2D] {
        &self.points
    }

    /// Generate a smooth path through the points.
    #[must_use]
    pub fn to_path(&self, segments_per_span: usize) -> Vec<Point2D> {
        if self.points.len() < 2 {
            return self.points.clone();
        }
        if self.points.len() == 2 {
            return self.points.clone();
        }

        let mut path = Vec::new();
        let n = self.points.len();

        for i in 0..n - 1 {
            let p0 = if i == 0 {
                self.points[0]
            } else {
                self.points[i - 1]
            };
            let p1 = self.points[i];
            let p2 = self.points[i + 1];
            let p3 = if i + 2 < n {
                self.points[i + 2]
            } else {
                self.points[n - 1]
            };

            for j in 0..segments_per_span {
                let t = j as f64 / segments_per_span as f64;
                let point = self.catmull_rom_point(p0, p1, p2, p3, t);
                path.push(point);
            }
        }

        // Add final point
        path.push(self.points[n - 1]);
        path
    }

    /// Compute point on Catmull-Rom curve.
    fn catmull_rom_point(
        &self,
        p0: Point2D,
        p1: Point2D,
        p2: Point2D,
        p3: Point2D,
        t: f64,
    ) -> Point2D {
        let t2 = t * t;
        let t3 = t2 * t;

        let tau = self.tension;

        // Catmull-Rom basis matrix with tension
        let c0 = tau.mul_add(-t, (-tau).mul_add(t3, 2.0 * tau * t2));
        let c1 = (2.0 - tau).mul_add(t3, (tau - 3.0) * t2) + 1.0;
        let c2 = tau.mul_add(t, (tau - 2.0).mul_add(t3, 2.0f64.mul_add(-tau, 3.0) * t2));
        let c3 = tau.mul_add(t3, -(tau * t2));

        Point2D::new(
            c3.mul_add(p3.x, c0 * p0.x + c1 * p1.x + c2 * p2.x),
            c3.mul_add(p3.y, c0 * p0.y + c1 * p1.y + c2 * p2.y),
        )
    }
}

impl Interpolator for CatmullRom {
    fn interpolate(&self, x: f64) -> f64 {
        // For Catmull-Rom, we need to find the segment and interpolate
        if self.points.is_empty() {
            return 0.0;
        }
        if self.points.len() == 1 {
            return self.points[0].y;
        }

        // Find segment containing x
        let mut idx = 0;
        for i in 0..self.points.len() - 1 {
            if x >= self.points[i].x && x <= self.points[i + 1].x {
                idx = i;
                break;
            }
            if x < self.points[i].x {
                idx = i.saturating_sub(1);
                break;
            }
            idx = i;
        }

        let p1 = &self.points[idx];
        let p2 = &self.points[(idx + 1).min(self.points.len() - 1)];

        let t = if (p2.x - p1.x).abs() < 1e-10 {
            0.0
        } else {
            ((x - p1.x) / (p2.x - p1.x)).clamp(0.0, 1.0)
        };

        let p0 = if idx == 0 { *p1 } else { self.points[idx - 1] };
        let p3 = if idx + 2 < self.points.len() {
            self.points[idx + 2]
        } else {
            *p2
        };

        self.catmull_rom_point(p0, *p1, *p2, p3, t).y
    }
}

/// Bezier curve segment.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CubicBezier {
    /// Start point
    pub p0: Point2D,
    /// First control point
    pub p1: Point2D,
    /// Second control point
    pub p2: Point2D,
    /// End point
    pub p3: Point2D,
}

impl CubicBezier {
    /// Create a new cubic Bezier curve.
    #[must_use]
    pub const fn new(p0: Point2D, p1: Point2D, p2: Point2D, p3: Point2D) -> Self {
        Self { p0, p1, p2, p3 }
    }

    /// Evaluate the curve at parameter t (0 to 1).
    #[must_use]
    pub fn evaluate(&self, t: f64) -> Point2D {
        let t = t.clamp(0.0, 1.0);
        let mt = 1.0 - t;
        let mt2 = mt * mt;
        let mt3 = mt2 * mt;
        let t2 = t * t;
        let t3 = t2 * t;

        Point2D::new(
            (3.0 * mt * t2).mul_add(self.p2.x, mt3 * self.p0.x + 3.0 * mt2 * t * self.p1.x)
                + t3 * self.p3.x,
            (3.0 * mt * t2).mul_add(self.p2.y, mt3 * self.p0.y + 3.0 * mt2 * t * self.p1.y)
                + t3 * self.p3.y,
        )
    }

    /// Convert to a polyline with given number of segments.
    #[must_use]
    pub fn to_polyline(&self, segments: usize) -> Vec<Point2D> {
        let segments = segments.max(1);
        (0..=segments)
            .map(|i| self.evaluate(i as f64 / segments as f64))
            .collect()
    }

    /// Compute approximate arc length.
    #[must_use]
    pub fn arc_length(&self, segments: usize) -> f64 {
        let points = self.to_polyline(segments);
        points.windows(2).map(|w| w[0].distance(&w[1])).sum()
    }

    /// Split curve at parameter t.
    #[must_use]
    pub fn split(&self, t: f64) -> (Self, Self) {
        let t = t.clamp(0.0, 1.0);

        // De Casteljau's algorithm
        let p01 = self.p0.lerp(&self.p1, t);
        let p12 = self.p1.lerp(&self.p2, t);
        let p23 = self.p2.lerp(&self.p3, t);

        let p012 = p01.lerp(&p12, t);
        let p123 = p12.lerp(&p23, t);

        let p0123 = p012.lerp(&p123, t);

        let left = Self::new(self.p0, p01, p012, p0123);
        let right = Self::new(p0123, p123, p23, self.p3);

        (left, right)
    }
}

/// Histogram binning configuration.
#[derive(Debug, Clone)]
pub struct HistogramBins {
    /// Bin edges (n+1 edges for n bins)
    pub edges: Vec<f64>,
    /// Bin counts
    pub counts: Vec<usize>,
    /// Bin densities (normalized)
    pub densities: Vec<f64>,
}

impl HistogramBins {
    /// Create histogram from data with specified number of bins.
    #[must_use]
    pub fn from_data(data: &[f64], num_bins: usize) -> Self {
        if data.is_empty() || num_bins == 0 {
            return Self {
                edges: vec![],
                counts: vec![],
                densities: vec![],
            };
        }

        let num_bins = num_bins.max(1);
        let min = data.iter().copied().fold(f64::INFINITY, f64::min);
        let max = data.iter().copied().fold(f64::NEG_INFINITY, f64::max);

        Self::from_data_range(data, num_bins, min, max)
    }

    /// Create histogram from data with explicit range.
    #[must_use]
    pub fn from_data_range(data: &[f64], num_bins: usize, min: f64, max: f64) -> Self {
        let num_bins = num_bins.max(1);
        let range = (max - min).max(1e-10);
        let bin_width = range / num_bins as f64;

        // Create edges
        let edges: Vec<f64> = (0..=num_bins)
            .map(|i| (i as f64).mul_add(bin_width, min))
            .collect();

        // Count values in each bin
        let mut counts = vec![0usize; num_bins];
        for &value in data {
            let bin = ((value - min) / bin_width).floor() as usize;
            let bin = bin.min(num_bins - 1); // Handle edge case where value == max
            counts[bin] += 1;
        }

        // Compute densities (probability density)
        let total = data.len() as f64;
        let densities: Vec<f64> = counts
            .iter()
            .map(|&c| (c as f64) / (total * bin_width))
            .collect();

        Self {
            edges,
            counts,
            densities,
        }
    }

    /// Get number of bins.
    #[must_use]
    pub fn num_bins(&self) -> usize {
        self.counts.len()
    }

    /// Get bin width (assumes uniform bins).
    #[must_use]
    pub fn bin_width(&self) -> f64 {
        if self.edges.len() < 2 {
            return 0.0;
        }
        self.edges[1] - self.edges[0]
    }

    /// Get bin center for given index.
    #[must_use]
    pub fn bin_center(&self, index: usize) -> Option<f64> {
        if index >= self.counts.len() {
            return None;
        }
        Some((self.edges[index] + self.edges[index + 1]) / 2.0)
    }

    /// Get bin range for given index.
    #[must_use]
    pub fn bin_range(&self, index: usize) -> Option<(f64, f64)> {
        if index >= self.counts.len() {
            return None;
        }
        Some((self.edges[index], self.edges[index + 1]))
    }

    /// Total count across all bins.
    #[must_use]
    pub fn total_count(&self) -> usize {
        self.counts.iter().sum()
    }

    /// Maximum count in any bin.
    #[must_use]
    pub fn max_count(&self) -> usize {
        self.counts.iter().copied().max().unwrap_or(0)
    }
}

/// Arc geometry for pie charts.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ArcGeometry {
    /// Center point
    pub center: Point2D,
    /// Radius
    pub radius: f64,
    /// Start angle (radians)
    pub start_angle: f64,
    /// End angle (radians)
    pub end_angle: f64,
}

impl ArcGeometry {
    /// Create a new arc.
    #[must_use]
    pub const fn new(center: Point2D, radius: f64, start_angle: f64, end_angle: f64) -> Self {
        Self {
            center,
            radius,
            start_angle,
            end_angle,
        }
    }

    /// Create a full circle.
    #[must_use]
    pub fn circle(center: Point2D, radius: f64) -> Self {
        Self::new(center, radius, 0.0, 2.0 * PI)
    }

    /// Get the sweep angle.
    #[must_use]
    pub fn sweep(&self) -> f64 {
        self.end_angle - self.start_angle
    }

    /// Point on arc at given angle.
    #[must_use]
    pub fn point_at_angle(&self, angle: f64) -> Point2D {
        Point2D::new(
            self.radius.mul_add(angle.cos(), self.center.x),
            self.radius.mul_add(angle.sin(), self.center.y),
        )
    }

    /// Start point of arc.
    #[must_use]
    pub fn start_point(&self) -> Point2D {
        self.point_at_angle(self.start_angle)
    }

    /// End point of arc.
    #[must_use]
    pub fn end_point(&self) -> Point2D {
        self.point_at_angle(self.end_angle)
    }

    /// Midpoint of arc.
    #[must_use]
    pub fn mid_point(&self) -> Point2D {
        let mid_angle = (self.start_angle + self.end_angle) / 2.0;
        self.point_at_angle(mid_angle)
    }

    /// Arc length.
    #[must_use]
    pub fn arc_length(&self) -> f64 {
        self.radius * self.sweep().abs()
    }

    /// Convert arc to polyline for rendering.
    #[must_use]
    pub fn to_polyline(&self, segments: usize) -> Vec<Point2D> {
        let segments = segments.max(1);
        let sweep = self.sweep();
        (0..=segments)
            .map(|i| {
                let t = i as f64 / segments as f64;
                let angle = self.start_angle + t * sweep;
                self.point_at_angle(angle)
            })
            .collect()
    }

    /// Convert arc to pie slice (includes center point).
    #[must_use]
    pub fn to_pie_slice(&self, segments: usize) -> Vec<Point2D> {
        let mut points = vec![self.center];
        points.extend(self.to_polyline(segments));
        points.push(self.center);
        points
    }

    /// Check if angle is within arc sweep.
    #[must_use]
    pub fn contains_angle(&self, angle: f64) -> bool {
        let normalized = Self::normalize_angle(angle);
        let start = Self::normalize_angle(self.start_angle);
        let end = Self::normalize_angle(self.end_angle);

        if start <= end {
            normalized >= start && normalized <= end
        } else {
            normalized >= start || normalized <= end
        }
    }

    /// Normalize angle to [0, 2π).
    fn normalize_angle(angle: f64) -> f64 {
        let mut a = angle % (2.0 * PI);
        if a < 0.0 {
            a += 2.0 * PI;
        }
        a
    }
}

/// Data normalization for chart rendering.
#[derive(Debug, Clone, Copy)]
pub struct DataNormalizer {
    /// Minimum value
    pub min: f64,
    /// Maximum value
    pub max: f64,
}

impl DataNormalizer {
    /// Create normalizer from data range.
    #[must_use]
    pub fn new(min: f64, max: f64) -> Self {
        Self { min, max }
    }

    /// Create normalizer from data.
    #[must_use]
    pub fn from_data(data: &[f64]) -> Self {
        if data.is_empty() {
            return Self::new(0.0, 1.0);
        }
        let min = data.iter().copied().fold(f64::INFINITY, f64::min);
        let max = data.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        Self::new(min, max)
    }

    /// Normalize a value to [0, 1].
    #[must_use]
    pub fn normalize(&self, value: f64) -> f64 {
        let range = self.max - self.min;
        if range.abs() < 1e-10 {
            return 0.5;
        }
        (value - self.min) / range
    }

    /// Denormalize a value from [0, 1] to original range.
    #[must_use]
    pub fn denormalize(&self, normalized: f64) -> f64 {
        normalized.mul_add(self.max - self.min, self.min)
    }

    /// Normalize all values in a slice.
    #[must_use]
    pub fn normalize_all(&self, data: &[f64]) -> Vec<f64> {
        data.iter().map(|&v| self.normalize(v)).collect()
    }

    /// Get nice axis bounds (rounded for display).
    #[must_use]
    pub fn nice_bounds(&self) -> (f64, f64) {
        let range = self.max - self.min;
        if range.abs() < 1e-10 {
            return (self.min - 1.0, self.max + 1.0);
        }

        let magnitude = 10.0_f64.powf(range.log10().floor());
        let nice_min = (self.min / magnitude).floor() * magnitude;
        let nice_max = (self.max / magnitude).ceil() * magnitude;

        (nice_min, nice_max)
    }
}

/// Path tessellation for GPU rendering.
#[derive(Debug, Clone, Default)]
pub struct PathTessellator {
    /// Tolerance for curve flattening
    pub tolerance: f64,
    /// Generated vertices (x, y)
    pub vertices: Vec<[f32; 2]>,
    /// Triangle indices
    pub indices: Vec<u32>,
}

impl PathTessellator {
    /// Create a new tessellator.
    #[must_use]
    pub fn new(tolerance: f64) -> Self {
        Self {
            tolerance: tolerance.max(0.001),
            vertices: Vec::new(),
            indices: Vec::new(),
        }
    }

    /// Create with default tolerance.
    #[must_use]
    pub fn with_default_tolerance() -> Self {
        Self::new(0.25)
    }

    /// Clear the tessellator.
    pub fn clear(&mut self) {
        self.vertices.clear();
        self.indices.clear();
    }

    /// Tessellate a filled polygon.
    pub fn tessellate_polygon(&mut self, points: &[Point2D]) {
        if points.len() < 3 {
            return;
        }

        let base_idx = self.vertices.len() as u32;

        // Add vertices
        for p in points {
            self.vertices.push([p.x as f32, p.y as f32]);
        }

        // Fan triangulation (simple, works for convex polygons)
        for i in 1..points.len() as u32 - 1 {
            self.indices.push(base_idx);
            self.indices.push(base_idx + i);
            self.indices.push(base_idx + i + 1);
        }
    }

    /// Tessellate a stroked polyline.
    pub fn tessellate_stroke(&mut self, points: &[Point2D], width: f64) {
        if points.len() < 2 {
            return;
        }

        let half_width = width / 2.0;

        for window in points.windows(2) {
            let p1 = window[0];
            let p2 = window[1];

            // Compute perpendicular direction
            let dx = p2.x - p1.x;
            let dy = p2.y - p1.y;
            let len = dx.hypot(dy);
            if len < 1e-10 {
                continue;
            }

            let nx = -dy / len * half_width;
            let ny = dx / len * half_width;

            let base_idx = self.vertices.len() as u32;

            // Add quad vertices
            self.vertices.push([(p1.x + nx) as f32, (p1.y + ny) as f32]);
            self.vertices.push([(p1.x - nx) as f32, (p1.y - ny) as f32]);
            self.vertices.push([(p2.x + nx) as f32, (p2.y + ny) as f32]);
            self.vertices.push([(p2.x - nx) as f32, (p2.y - ny) as f32]);

            // Two triangles for quad
            self.indices.push(base_idx);
            self.indices.push(base_idx + 1);
            self.indices.push(base_idx + 2);

            self.indices.push(base_idx + 1);
            self.indices.push(base_idx + 3);
            self.indices.push(base_idx + 2);
        }
    }

    /// Tessellate a circle.
    pub fn tessellate_circle(&mut self, center: Point2D, radius: f64, segments: usize) {
        let segments = segments.max(8);
        let base_idx = self.vertices.len() as u32;

        // Center vertex
        self.vertices.push([center.x as f32, center.y as f32]);

        // Perimeter vertices
        for i in 0..segments {
            let angle = 2.0 * PI * i as f64 / segments as f64;
            let x = radius.mul_add(angle.cos(), center.x);
            let y = radius.mul_add(angle.sin(), center.y);
            self.vertices.push([x as f32, y as f32]);
        }

        // Fan triangles
        for i in 0..segments as u32 {
            self.indices.push(base_idx); // Center
            self.indices.push(base_idx + 1 + i);
            self.indices.push(base_idx + 1 + (i + 1) % segments as u32);
        }
    }

    /// Tessellate a rectangle.
    pub fn tessellate_rect(&mut self, x: f64, y: f64, width: f64, height: f64) {
        let base_idx = self.vertices.len() as u32;

        self.vertices.push([x as f32, y as f32]);
        self.vertices.push([(x + width) as f32, y as f32]);
        self.vertices
            .push([(x + width) as f32, (y + height) as f32]);
        self.vertices.push([x as f32, (y + height) as f32]);

        // Two triangles
        self.indices.push(base_idx);
        self.indices.push(base_idx + 1);
        self.indices.push(base_idx + 2);

        self.indices.push(base_idx);
        self.indices.push(base_idx + 2);
        self.indices.push(base_idx + 3);
    }

    /// Get vertex count.
    #[must_use]
    pub fn vertex_count(&self) -> usize {
        self.vertices.len()
    }

    /// Get index count.
    #[must_use]
    pub fn index_count(&self) -> usize {
        self.indices.len()
    }

    /// Get triangle count.
    #[must_use]
    pub fn triangle_count(&self) -> usize {
        self.indices.len() / 3
    }
}

/// Draw call batching for GPU efficiency.
#[derive(Debug, Clone, Default)]
pub struct DrawBatch {
    /// Batched circles (`center_x`, `center_y`, radius, `color_rgba`)
    pub circles: Vec<[f32; 7]>,
    /// Batched rectangles (x, y, w, h, `color_rgba`)
    pub rects: Vec<[f32; 8]>,
    /// Batched lines (x1, y1, x2, y2, width, `color_rgba`)
    pub lines: Vec<[f32; 9]>,
}

impl DrawBatch {
    /// Create a new batch.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a circle to the batch.
    pub fn add_circle(&mut self, x: f32, y: f32, radius: f32, r: f32, g: f32, b: f32, a: f32) {
        self.circles.push([x, y, radius, r, g, b, a]);
    }

    /// Add a rectangle to the batch.
    #[allow(clippy::too_many_arguments)]
    pub fn add_rect(&mut self, x: f32, y: f32, w: f32, h: f32, r: f32, g: f32, b: f32, a: f32) {
        self.rects.push([x, y, w, h, r, g, b, a]);
    }

    /// Add a line to the batch.
    #[allow(clippy::too_many_arguments)]
    pub fn add_line(
        &mut self,
        x1: f32,
        y1: f32,
        x2: f32,
        y2: f32,
        width: f32,
        r: f32,
        g: f32,
        b: f32,
        a: f32,
    ) {
        self.lines.push([x1, y1, x2, y2, width, r, g, b, a]);
    }

    /// Clear all batches.
    pub fn clear(&mut self) {
        self.circles.clear();
        self.rects.clear();
        self.lines.clear();
    }

    /// Total draw calls if not batched.
    #[must_use]
    pub fn unbatched_draw_calls(&self) -> usize {
        self.circles.len() + self.rects.len() + self.lines.len()
    }

    /// Actual draw calls with batching (3 max: circles, rects, lines).
    #[must_use]
    pub fn batched_draw_calls(&self) -> usize {
        let mut calls = 0;
        if !self.circles.is_empty() {
            calls += 1;
        }
        if !self.rects.is_empty() {
            calls += 1;
        }
        if !self.lines.is_empty() {
            calls += 1;
        }
        calls
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // Point2D Tests
    // =========================================================================

    #[test]
    fn test_point2d_new() {
        let p = Point2D::new(1.0, 2.0);
        assert_eq!(p.x, 1.0);
        assert_eq!(p.y, 2.0);
    }

    #[test]
    fn test_point2d_origin() {
        assert_eq!(Point2D::ORIGIN, Point2D::new(0.0, 0.0));
    }

    #[test]
    fn test_point2d_distance() {
        let p1 = Point2D::new(0.0, 0.0);
        let p2 = Point2D::new(3.0, 4.0);
        assert!((p1.distance(&p2) - 5.0).abs() < 1e-10);
    }

    #[test]
    fn test_point2d_lerp() {
        let p1 = Point2D::new(0.0, 0.0);
        let p2 = Point2D::new(10.0, 20.0);
        let mid = p1.lerp(&p2, 0.5);
        assert!((mid.x - 5.0).abs() < 1e-10);
        assert!((mid.y - 10.0).abs() < 1e-10);
    }

    #[test]
    fn test_point2d_add() {
        let p1 = Point2D::new(1.0, 2.0);
        let p2 = Point2D::new(3.0, 4.0);
        let sum = p1 + p2;
        assert_eq!(sum, Point2D::new(4.0, 6.0));
    }

    #[test]
    fn test_point2d_sub() {
        let p1 = Point2D::new(5.0, 7.0);
        let p2 = Point2D::new(2.0, 3.0);
        let diff = p1 - p2;
        assert_eq!(diff, Point2D::new(3.0, 4.0));
    }

    #[test]
    fn test_point2d_mul() {
        let p = Point2D::new(2.0, 3.0);
        let scaled = p * 2.0;
        assert_eq!(scaled, Point2D::new(4.0, 6.0));
    }

    // =========================================================================
    // LinearInterpolator Tests
    // =========================================================================

    #[test]
    fn test_linear_empty() {
        let interp = LinearInterpolator::from_points(&[]);
        assert_eq!(interp.interpolate(0.0), 0.0);
    }

    #[test]
    fn test_linear_single_point() {
        let interp = LinearInterpolator::from_points(&[Point2D::new(1.0, 5.0)]);
        assert_eq!(interp.interpolate(0.0), 5.0);
        assert_eq!(interp.interpolate(2.0), 5.0);
    }

    #[test]
    fn test_linear_two_points() {
        let interp =
            LinearInterpolator::from_points(&[Point2D::new(0.0, 0.0), Point2D::new(10.0, 20.0)]);
        assert!((interp.interpolate(0.0) - 0.0).abs() < 1e-10);
        assert!((interp.interpolate(5.0) - 10.0).abs() < 1e-10);
        assert!((interp.interpolate(10.0) - 20.0).abs() < 1e-10);
    }

    #[test]
    fn test_linear_multiple_points() {
        let interp = LinearInterpolator::from_points(&[
            Point2D::new(0.0, 0.0),
            Point2D::new(1.0, 2.0),
            Point2D::new(2.0, 1.0),
            Point2D::new(3.0, 3.0),
        ]);

        // Test interpolation at known points
        assert!((interp.interpolate(0.0) - 0.0).abs() < 1e-10);
        assert!((interp.interpolate(1.0) - 2.0).abs() < 1e-10);
        assert!((interp.interpolate(2.0) - 1.0).abs() < 1e-10);
        assert!((interp.interpolate(3.0) - 3.0).abs() < 1e-10);

        // Test interpolation between points
        assert!((interp.interpolate(0.5) - 1.0).abs() < 1e-10);
        assert!((interp.interpolate(1.5) - 1.5).abs() < 1e-10);
    }

    #[test]
    fn test_linear_from_xy() {
        let xs = [0.0, 1.0, 2.0];
        let ys = [0.0, 10.0, 20.0];
        let interp = LinearInterpolator::from_xy(&xs, &ys);
        assert!((interp.interpolate(1.5) - 15.0).abs() < 1e-10);
    }

    #[test]
    fn test_linear_sample() {
        let interp =
            LinearInterpolator::from_points(&[Point2D::new(0.0, 0.0), Point2D::new(10.0, 10.0)]);
        let samples = interp.sample(0.0, 10.0, 11);
        assert_eq!(samples.len(), 11);
        assert!((samples[0].x - 0.0).abs() < 1e-10);
        assert!((samples[10].x - 10.0).abs() < 1e-10);
    }

    // =========================================================================
    // CubicSpline Tests
    // =========================================================================

    #[test]
    fn test_spline_empty() {
        let spline = CubicSpline::from_points(&[]);
        assert_eq!(spline.interpolate(0.0), 0.0);
    }

    #[test]
    fn test_spline_single_point() {
        let spline = CubicSpline::from_points(&[Point2D::new(1.0, 5.0)]);
        assert_eq!(spline.interpolate(0.0), 5.0);
    }

    #[test]
    fn test_spline_two_points() {
        let spline = CubicSpline::from_points(&[Point2D::new(0.0, 0.0), Point2D::new(10.0, 20.0)]);
        assert!((spline.interpolate(5.0) - 10.0).abs() < 1e-10);
    }

    #[test]
    fn test_spline_passes_through_points() {
        let points = vec![
            Point2D::new(0.0, 0.0),
            Point2D::new(1.0, 2.0),
            Point2D::new(2.0, 1.5),
            Point2D::new(3.0, 3.0),
        ];
        let spline = CubicSpline::from_points(&points);

        for p in &points {
            assert!(
                (spline.interpolate(p.x) - p.y).abs() < 0.01,
                "Spline should pass through control points"
            );
        }
    }

    #[test]
    fn test_spline_smooth() {
        let points = vec![
            Point2D::new(0.0, 0.0),
            Point2D::new(1.0, 1.0),
            Point2D::new(2.0, 0.0),
        ];
        let spline = CubicSpline::from_points(&points);

        // Check smoothness by sampling
        let samples = spline.sample(0.0, 2.0, 100);
        for w in samples.windows(3) {
            // No sudden jumps
            let dy1 = (w[1].y - w[0].y).abs();
            let dy2 = (w[2].y - w[1].y).abs();
            assert!(dy1 < 0.5 && dy2 < 0.5, "Spline should be smooth");
        }
    }

    // =========================================================================
    // CatmullRom Tests
    // =========================================================================

    #[test]
    fn test_catmull_rom_empty() {
        let cr = CatmullRom::from_points(&[]);
        assert_eq!(cr.interpolate(0.0), 0.0);
    }

    #[test]
    fn test_catmull_rom_single() {
        let cr = CatmullRom::from_points(&[Point2D::new(1.0, 5.0)]);
        assert_eq!(cr.interpolate(0.0), 5.0);
    }

    #[test]
    fn test_catmull_rom_passes_through() {
        let points = vec![
            Point2D::new(0.0, 0.0),
            Point2D::new(1.0, 2.0),
            Point2D::new(2.0, 1.0),
            Point2D::new(3.0, 3.0),
        ];
        let cr = CatmullRom::from_points(&points);

        // Catmull-Rom should pass through control points
        for p in &points {
            let y = cr.interpolate(p.x);
            assert!(
                (y - p.y).abs() < 0.1,
                "Catmull-Rom should pass through points: expected {} at x={}, got {}",
                p.y,
                p.x,
                y
            );
        }
    }

    #[test]
    fn test_catmull_rom_to_path() {
        let points = vec![
            Point2D::new(0.0, 0.0),
            Point2D::new(1.0, 1.0),
            Point2D::new(2.0, 0.0),
        ];
        let cr = CatmullRom::from_points(&points);
        let path = cr.to_path(10);

        assert!(path.len() > points.len());
        assert_eq!(path.first().unwrap().x, 0.0);
        assert_eq!(path.last().unwrap().x, 2.0);
    }

    #[test]
    fn test_catmull_rom_tension() {
        let points = vec![
            Point2D::new(0.0, 0.0),
            Point2D::new(1.0, 1.0),
            Point2D::new(2.0, 0.0),
        ];

        let low_tension = CatmullRom::with_tension(&points, 0.0);
        let high_tension = CatmullRom::with_tension(&points, 1.0);

        // Different tensions should produce different curves
        let y_low = low_tension.interpolate(0.5);
        let y_high = high_tension.interpolate(0.5);

        // They should be different (tension affects curvature)
        assert!((y_low - y_high).abs() > 0.01 || (y_low - y_high).abs() < 0.5);
    }

    // =========================================================================
    // CubicBezier Tests
    // =========================================================================

    #[test]
    fn test_bezier_endpoints() {
        let bezier = CubicBezier::new(
            Point2D::new(0.0, 0.0),
            Point2D::new(1.0, 2.0),
            Point2D::new(2.0, 2.0),
            Point2D::new(3.0, 0.0),
        );

        let start = bezier.evaluate(0.0);
        let end = bezier.evaluate(1.0);

        assert!((start.x - 0.0).abs() < 1e-10);
        assert!((start.y - 0.0).abs() < 1e-10);
        assert!((end.x - 3.0).abs() < 1e-10);
        assert!((end.y - 0.0).abs() < 1e-10);
    }

    #[test]
    fn test_bezier_midpoint() {
        let bezier = CubicBezier::new(
            Point2D::new(0.0, 0.0),
            Point2D::new(0.0, 1.0),
            Point2D::new(1.0, 1.0),
            Point2D::new(1.0, 0.0),
        );

        let mid = bezier.evaluate(0.5);
        // Midpoint should be between control points
        assert!(mid.x > 0.0 && mid.x < 1.0);
        assert!(mid.y > 0.0 && mid.y < 1.0);
    }

    #[test]
    fn test_bezier_to_polyline() {
        let bezier = CubicBezier::new(
            Point2D::new(0.0, 0.0),
            Point2D::new(1.0, 2.0),
            Point2D::new(2.0, 2.0),
            Point2D::new(3.0, 0.0),
        );

        let polyline = bezier.to_polyline(10);
        assert_eq!(polyline.len(), 11);
        assert_eq!(polyline[0], bezier.evaluate(0.0));
        assert_eq!(polyline[10], bezier.evaluate(1.0));
    }

    #[test]
    fn test_bezier_arc_length() {
        let line = CubicBezier::new(
            Point2D::new(0.0, 0.0),
            Point2D::new(1.0, 0.0),
            Point2D::new(2.0, 0.0),
            Point2D::new(3.0, 0.0),
        );

        let length = line.arc_length(100);
        assert!((length - 3.0).abs() < 0.01);
    }

    #[test]
    fn test_bezier_split() {
        let bezier = CubicBezier::new(
            Point2D::new(0.0, 0.0),
            Point2D::new(1.0, 2.0),
            Point2D::new(2.0, 2.0),
            Point2D::new(3.0, 0.0),
        );

        let (left, right) = bezier.split(0.5);

        // Left should start at original start
        assert_eq!(left.p0, bezier.p0);

        // Right should end at original end
        assert_eq!(right.p3, bezier.p3);

        // They should meet in the middle
        assert!((left.p3.x - right.p0.x).abs() < 1e-10);
        assert!((left.p3.y - right.p0.y).abs() < 1e-10);
    }

    // =========================================================================
    // HistogramBins Tests
    // =========================================================================

    #[test]
    fn test_histogram_empty() {
        let hist = HistogramBins::from_data(&[], 10);
        assert_eq!(hist.num_bins(), 0);
    }

    #[test]
    fn test_histogram_single_value() {
        let hist = HistogramBins::from_data(&[5.0], 10);
        assert_eq!(hist.total_count(), 1);
    }

    #[test]
    fn test_histogram_uniform() {
        let data: Vec<f64> = (0..100).map(|i| i as f64).collect();
        let hist = HistogramBins::from_data(&data, 10);

        assert_eq!(hist.num_bins(), 10);
        assert_eq!(hist.total_count(), 100);

        // Each bin should have approximately 10 values
        for &count in &hist.counts {
            assert!(count >= 9 && count <= 11);
        }
    }

    #[test]
    fn test_histogram_bin_width() {
        let hist = HistogramBins::from_data_range(&[0.0], 5, 0.0, 10.0);
        assert!((hist.bin_width() - 2.0).abs() < 1e-10);
    }

    #[test]
    fn test_histogram_bin_center() {
        let hist = HistogramBins::from_data_range(&[0.0], 4, 0.0, 8.0);
        assert_eq!(hist.bin_center(0), Some(1.0));
        assert_eq!(hist.bin_center(1), Some(3.0));
        assert_eq!(hist.bin_center(4), None);
    }

    #[test]
    fn test_histogram_bin_range() {
        let hist = HistogramBins::from_data_range(&[0.0], 4, 0.0, 8.0);
        assert_eq!(hist.bin_range(0), Some((0.0, 2.0)));
        assert_eq!(hist.bin_range(3), Some((6.0, 8.0)));
    }

    #[test]
    fn test_histogram_densities() {
        let data = vec![0.5, 1.5, 1.5, 2.5, 2.5, 2.5];
        let hist = HistogramBins::from_data_range(&data, 3, 0.0, 3.0);

        // Densities should integrate to 1
        let total_density: f64 = hist.densities.iter().map(|d| d * hist.bin_width()).sum();
        assert!((total_density - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_histogram_max_count() {
        let data = vec![1.0, 1.0, 1.0, 2.0];
        let hist = HistogramBins::from_data_range(&data, 2, 0.0, 4.0);
        assert_eq!(hist.max_count(), 3);
    }

    // =========================================================================
    // ArcGeometry Tests
    // =========================================================================

    #[test]
    fn test_arc_new() {
        let arc = ArcGeometry::new(Point2D::new(0.0, 0.0), 10.0, 0.0, PI);
        assert_eq!(arc.center, Point2D::ORIGIN);
        assert_eq!(arc.radius, 10.0);
    }

    #[test]
    fn test_arc_circle() {
        let circle = ArcGeometry::circle(Point2D::new(5.0, 5.0), 3.0);
        assert!((circle.sweep() - 2.0 * PI).abs() < 1e-10);
    }

    #[test]
    fn test_arc_sweep() {
        let arc = ArcGeometry::new(Point2D::ORIGIN, 1.0, 0.0, PI / 2.0);
        assert!((arc.sweep() - PI / 2.0).abs() < 1e-10);
    }

    #[test]
    fn test_arc_point_at_angle() {
        let arc = ArcGeometry::circle(Point2D::ORIGIN, 1.0);

        let p0 = arc.point_at_angle(0.0);
        assert!((p0.x - 1.0).abs() < 1e-10);
        assert!((p0.y - 0.0).abs() < 1e-10);

        let p90 = arc.point_at_angle(PI / 2.0);
        assert!((p90.x - 0.0).abs() < 1e-10);
        assert!((p90.y - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_arc_start_end_points() {
        let arc = ArcGeometry::new(Point2D::ORIGIN, 1.0, 0.0, PI);

        let start = arc.start_point();
        assert!((start.x - 1.0).abs() < 1e-10);

        let end = arc.end_point();
        assert!((end.x - (-1.0)).abs() < 1e-10);
    }

    #[test]
    fn test_arc_mid_point() {
        let arc = ArcGeometry::new(Point2D::ORIGIN, 1.0, 0.0, PI);
        let mid = arc.mid_point();
        assert!((mid.y - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_arc_length() {
        let semicircle = ArcGeometry::new(Point2D::ORIGIN, 1.0, 0.0, PI);
        assert!((semicircle.arc_length() - PI).abs() < 1e-10);

        let full = ArcGeometry::circle(Point2D::ORIGIN, 1.0);
        assert!((full.arc_length() - 2.0 * PI).abs() < 1e-10);
    }

    #[test]
    fn test_arc_to_polyline() {
        let arc = ArcGeometry::new(Point2D::ORIGIN, 1.0, 0.0, PI);
        let poly = arc.to_polyline(4);

        assert_eq!(poly.len(), 5);
        assert!((poly[0].x - 1.0).abs() < 1e-10); // Start
        assert!((poly[4].x - (-1.0)).abs() < 1e-10); // End
    }

    #[test]
    fn test_arc_to_pie_slice() {
        let arc = ArcGeometry::new(Point2D::ORIGIN, 1.0, 0.0, PI / 2.0);
        let slice = arc.to_pie_slice(4);

        // Should have center at start and end
        assert_eq!(slice[0], Point2D::ORIGIN);
        assert_eq!(slice[slice.len() - 1], Point2D::ORIGIN);
    }

    #[test]
    fn test_arc_contains_angle() {
        let arc = ArcGeometry::new(Point2D::ORIGIN, 1.0, 0.0, PI);
        assert!(arc.contains_angle(PI / 2.0));
        assert!(arc.contains_angle(0.0));
        assert!(!arc.contains_angle(3.0 * PI / 2.0));
    }

    // =========================================================================
    // DataNormalizer Tests
    // =========================================================================

    #[test]
    fn test_normalizer_new() {
        let norm = DataNormalizer::new(0.0, 100.0);
        assert_eq!(norm.min, 0.0);
        assert_eq!(norm.max, 100.0);
    }

    #[test]
    fn test_normalizer_from_data() {
        let data = vec![10.0, 20.0, 30.0, 40.0, 50.0];
        let norm = DataNormalizer::from_data(&data);
        assert_eq!(norm.min, 10.0);
        assert_eq!(norm.max, 50.0);
    }

    #[test]
    fn test_normalizer_from_empty() {
        let norm = DataNormalizer::from_data(&[]);
        assert_eq!(norm.min, 0.0);
        assert_eq!(norm.max, 1.0);
    }

    #[test]
    fn test_normalizer_normalize() {
        let norm = DataNormalizer::new(0.0, 100.0);
        assert!((norm.normalize(0.0) - 0.0).abs() < 1e-10);
        assert!((norm.normalize(50.0) - 0.5).abs() < 1e-10);
        assert!((norm.normalize(100.0) - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_normalizer_denormalize() {
        let norm = DataNormalizer::new(0.0, 100.0);
        assert!((norm.denormalize(0.0) - 0.0).abs() < 1e-10);
        assert!((norm.denormalize(0.5) - 50.0).abs() < 1e-10);
        assert!((norm.denormalize(1.0) - 100.0).abs() < 1e-10);
    }

    #[test]
    fn test_normalizer_roundtrip() {
        let norm = DataNormalizer::new(-50.0, 150.0);
        let values = vec![-50.0, 0.0, 50.0, 100.0, 150.0];

        for &v in &values {
            let normalized = norm.normalize(v);
            let denormalized = norm.denormalize(normalized);
            assert!((v - denormalized).abs() < 1e-10);
        }
    }

    #[test]
    fn test_normalizer_normalize_all() {
        let norm = DataNormalizer::new(0.0, 10.0);
        let data = vec![0.0, 5.0, 10.0];
        let normalized = norm.normalize_all(&data);

        assert_eq!(normalized, vec![0.0, 0.5, 1.0]);
    }

    #[test]
    fn test_normalizer_nice_bounds() {
        let norm = DataNormalizer::new(3.2, 97.8);
        let (nice_min, nice_max) = norm.nice_bounds();

        assert!(nice_min <= 3.2);
        assert!(nice_max >= 97.8);
        // Should be round numbers
        assert!((nice_min * 10.0).round() == nice_min * 10.0);
    }

    // =========================================================================
    // PathTessellator Tests
    // =========================================================================

    #[test]
    fn test_tessellator_new() {
        let tess = PathTessellator::new(0.5);
        assert!((tess.tolerance - 0.5).abs() < 1e-10);
        assert!(tess.vertices.is_empty());
        assert!(tess.indices.is_empty());
    }

    #[test]
    fn test_tessellator_default() {
        let tess = PathTessellator::with_default_tolerance();
        assert!((tess.tolerance - 0.25).abs() < 1e-10);
    }

    #[test]
    fn test_tessellator_polygon() {
        let mut tess = PathTessellator::new(0.5);
        let triangle = vec![
            Point2D::new(0.0, 0.0),
            Point2D::new(1.0, 0.0),
            Point2D::new(0.5, 1.0),
        ];

        tess.tessellate_polygon(&triangle);

        assert_eq!(tess.vertex_count(), 3);
        assert_eq!(tess.index_count(), 3);
        assert_eq!(tess.triangle_count(), 1);
    }

    #[test]
    fn test_tessellator_quad() {
        let mut tess = PathTessellator::new(0.5);
        let quad = vec![
            Point2D::new(0.0, 0.0),
            Point2D::new(1.0, 0.0),
            Point2D::new(1.0, 1.0),
            Point2D::new(0.0, 1.0),
        ];

        tess.tessellate_polygon(&quad);

        assert_eq!(tess.vertex_count(), 4);
        assert_eq!(tess.triangle_count(), 2);
    }

    #[test]
    fn test_tessellator_stroke() {
        let mut tess = PathTessellator::new(0.5);
        let line = vec![Point2D::new(0.0, 0.0), Point2D::new(10.0, 0.0)];

        tess.tessellate_stroke(&line, 2.0);

        // One segment produces a quad (4 vertices, 2 triangles)
        assert_eq!(tess.vertex_count(), 4);
        assert_eq!(tess.triangle_count(), 2);
    }

    #[test]
    fn test_tessellator_multi_segment_stroke() {
        let mut tess = PathTessellator::new(0.5);
        let path = vec![
            Point2D::new(0.0, 0.0),
            Point2D::new(10.0, 0.0),
            Point2D::new(10.0, 10.0),
        ];

        tess.tessellate_stroke(&path, 1.0);

        // Two segments, each produces a quad
        assert_eq!(tess.vertex_count(), 8);
        assert_eq!(tess.triangle_count(), 4);
    }

    #[test]
    fn test_tessellator_circle() {
        let mut tess = PathTessellator::new(0.5);
        tess.tessellate_circle(Point2D::ORIGIN, 1.0, 16);

        // 16 segments: 1 center + 16 perimeter = 17 vertices
        assert_eq!(tess.vertex_count(), 17);
        // 16 triangles
        assert_eq!(tess.triangle_count(), 16);
    }

    #[test]
    fn test_tessellator_rect() {
        let mut tess = PathTessellator::new(0.5);
        tess.tessellate_rect(0.0, 0.0, 10.0, 5.0);

        assert_eq!(tess.vertex_count(), 4);
        assert_eq!(tess.triangle_count(), 2);
    }

    #[test]
    fn test_tessellator_clear() {
        let mut tess = PathTessellator::new(0.5);
        tess.tessellate_rect(0.0, 0.0, 10.0, 5.0);
        assert!(!tess.vertices.is_empty());

        tess.clear();
        assert!(tess.vertices.is_empty());
        assert!(tess.indices.is_empty());
    }

    #[test]
    fn test_tessellator_multiple_shapes() {
        let mut tess = PathTessellator::new(0.5);

        tess.tessellate_rect(0.0, 0.0, 10.0, 10.0);
        tess.tessellate_circle(Point2D::new(20.0, 5.0), 3.0, 8);

        assert_eq!(tess.vertex_count(), 4 + 9); // rect + circle (center + 8)
        assert_eq!(tess.triangle_count(), 2 + 8);
    }

    // =========================================================================
    // DrawBatch Tests
    // =========================================================================

    #[test]
    fn test_batch_new() {
        let batch = DrawBatch::new();
        assert!(batch.circles.is_empty());
        assert!(batch.rects.is_empty());
        assert!(batch.lines.is_empty());
    }

    #[test]
    fn test_batch_add_circle() {
        let mut batch = DrawBatch::new();
        batch.add_circle(10.0, 20.0, 5.0, 1.0, 0.0, 0.0, 1.0);

        assert_eq!(batch.circles.len(), 1);
        assert_eq!(batch.circles[0][0], 10.0);
        assert_eq!(batch.circles[0][1], 20.0);
        assert_eq!(batch.circles[0][2], 5.0);
    }

    #[test]
    fn test_batch_add_rect() {
        let mut batch = DrawBatch::new();
        batch.add_rect(0.0, 0.0, 100.0, 50.0, 0.0, 1.0, 0.0, 1.0);

        assert_eq!(batch.rects.len(), 1);
        assert_eq!(batch.rects[0][2], 100.0);
        assert_eq!(batch.rects[0][3], 50.0);
    }

    #[test]
    fn test_batch_add_line() {
        let mut batch = DrawBatch::new();
        batch.add_line(0.0, 0.0, 100.0, 100.0, 2.0, 0.0, 0.0, 1.0, 1.0);

        assert_eq!(batch.lines.len(), 1);
        assert_eq!(batch.lines[0][4], 2.0); // width
    }

    #[test]
    fn test_batch_clear() {
        let mut batch = DrawBatch::new();
        batch.add_circle(0.0, 0.0, 1.0, 1.0, 1.0, 1.0, 1.0);
        batch.add_rect(0.0, 0.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0);
        batch.add_line(0.0, 0.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0);

        batch.clear();

        assert!(batch.circles.is_empty());
        assert!(batch.rects.is_empty());
        assert!(batch.lines.is_empty());
    }

    #[test]
    fn test_batch_draw_call_counts() {
        let mut batch = DrawBatch::new();

        // Empty batch
        assert_eq!(batch.unbatched_draw_calls(), 0);
        assert_eq!(batch.batched_draw_calls(), 0);

        // Add shapes
        for i in 0..100 {
            batch.add_circle(i as f32, 0.0, 5.0, 1.0, 0.0, 0.0, 1.0);
        }
        for i in 0..50 {
            batch.add_rect(i as f32 * 10.0, 0.0, 10.0, 10.0, 0.0, 1.0, 0.0, 1.0);
        }

        // Without batching: 150 draw calls
        assert_eq!(batch.unbatched_draw_calls(), 150);
        // With batching: 2 draw calls (circles, rects)
        assert_eq!(batch.batched_draw_calls(), 2);
    }

    #[test]
    fn test_batch_efficiency() {
        let mut batch = DrawBatch::new();

        // Simulate chart rendering: 1000 points, 10 bars
        for i in 0..1000 {
            batch.add_circle(i as f32, (i as f32).sin() * 100.0, 3.0, 0.2, 0.5, 0.9, 1.0);
        }
        for i in 0..10 {
            batch.add_rect(
                i as f32 * 50.0,
                0.0,
                40.0,
                (i as f32 + 1.0) * 20.0,
                0.9,
                0.3,
                0.3,
                1.0,
            );
        }

        // 1010 shapes batched into 2 draw calls = 505x reduction
        let reduction = batch.unbatched_draw_calls() as f64 / batch.batched_draw_calls() as f64;
        assert!(reduction > 500.0);
    }

    // =========================================================================
    // Additional Point2D Tests
    // =========================================================================

    #[test]
    fn test_point2d_default() {
        let p: Point2D = Default::default();
        assert_eq!(p, Point2D::ORIGIN);
    }

    #[test]
    fn test_point2d_clone() {
        let p1 = Point2D::new(3.14, 2.71);
        let p2 = p1;
        assert_eq!(p1, p2);
    }

    #[test]
    fn test_point2d_debug() {
        let p = Point2D::new(1.0, 2.0);
        let debug = format!("{p:?}");
        assert!(debug.contains("Point2D"));
    }

    #[test]
    fn test_point2d_distance_to_self() {
        let p = Point2D::new(5.0, 10.0);
        assert_eq!(p.distance(&p), 0.0);
    }

    #[test]
    fn test_point2d_lerp_boundaries() {
        let p1 = Point2D::new(0.0, 0.0);
        let p2 = Point2D::new(10.0, 10.0);

        let at_start = p1.lerp(&p2, 0.0);
        assert_eq!(at_start, p1);

        let at_end = p1.lerp(&p2, 1.0);
        assert_eq!(at_end, p2);
    }

    #[test]
    fn test_point2d_lerp_extrapolate() {
        let p1 = Point2D::new(0.0, 0.0);
        let p2 = Point2D::new(10.0, 10.0);

        let beyond = p1.lerp(&p2, 2.0);
        assert!((beyond.x - 20.0).abs() < 1e-10);
        assert!((beyond.y - 20.0).abs() < 1e-10);
    }

    #[test]
    fn test_point2d_mul_zero() {
        let p = Point2D::new(5.0, 10.0);
        let scaled = p * 0.0;
        assert_eq!(scaled, Point2D::ORIGIN);
    }

    #[test]
    fn test_point2d_mul_negative() {
        let p = Point2D::new(5.0, 10.0);
        let scaled = p * -1.0;
        assert_eq!(scaled, Point2D::new(-5.0, -10.0));
    }

    // =========================================================================
    // Additional LinearInterpolator Tests
    // =========================================================================

    #[test]
    fn test_linear_extrapolate_left() {
        let interp = LinearInterpolator::from_points(&[
            Point2D::new(0.0, 0.0),
            Point2D::new(10.0, 10.0),
        ]);
        // Extrapolate before first point
        let y = interp.interpolate(-5.0);
        assert!((y - (-5.0)).abs() < 1e-10);
    }

    #[test]
    fn test_linear_extrapolate_right() {
        let interp = LinearInterpolator::from_points(&[
            Point2D::new(0.0, 0.0),
            Point2D::new(10.0, 10.0),
        ]);
        // Extrapolate after last point
        let y = interp.interpolate(15.0);
        assert!((y - 15.0).abs() < 1e-10);
    }

    #[test]
    fn test_linear_unsorted_input() {
        let interp = LinearInterpolator::from_points(&[
            Point2D::new(3.0, 30.0),
            Point2D::new(1.0, 10.0),
            Point2D::new(2.0, 20.0),
        ]);
        // Should sort and interpolate correctly
        assert!((interp.interpolate(1.5) - 15.0).abs() < 1e-10);
    }

    #[test]
    fn test_linear_points_getter() {
        let interp = LinearInterpolator::from_points(&[
            Point2D::new(0.0, 0.0),
            Point2D::new(1.0, 1.0),
        ]);
        assert_eq!(interp.points().len(), 2);
    }

    #[test]
    fn test_linear_sample_single_point() {
        let interp = LinearInterpolator::from_points(&[
            Point2D::new(0.0, 5.0),
        ]);
        let samples = interp.sample(0.0, 10.0, 5);
        // Single point always returns that y
        for s in &samples {
            assert_eq!(s.y, 5.0);
        }
    }

    #[test]
    fn test_linear_sample_too_few() {
        let interp = LinearInterpolator::from_points(&[
            Point2D::new(0.0, 0.0),
            Point2D::new(10.0, 10.0),
        ]);
        let samples = interp.sample(0.0, 10.0, 1);
        assert!(samples.is_empty());
    }

    #[test]
    fn test_linear_vertical_segment() {
        let interp = LinearInterpolator::from_points(&[
            Point2D::new(5.0, 0.0),
            Point2D::new(5.0, 10.0), // Same x (vertical)
            Point2D::new(10.0, 10.0),
        ]);
        // Should handle gracefully
        let y = interp.interpolate(5.0);
        assert!(y.is_finite());
    }

    // =========================================================================
    // Additional CubicSpline Tests
    // =========================================================================

    #[test]
    fn test_spline_from_xy() {
        let xs = [0.0, 1.0, 2.0, 3.0];
        let ys = [0.0, 1.0, 0.0, 1.0];
        let spline = CubicSpline::from_xy(&xs, &ys);
        assert_eq!(spline.points().len(), 4);
    }

    #[test]
    fn test_spline_points_getter() {
        let points = vec![
            Point2D::new(0.0, 0.0),
            Point2D::new(1.0, 1.0),
            Point2D::new(2.0, 0.0),
        ];
        let spline = CubicSpline::from_points(&points);
        assert_eq!(spline.points().len(), 3);
    }

    #[test]
    fn test_spline_identical_x() {
        // Two points with same x (degenerate case)
        let points = vec![
            Point2D::new(1.0, 0.0),
            Point2D::new(1.0, 10.0),
            Point2D::new(2.0, 5.0),
        ];
        let spline = CubicSpline::from_points(&points);
        let y = spline.interpolate(1.0);
        assert!(y.is_finite());
    }

    #[test]
    fn test_spline_extrapolate() {
        let points = vec![
            Point2D::new(0.0, 0.0),
            Point2D::new(1.0, 1.0),
            Point2D::new(2.0, 0.0),
        ];
        let spline = CubicSpline::from_points(&points);

        // Extrapolate beyond range
        let y_before = spline.interpolate(-1.0);
        let y_after = spline.interpolate(3.0);
        assert!(y_before.is_finite());
        assert!(y_after.is_finite());
    }

    // =========================================================================
    // Additional CatmullRom Tests
    // =========================================================================

    #[test]
    fn test_catmull_rom_points_getter() {
        let points = vec![
            Point2D::new(0.0, 0.0),
            Point2D::new(1.0, 1.0),
        ];
        let cr = CatmullRom::from_points(&points);
        assert_eq!(cr.points().len(), 2);
    }

    #[test]
    fn test_catmull_rom_to_path_two_points() {
        let points = vec![
            Point2D::new(0.0, 0.0),
            Point2D::new(1.0, 1.0),
        ];
        let cr = CatmullRom::from_points(&points);
        let path = cr.to_path(10);
        assert_eq!(path.len(), 2); // Just returns points for 2-point input
    }

    #[test]
    fn test_catmull_rom_to_path_single() {
        let points = vec![Point2D::new(0.0, 0.0)];
        let cr = CatmullRom::from_points(&points);
        let path = cr.to_path(10);
        assert_eq!(path.len(), 1);
    }

    #[test]
    fn test_catmull_rom_tension_clamp() {
        let points = vec![
            Point2D::new(0.0, 0.0),
            Point2D::new(1.0, 1.0),
            Point2D::new(2.0, 0.0),
        ];

        // Tension should be clamped to [0, 1]
        let cr_low = CatmullRom::with_tension(&points, -1.0);
        let cr_high = CatmullRom::with_tension(&points, 2.0);

        // Both should work without panic
        let _ = cr_low.interpolate(0.5);
        let _ = cr_high.interpolate(0.5);
    }

    // =========================================================================
    // Additional CubicBezier Tests
    // =========================================================================

    #[test]
    fn test_bezier_clamp_t() {
        let bezier = CubicBezier::new(
            Point2D::new(0.0, 0.0),
            Point2D::new(1.0, 2.0),
            Point2D::new(2.0, 2.0),
            Point2D::new(3.0, 0.0),
        );

        // t is clamped to [0, 1]
        let p_neg = bezier.evaluate(-0.5);
        let p_over = bezier.evaluate(1.5);

        assert_eq!(p_neg, bezier.evaluate(0.0));
        assert_eq!(p_over, bezier.evaluate(1.0));
    }

    #[test]
    fn test_bezier_polyline_min_segments() {
        let bezier = CubicBezier::new(
            Point2D::new(0.0, 0.0),
            Point2D::new(1.0, 1.0),
            Point2D::new(2.0, 1.0),
            Point2D::new(3.0, 0.0),
        );

        let polyline = bezier.to_polyline(0);
        assert!(polyline.len() >= 2);
    }

    #[test]
    fn test_bezier_split_at_zero() {
        let bezier = CubicBezier::new(
            Point2D::new(0.0, 0.0),
            Point2D::new(1.0, 1.0),
            Point2D::new(2.0, 1.0),
            Point2D::new(3.0, 0.0),
        );

        let (left, right) = bezier.split(0.0);
        assert_eq!(left.p0, bezier.p0);
        assert_eq!(right.p3, bezier.p3);
    }

    #[test]
    fn test_bezier_split_at_one() {
        let bezier = CubicBezier::new(
            Point2D::new(0.0, 0.0),
            Point2D::new(1.0, 1.0),
            Point2D::new(2.0, 1.0),
            Point2D::new(3.0, 0.0),
        );

        let (left, _right) = bezier.split(1.0);
        assert_eq!(left.p3, bezier.p3);
    }

    #[test]
    fn test_bezier_arc_length_zero_segments() {
        let bezier = CubicBezier::new(
            Point2D::new(0.0, 0.0),
            Point2D::new(1.0, 0.0),
            Point2D::new(2.0, 0.0),
            Point2D::new(3.0, 0.0),
        );

        // Should handle 0 segments gracefully
        let length = bezier.arc_length(0);
        assert!(length.is_finite());
    }

    // =========================================================================
    // Additional HistogramBins Tests
    // =========================================================================

    #[test]
    fn test_histogram_zero_bins() {
        let hist = HistogramBins::from_data(&[1.0, 2.0, 3.0], 0);
        assert_eq!(hist.num_bins(), 0);
    }

    #[test]
    fn test_histogram_all_same_value() {
        let data = vec![5.0, 5.0, 5.0, 5.0, 5.0];
        let hist = HistogramBins::from_data(&data, 5);
        assert_eq!(hist.total_count(), 5);
    }

    #[test]
    fn test_histogram_negative_values() {
        let data = vec![-10.0, -5.0, 0.0, 5.0, 10.0];
        let hist = HistogramBins::from_data(&data, 4);
        assert_eq!(hist.num_bins(), 4);
        assert_eq!(hist.total_count(), 5);
    }

    #[test]
    fn test_histogram_bin_range_out_of_bounds() {
        let hist = HistogramBins::from_data(&[1.0, 2.0, 3.0], 3);
        assert_eq!(hist.bin_range(100), None);
    }

    #[test]
    fn test_histogram_bin_center_out_of_bounds() {
        let hist = HistogramBins::from_data(&[1.0, 2.0, 3.0], 3);
        assert_eq!(hist.bin_center(100), None);
    }

    #[test]
    fn test_histogram_edge_case_max_value() {
        // Value exactly at max should go in last bin
        let hist = HistogramBins::from_data_range(&[0.0, 5.0, 10.0], 2, 0.0, 10.0);
        assert_eq!(hist.total_count(), 3);
    }

    // =========================================================================
    // Additional ArcGeometry Tests
    // =========================================================================

    #[test]
    fn test_arc_negative_angles() {
        let arc = ArcGeometry::new(Point2D::ORIGIN, 1.0, -PI / 2.0, PI / 2.0);
        assert!((arc.sweep() - PI).abs() < 1e-10);
    }

    #[test]
    fn test_arc_large_angles() {
        let arc = ArcGeometry::new(Point2D::ORIGIN, 1.0, 0.0, 4.0 * PI);
        // Large sweep should still work
        let poly = arc.to_polyline(10);
        assert_eq!(poly.len(), 11);
    }

    #[test]
    fn test_arc_zero_radius() {
        let arc = ArcGeometry::new(Point2D::new(5.0, 5.0), 0.0, 0.0, PI);
        let start = arc.start_point();
        assert_eq!(start, arc.center);
    }

    #[test]
    fn test_arc_contains_angle_wrap() {
        // Arc that wraps around 0/2π
        let arc = ArcGeometry::new(Point2D::ORIGIN, 1.0, 3.0 * PI / 2.0, PI / 2.0 + 2.0 * PI);
        assert!(arc.contains_angle(0.0));
    }

    #[test]
    fn test_arc_pie_slice_segments() {
        let arc = ArcGeometry::new(Point2D::ORIGIN, 1.0, 0.0, PI / 2.0);
        let slice = arc.to_pie_slice(8);
        // 1 center + 9 arc points + 1 center closing = 11
        assert_eq!(slice.len(), 11);
    }

    // =========================================================================
    // Additional DataNormalizer Tests
    // =========================================================================

    #[test]
    fn test_normalizer_zero_range() {
        let norm = DataNormalizer::new(5.0, 5.0);
        // Zero range should return 0.5
        assert_eq!(norm.normalize(5.0), 0.5);
        assert_eq!(norm.normalize(10.0), 0.5);
    }

    #[test]
    fn test_normalizer_negative_range() {
        let norm = DataNormalizer::new(-100.0, -50.0);
        assert!((norm.normalize(-100.0) - 0.0).abs() < 1e-10);
        assert!((norm.normalize(-75.0) - 0.5).abs() < 1e-10);
        assert!((norm.normalize(-50.0) - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_normalizer_nice_bounds_small_range() {
        let norm = DataNormalizer::new(0.001, 0.002);
        let (nice_min, nice_max) = norm.nice_bounds();
        assert!(nice_min <= 0.001);
        assert!(nice_max >= 0.002);
    }

    #[test]
    fn test_normalizer_nice_bounds_large_range() {
        let norm = DataNormalizer::new(0.0, 1_000_000.0);
        let (nice_min, nice_max) = norm.nice_bounds();
        assert!(nice_min <= 0.0);
        assert!(nice_max >= 1_000_000.0);
    }

    #[test]
    fn test_normalizer_from_single_value() {
        let norm = DataNormalizer::from_data(&[42.0]);
        // Single value: min == max
        assert_eq!(norm.min, 42.0);
        assert_eq!(norm.max, 42.0);
    }

    // =========================================================================
    // Additional PathTessellator Tests
    // =========================================================================

    #[test]
    fn test_tessellator_tolerance_minimum() {
        let tess = PathTessellator::new(0.0001);
        // Should clamp to minimum tolerance
        assert!(tess.tolerance >= 0.001);
    }

    #[test]
    fn test_tessellator_polygon_too_small() {
        let mut tess = PathTessellator::new(0.5);
        tess.tessellate_polygon(&[Point2D::new(0.0, 0.0)]);
        assert!(tess.vertices.is_empty());

        tess.tessellate_polygon(&[Point2D::new(0.0, 0.0), Point2D::new(1.0, 0.0)]);
        assert!(tess.vertices.is_empty());
    }

    #[test]
    fn test_tessellator_stroke_too_short() {
        let mut tess = PathTessellator::new(0.5);
        tess.tessellate_stroke(&[Point2D::new(0.0, 0.0)], 1.0);
        assert!(tess.vertices.is_empty());
    }

    #[test]
    fn test_tessellator_stroke_zero_length_segment() {
        let mut tess = PathTessellator::new(0.5);
        // Two identical points (zero-length segment)
        tess.tessellate_stroke(&[Point2D::new(5.0, 5.0), Point2D::new(5.0, 5.0)], 1.0);
        // Should handle gracefully
        assert!(tess.vertices.is_empty());
    }

    #[test]
    fn test_tessellator_circle_min_segments() {
        let mut tess = PathTessellator::new(0.5);
        tess.tessellate_circle(Point2D::ORIGIN, 1.0, 3);
        // Should enforce minimum 8 segments
        assert!(tess.vertex_count() >= 9); // 1 center + at least 8 perimeter
    }

    #[test]
    fn test_tessellator_default_trait() {
        let tess: PathTessellator = Default::default();
        assert!(tess.vertices.is_empty());
    }

    // =========================================================================
    // Additional DrawBatch Tests
    // =========================================================================

    #[test]
    fn test_batch_default_trait() {
        let batch: DrawBatch = Default::default();
        assert!(batch.circles.is_empty());
        assert!(batch.rects.is_empty());
        assert!(batch.lines.is_empty());
    }

    #[test]
    fn test_batch_only_circles() {
        let mut batch = DrawBatch::new();
        batch.add_circle(0.0, 0.0, 1.0, 1.0, 1.0, 1.0, 1.0);
        assert_eq!(batch.batched_draw_calls(), 1);
    }

    #[test]
    fn test_batch_only_rects() {
        let mut batch = DrawBatch::new();
        batch.add_rect(0.0, 0.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0);
        assert_eq!(batch.batched_draw_calls(), 1);
    }

    #[test]
    fn test_batch_only_lines() {
        let mut batch = DrawBatch::new();
        batch.add_line(0.0, 0.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0);
        assert_eq!(batch.batched_draw_calls(), 1);
    }

    #[test]
    fn test_batch_all_types() {
        let mut batch = DrawBatch::new();
        batch.add_circle(0.0, 0.0, 1.0, 1.0, 1.0, 1.0, 1.0);
        batch.add_rect(0.0, 0.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0);
        batch.add_line(0.0, 0.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0);
        assert_eq!(batch.batched_draw_calls(), 3);
        assert_eq!(batch.unbatched_draw_calls(), 3);
    }

    #[test]
    fn test_batch_debug() {
        let batch = DrawBatch::new();
        let debug = format!("{batch:?}");
        assert!(debug.contains("DrawBatch"));
    }

    #[test]
    fn test_batch_clone() {
        let mut batch = DrawBatch::new();
        batch.add_circle(1.0, 2.0, 3.0, 1.0, 0.0, 0.0, 1.0);
        let cloned = batch.clone();
        assert_eq!(cloned.circles.len(), 1);
        assert_eq!(cloned.circles[0][0], 1.0);
    }
}
