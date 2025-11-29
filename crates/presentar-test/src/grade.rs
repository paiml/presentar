//! Grade scoring system for quality evaluation.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Quality grade levels (A-F).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum Grade {
    /// Failing (<60%)
    #[default]
    F,
    /// Poor (60-69%)
    D,
    /// Satisfactory (70-79%)
    C,
    /// Good (80-89%)
    B,
    /// Excellent (90-100%)
    A,
}

impl PartialOrd for Grade {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Grade {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // Compare by minimum percentage (A > B > C > D > F)
        self.min_percentage()
            .partial_cmp(&other.min_percentage())
            .unwrap_or(std::cmp::Ordering::Equal)
    }
}

impl Grade {
    /// Create a grade from a percentage score.
    #[must_use]
    pub fn from_percentage(percent: f32) -> Self {
        match percent {
            p if p >= 90.0 => Self::A,
            p if p >= 80.0 => Self::B,
            p if p >= 70.0 => Self::C,
            p if p >= 60.0 => Self::D,
            _ => Self::F,
        }
    }

    /// Get the minimum percentage for this grade.
    #[must_use]
    pub fn min_percentage(&self) -> f32 {
        match self {
            Self::A => 90.0,
            Self::B => 80.0,
            Self::C => 70.0,
            Self::D => 60.0,
            Self::F => 0.0,
        }
    }

    /// Get grade as a letter string.
    #[must_use]
    pub fn letter(&self) -> &'static str {
        match self {
            Self::A => "A",
            Self::B => "B",
            Self::C => "C",
            Self::D => "D",
            Self::F => "F",
        }
    }

    /// Check if this is a passing grade (C or better).
    #[must_use]
    pub fn is_passing(&self) -> bool {
        matches!(self, Self::A | Self::B | Self::C)
    }
}

impl std::fmt::Display for Grade {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.letter())
    }
}

/// A scored criterion with name, weight, and result.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Criterion {
    /// Name of the criterion
    pub name: String,
    /// Description of what is being measured
    pub description: String,
    /// Weight (importance) of this criterion (0.0 - 1.0)
    pub weight: f32,
    /// Score achieved (0.0 - 100.0)
    pub score: f32,
    /// Whether this criterion passed
    pub passed: bool,
    /// Detailed feedback
    pub feedback: Option<String>,
}

impl Criterion {
    /// Create a new criterion.
    #[must_use]
    pub fn new(name: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            weight: 1.0,
            score: 0.0,
            passed: false,
            feedback: None,
        }
    }

    /// Set the weight.
    #[must_use]
    pub fn weight(mut self, weight: f32) -> Self {
        self.weight = weight.clamp(0.0, 1.0);
        self
    }

    /// Set the score.
    #[must_use]
    pub fn score(mut self, score: f32) -> Self {
        self.score = score.clamp(0.0, 100.0);
        self.passed = self.score >= 60.0;
        self
    }

    /// Mark as passed with a perfect score.
    #[must_use]
    pub fn pass(mut self) -> Self {
        self.score = 100.0;
        self.passed = true;
        self
    }

    /// Mark as failed with zero score.
    #[must_use]
    pub fn fail(mut self) -> Self {
        self.score = 0.0;
        self.passed = false;
        self
    }

    /// Add feedback.
    #[must_use]
    pub fn feedback(mut self, feedback: impl Into<String>) -> Self {
        self.feedback = Some(feedback.into());
        self
    }

    /// Get the grade for this criterion.
    #[must_use]
    pub fn grade(&self) -> Grade {
        Grade::from_percentage(self.score)
    }

    /// Get weighted score (score * weight).
    #[must_use]
    pub fn weighted_score(&self) -> f32 {
        self.score * self.weight
    }
}

/// A report card containing multiple criteria scores.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ReportCard {
    /// Title of the evaluation
    pub title: String,
    /// Individual criteria scores
    pub criteria: Vec<Criterion>,
    /// Category scores (aggregated)
    pub categories: HashMap<String, f32>,
}

