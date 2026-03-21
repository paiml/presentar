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
        let data: Vec<f64> = (0..100).map(f64::from).collect();
        let hist = HistogramBins::from_data(&data, 10);

        assert_eq!(hist.num_bins(), 10);
        assert_eq!(hist.total_count(), 100);

        // Each bin should have approximately 10 values
        for &count in &hist.counts {
            assert!((9..=11).contains(&count));
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
        assert!(2.0f64.mul_add(-PI, circle.sweep()).abs() < 1e-10);
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
        assert!(2.0f64.mul_add(-PI, full.arc_length()).abs() < 1e-10);
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
        let p: Point2D = Point2D::default();
        assert_eq!(p, Point2D::ORIGIN);
    }

    #[test]
    fn test_point2d_clone() {
        let p1 = Point2D::new(2.5, 2.71);
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
        let interp =
            LinearInterpolator::from_points(&[Point2D::new(0.0, 0.0), Point2D::new(10.0, 10.0)]);
        // Extrapolate before first point
        let y = interp.interpolate(-5.0);
        assert!((y - (-5.0)).abs() < 1e-10);
    }

    #[test]
    fn test_linear_extrapolate_right() {
        let interp =
            LinearInterpolator::from_points(&[Point2D::new(0.0, 0.0), Point2D::new(10.0, 10.0)]);
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
        let interp =
            LinearInterpolator::from_points(&[Point2D::new(0.0, 0.0), Point2D::new(1.0, 1.0)]);
        assert_eq!(interp.points().len(), 2);
    }

    #[test]
    fn test_linear_sample_single_point() {
        let interp = LinearInterpolator::from_points(&[Point2D::new(0.0, 5.0)]);
        let samples = interp.sample(0.0, 10.0, 5);
        // Single point always returns that y
        for s in &samples {
            assert_eq!(s.y, 5.0);
        }
    }

    #[test]
    fn test_linear_sample_too_few() {
        let interp =
            LinearInterpolator::from_points(&[Point2D::new(0.0, 0.0), Point2D::new(10.0, 10.0)]);
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
        let points = vec![Point2D::new(0.0, 0.0), Point2D::new(1.0, 1.0)];
        let cr = CatmullRom::from_points(&points);
        assert_eq!(cr.points().len(), 2);
    }

    #[test]
    fn test_catmull_rom_to_path_two_points() {
        let points = vec![Point2D::new(0.0, 0.0), Point2D::new(1.0, 1.0)];
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
        let arc = ArcGeometry::new(
            Point2D::ORIGIN,
            1.0,
            3.0 * PI / 2.0,
            2.0f64.mul_add(PI, PI / 2.0),
        );
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
        let tess = PathTessellator::default();
        assert!(tess.vertices.is_empty());
    }

    // =========================================================================
    // Additional DrawBatch Tests
    // =========================================================================

    #[test]
    fn test_batch_default_trait() {
        let batch = DrawBatch::default();
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