impl ReportCard {
    /// Create a new report card.
    #[must_use]
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            criteria: Vec::new(),
            categories: HashMap::new(),
        }
    }

    /// Add a criterion.
    pub fn add_criterion(&mut self, criterion: Criterion) {
        self.criteria.push(criterion);
    }

    /// Add criterion with builder pattern.
    #[must_use]
    pub fn criterion(mut self, criterion: Criterion) -> Self {
        self.criteria.push(criterion);
        self
    }

    /// Add a category score.
    pub fn add_category(&mut self, name: impl Into<String>, score: f32) {
        self.categories.insert(name.into(), score.clamp(0.0, 100.0));
    }

    /// Calculate the overall weighted score.
    #[must_use]
    pub fn overall_score(&self) -> f32 {
        if self.criteria.is_empty() {
            return 0.0;
        }

        let total_weight: f32 = self.criteria.iter().map(|c| c.weight).sum();
        if total_weight == 0.0 {
            return 0.0;
        }

        let weighted_sum: f32 = self.criteria.iter().map(Criterion::weighted_score).sum();
        weighted_sum / total_weight
    }

    /// Get the overall grade.
    #[must_use]
    pub fn overall_grade(&self) -> Grade {
        Grade::from_percentage(self.overall_score())
    }

    /// Check if all criteria passed.
    #[must_use]
    pub fn all_passed(&self) -> bool {
        self.criteria.iter().all(|c| c.passed)
    }

    /// Count passed criteria.
    #[must_use]
    pub fn passed_count(&self) -> usize {
        self.criteria.iter().filter(|c| c.passed).count()
    }

    /// Count failed criteria.
    #[must_use]
    pub fn failed_count(&self) -> usize {
        self.criteria.iter().filter(|c| !c.passed).count()
    }

    /// Get all failing criteria.
    #[must_use]
    pub fn failures(&self) -> Vec<&Criterion> {
        self.criteria.iter().filter(|c| !c.passed).collect()
    }

    /// Check if the overall grade is passing.
    #[must_use]
    pub fn is_passing(&self) -> bool {
        self.overall_grade().is_passing()
    }
}

/// Standard evaluation categories.
pub mod categories {
    /// Accessibility evaluation.
    pub const ACCESSIBILITY: &str = "accessibility";
    /// Performance evaluation.
    pub const PERFORMANCE: &str = "performance";
    /// Visual consistency.
    pub const VISUAL: &str = "visual";
    /// Code quality.
    pub const CODE_QUALITY: &str = "code_quality";
    /// Test coverage.
    pub const TESTING: &str = "testing";
    /// Documentation.
    pub const DOCUMENTATION: &str = "documentation";
    /// Security.
    pub const SECURITY: &str = "security";
}

/// Builder for creating standard evaluation criteria.
#[derive(Debug, Clone, Default)]
pub struct EvaluationBuilder {
    report: ReportCard,
}

impl EvaluationBuilder {
    /// Create a new evaluation builder.
    #[must_use]
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            report: ReportCard::new(title),
        }
    }

    /// Add accessibility criterion.
    #[must_use]
    pub fn accessibility(mut self, score: f32, feedback: Option<&str>) -> Self {
        let mut criterion = Criterion::new(
            "Accessibility",
            "WCAG 2.1 AA compliance and screen reader support",
        )
        .weight(1.0)
        .score(score);

        if let Some(fb) = feedback {
            criterion = criterion.feedback(fb);
        }

        self.report.add_criterion(criterion);
        self.report
            .add_category(categories::ACCESSIBILITY.to_string(), score);
        self
    }

    /// Add performance criterion.
    #[must_use]
    pub fn performance(mut self, score: f32, feedback: Option<&str>) -> Self {
        let mut criterion = Criterion::new(
            "Performance",
            "Frame rate, memory usage, and responsiveness",
        )
        .weight(1.0)
        .score(score);

        if let Some(fb) = feedback {
            criterion = criterion.feedback(fb);
        }

        self.report.add_criterion(criterion);
        self.report
            .add_category(categories::PERFORMANCE.to_string(), score);
        self
    }

    /// Add visual criterion.
    #[must_use]
    pub fn visual(mut self, score: f32, feedback: Option<&str>) -> Self {
        let mut criterion =
            Criterion::new("Visual Consistency", "Theme adherence and visual polish")
                .weight(0.8)
                .score(score);

        if let Some(fb) = feedback {
            criterion = criterion.feedback(fb);
        }

        self.report.add_criterion(criterion);
        self.report
            .add_category(categories::VISUAL.to_string(), score);
        self
    }

    /// Add code quality criterion.
    #[must_use]
    pub fn code_quality(mut self, score: f32, feedback: Option<&str>) -> Self {
        let mut criterion = Criterion::new(
            "Code Quality",
            "Lint compliance, documentation, and maintainability",
        )
        .weight(0.8)
        .score(score);

        if let Some(fb) = feedback {
            criterion = criterion.feedback(fb);
        }

        self.report.add_criterion(criterion);
        self.report
            .add_category(categories::CODE_QUALITY.to_string(), score);
        self
    }

    /// Add testing criterion.
    #[must_use]
    pub fn testing(mut self, score: f32, feedback: Option<&str>) -> Self {
        let mut criterion = Criterion::new("Testing", "Test coverage and mutation testing score")
            .weight(1.0)
            .score(score);

        if let Some(fb) = feedback {
            criterion = criterion.feedback(fb);
        }

        self.report.add_criterion(criterion);
        self.report
            .add_category(categories::TESTING.to_string(), score);
        self
    }

    /// Add custom criterion.
    #[must_use]
    pub fn custom(mut self, criterion: Criterion) -> Self {
        self.report.add_criterion(criterion);
        self
    }

    /// Build the final report card.
    #[must_use]
    pub fn build(self) -> ReportCard {
        self.report
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // Grade Tests - TESTS FIRST
    // =========================================================================

    #[test]
    fn test_grade_from_percentage() {
        assert_eq!(Grade::from_percentage(100.0), Grade::A);
        assert_eq!(Grade::from_percentage(95.0), Grade::A);
        assert_eq!(Grade::from_percentage(90.0), Grade::A);
        assert_eq!(Grade::from_percentage(85.0), Grade::B);
        assert_eq!(Grade::from_percentage(80.0), Grade::B);
        assert_eq!(Grade::from_percentage(75.0), Grade::C);
        assert_eq!(Grade::from_percentage(70.0), Grade::C);
        assert_eq!(Grade::from_percentage(65.0), Grade::D);
        assert_eq!(Grade::from_percentage(60.0), Grade::D);
        assert_eq!(Grade::from_percentage(59.0), Grade::F);
        assert_eq!(Grade::from_percentage(0.0), Grade::F);
    }

    #[test]
    fn test_grade_min_percentage() {
        assert_eq!(Grade::A.min_percentage(), 90.0);
        assert_eq!(Grade::B.min_percentage(), 80.0);
        assert_eq!(Grade::C.min_percentage(), 70.0);
        assert_eq!(Grade::D.min_percentage(), 60.0);
        assert_eq!(Grade::F.min_percentage(), 0.0);
    }

    #[test]
    fn test_grade_letter() {
        assert_eq!(Grade::A.letter(), "A");
        assert_eq!(Grade::B.letter(), "B");
        assert_eq!(Grade::C.letter(), "C");
        assert_eq!(Grade::D.letter(), "D");
        assert_eq!(Grade::F.letter(), "F");
    }

    #[test]
    fn test_grade_is_passing() {
        assert!(Grade::A.is_passing());
        assert!(Grade::B.is_passing());
        assert!(Grade::C.is_passing());
        assert!(!Grade::D.is_passing());
        assert!(!Grade::F.is_passing());
    }

    #[test]
    fn test_grade_default() {
        assert_eq!(Grade::default(), Grade::F);
    }

    #[test]
    fn test_grade_display() {
        assert_eq!(format!("{}", Grade::A), "A");
        assert_eq!(format!("{}", Grade::F), "F");
    }

    #[test]
    fn test_grade_ordering() {
        assert!(Grade::A > Grade::B);
        assert!(Grade::B > Grade::C);
        assert!(Grade::C > Grade::D);
        assert!(Grade::D > Grade::F);
    }

    // =========================================================================
    // Criterion Tests - TESTS FIRST
    // =========================================================================

    #[test]
    fn test_criterion_new() {
        let c = Criterion::new("Test", "Description");
        assert_eq!(c.name, "Test");
        assert_eq!(c.description, "Description");
        assert_eq!(c.weight, 1.0);
        assert_eq!(c.score, 0.0);
        assert!(!c.passed);
    }

    #[test]
    fn test_criterion_weight() {
        let c = Criterion::new("Test", "Desc").weight(0.5);
        assert_eq!(c.weight, 0.5);
    }

    #[test]
    fn test_criterion_weight_clamped() {
        let c1 = Criterion::new("Test", "Desc").weight(2.0);
        assert_eq!(c1.weight, 1.0);

        let c2 = Criterion::new("Test", "Desc").weight(-1.0);
        assert_eq!(c2.weight, 0.0);
    }

    #[test]
    fn test_criterion_score() {
        let c = Criterion::new("Test", "Desc").score(85.0);
        assert_eq!(c.score, 85.0);
        assert!(c.passed);
    }

    #[test]
    fn test_criterion_score_failing() {
        let c = Criterion::new("Test", "Desc").score(50.0);
        assert_eq!(c.score, 50.0);
        assert!(!c.passed);
    }

    #[test]
    fn test_criterion_score_clamped() {
        let c1 = Criterion::new("Test", "Desc").score(150.0);
        assert_eq!(c1.score, 100.0);

        let c2 = Criterion::new("Test", "Desc").score(-10.0);
        assert_eq!(c2.score, 0.0);
    }

    #[test]
    fn test_criterion_pass() {
        let c = Criterion::new("Test", "Desc").pass();
        assert_eq!(c.score, 100.0);
        assert!(c.passed);
    }

    #[test]
    fn test_criterion_fail() {
        let c = Criterion::new("Test", "Desc").fail();
        assert_eq!(c.score, 0.0);
        assert!(!c.passed);
    }

    #[test]
    fn test_criterion_feedback() {
        let c = Criterion::new("Test", "Desc").feedback("Good work!");
        assert_eq!(c.feedback, Some("Good work!".to_string()));
    }

    #[test]
    fn test_criterion_grade() {
        assert_eq!(Criterion::new("T", "D").score(95.0).grade(), Grade::A);
        assert_eq!(Criterion::new("T", "D").score(85.0).grade(), Grade::B);
        assert_eq!(Criterion::new("T", "D").score(75.0).grade(), Grade::C);
        assert_eq!(Criterion::new("T", "D").score(65.0).grade(), Grade::D);
        assert_eq!(Criterion::new("T", "D").score(50.0).grade(), Grade::F);
    }

    #[test]
    fn test_criterion_weighted_score() {
        let c = Criterion::new("Test", "Desc").weight(0.5).score(80.0);
        assert_eq!(c.weighted_score(), 40.0);
    }

    // =========================================================================
    // ReportCard Tests - TESTS FIRST
    // =========================================================================

    #[test]
    fn test_report_card_new() {
        let report = ReportCard::new("My Report");
        assert_eq!(report.title, "My Report");
        assert!(report.criteria.is_empty());
    }

    #[test]
    fn test_report_card_add_criterion() {
        let mut report = ReportCard::new("Test");
        report.add_criterion(Criterion::new("C1", "D1").score(90.0));
        assert_eq!(report.criteria.len(), 1);
    }

    #[test]
    fn test_report_card_builder() {
        let report = ReportCard::new("Test")
            .criterion(Criterion::new("C1", "D1").score(90.0))
            .criterion(Criterion::new("C2", "D2").score(80.0));
        assert_eq!(report.criteria.len(), 2);
    }

    #[test]
    fn test_report_card_overall_score_empty() {
        let report = ReportCard::new("Test");
        assert_eq!(report.overall_score(), 0.0);
    }

    #[test]
    fn test_report_card_overall_score_equal_weights() {
        let report = ReportCard::new("Test")
            .criterion(Criterion::new("C1", "D1").weight(1.0).score(100.0))
            .criterion(Criterion::new("C2", "D2").weight(1.0).score(80.0));
        assert_eq!(report.overall_score(), 90.0);
    }

    #[test]
    fn test_report_card_overall_score_different_weights() {
        let report = ReportCard::new("Test")
            .criterion(Criterion::new("C1", "D1").weight(0.75).score(100.0))
            .criterion(Criterion::new("C2", "D2").weight(0.25).score(80.0));
        // (100*0.75 + 80*0.25) / (0.75 + 0.25) = (75 + 20) / 1.0 = 95
        assert_eq!(report.overall_score(), 95.0);
    }

    #[test]
    fn test_report_card_overall_grade() {
        let report = ReportCard::new("Test").criterion(Criterion::new("C1", "D1").score(90.0));
        assert_eq!(report.overall_grade(), Grade::A);
    }

    #[test]
    fn test_report_card_all_passed() {
        let report = ReportCard::new("Test")
            .criterion(Criterion::new("C1", "D1").pass())
            .criterion(Criterion::new("C2", "D2").pass());
        assert!(report.all_passed());
    }

    #[test]
    fn test_report_card_not_all_passed() {
        let report = ReportCard::new("Test")
            .criterion(Criterion::new("C1", "D1").pass())
            .criterion(Criterion::new("C2", "D2").fail());
        assert!(!report.all_passed());
    }

    #[test]
    fn test_report_card_passed_count() {
        let report = ReportCard::new("Test")
            .criterion(Criterion::new("C1", "D1").pass())
            .criterion(Criterion::new("C2", "D2").pass())
            .criterion(Criterion::new("C3", "D3").fail());
        assert_eq!(report.passed_count(), 2);
        assert_eq!(report.failed_count(), 1);
    }

    #[test]
    fn test_report_card_failures() {
        let report = ReportCard::new("Test")
            .criterion(Criterion::new("C1", "D1").pass())
            .criterion(Criterion::new("C2", "D2").fail());
        let failures = report.failures();
        assert_eq!(failures.len(), 1);
        assert_eq!(failures[0].name, "C2");
    }

    #[test]
    fn test_report_card_is_passing() {
        let passing = ReportCard::new("Test").criterion(Criterion::new("C1", "D1").score(90.0));
        assert!(passing.is_passing());

        let failing = ReportCard::new("Test").criterion(Criterion::new("C1", "D1").score(50.0));
        assert!(!failing.is_passing());
    }

    #[test]
    fn test_report_card_add_category() {
        let mut report = ReportCard::new("Test");
        report.add_category("performance", 95.0);
        assert_eq!(report.categories.get("performance"), Some(&95.0));
    }

    // =========================================================================
    // EvaluationBuilder Tests - TESTS FIRST
    // =========================================================================

    #[test]
    fn test_evaluation_builder_new() {
        let builder = EvaluationBuilder::new("My Eval");
        let report = builder.build();
        assert_eq!(report.title, "My Eval");
    }

    #[test]
    fn test_evaluation_builder_accessibility() {
        let report = EvaluationBuilder::new("Test")
            .accessibility(95.0, Some("Good a11y"))
            .build();

        assert_eq!(report.criteria.len(), 1);
        assert_eq!(report.criteria[0].name, "Accessibility");
        assert_eq!(report.criteria[0].score, 95.0);
        assert_eq!(
            report.categories.get(categories::ACCESSIBILITY),
            Some(&95.0)
        );
    }

    #[test]
    fn test_evaluation_builder_performance() {
        let report = EvaluationBuilder::new("Test")
            .performance(88.0, None)
            .build();

        assert_eq!(report.criteria[0].name, "Performance");
        assert_eq!(report.criteria[0].score, 88.0);
    }

    #[test]
    fn test_evaluation_builder_full() {
        let report = EvaluationBuilder::new("Full Evaluation")
            .accessibility(95.0, None)
            .performance(90.0, None)
            .visual(85.0, None)
            .code_quality(92.0, None)
            .testing(98.0, None)
            .build();

        assert_eq!(report.criteria.len(), 5);
        assert!(report.overall_score() > 90.0);
        assert_eq!(report.overall_grade(), Grade::A);
    }

    #[test]
    fn test_evaluation_builder_custom() {
        let report = EvaluationBuilder::new("Test")
            .custom(Criterion::new("Custom", "My custom criterion").score(75.0))
            .build();

        assert_eq!(report.criteria[0].name, "Custom");
    }
}
