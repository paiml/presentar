//! Expression executor for data transformations.

use crate::expression::{AggregateOp, Expression, RankMethod, Transform};
use std::collections::HashMap;

/// A generic value that can hold any data type.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum Value {
    /// Null value
    #[default]
    Null,
    /// Boolean value
    Bool(bool),
    /// Numeric value
    Number(f64),
    /// String value
    String(String),
    /// Array of values
    Array(Vec<Self>),
    /// Object (key-value map)
    Object(HashMap<String, Self>),
}

impl Value {
    /// Create a new null value.
    #[must_use]
    pub const fn null() -> Self {
        Self::Null
    }

    /// Create a new boolean value.
    #[must_use]
    pub const fn bool(v: bool) -> Self {
        Self::Bool(v)
    }

    /// Create a new number value.
    #[must_use]
    pub const fn number(v: f64) -> Self {
        Self::Number(v)
    }

    /// Create a new string value.
    #[must_use]
    pub fn string(v: impl Into<String>) -> Self {
        Self::String(v.into())
    }

    /// Create a new array value.
    #[must_use]
    pub const fn array(v: Vec<Self>) -> Self {
        Self::Array(v)
    }

    /// Create a new object value.
    #[must_use]
    pub const fn object(v: HashMap<String, Self>) -> Self {
        Self::Object(v)
    }

    /// Check if value is null.
    #[must_use]
    pub const fn is_null(&self) -> bool {
        matches!(self, Self::Null)
    }

    /// Check if value is a boolean.
    #[must_use]
    pub const fn is_bool(&self) -> bool {
        matches!(self, Self::Bool(_))
    }

    /// Check if value is a number.
    #[must_use]
    pub const fn is_number(&self) -> bool {
        matches!(self, Self::Number(_))
    }

    /// Check if value is a string.
    #[must_use]
    pub const fn is_string(&self) -> bool {
        matches!(self, Self::String(_))
    }

    /// Check if value is an array.
    #[must_use]
    pub const fn is_array(&self) -> bool {
        matches!(self, Self::Array(_))
    }

    /// Check if value is an object.
    #[must_use]
    pub const fn is_object(&self) -> bool {
        matches!(self, Self::Object(_))
    }

    /// Get as boolean.
    #[must_use]
    pub const fn as_bool(&self) -> Option<bool> {
        match self {
            Self::Bool(v) => Some(*v),
            _ => None,
        }
    }

    /// Get as number.
    #[must_use]
    pub const fn as_number(&self) -> Option<f64> {
        match self {
            Self::Number(v) => Some(*v),
            _ => None,
        }
    }

    /// Get as string.
    #[must_use]
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::String(v) => Some(v),
            _ => None,
        }
    }

    /// Get as array.
    #[must_use]
    pub const fn as_array(&self) -> Option<&Vec<Self>> {
        match self {
            Self::Array(v) => Some(v),
            _ => None,
        }
    }

    /// Get as mutable array.
    #[must_use]
    pub fn as_array_mut(&mut self) -> Option<&mut Vec<Self>> {
        match self {
            Self::Array(v) => Some(v),
            _ => None,
        }
    }

    /// Get as object.
    #[must_use]
    pub const fn as_object(&self) -> Option<&HashMap<String, Self>> {
        match self {
            Self::Object(v) => Some(v),
            _ => None,
        }
    }

    /// Get field from object.
    #[must_use]
    pub fn get(&self, key: &str) -> Option<&Self> {
        match self {
            Self::Object(map) => map.get(key),
            _ => None,
        }
    }

    /// Get array length or object key count.
    #[must_use]
    pub fn len(&self) -> usize {
        match self {
            Self::Array(arr) => arr.len(),
            Self::Object(obj) => obj.len(),
            Self::String(s) => s.len(),
            _ => 0,
        }
    }

    /// Check if array or object is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl From<bool> for Value {
    fn from(v: bool) -> Self {
        Self::Bool(v)
    }
}

impl From<f64> for Value {
    fn from(v: f64) -> Self {
        Self::Number(v)
    }
}

impl From<i32> for Value {
    fn from(v: i32) -> Self {
        Self::Number(f64::from(v))
    }
}

impl From<i64> for Value {
    fn from(v: i64) -> Self {
        Self::Number(v as f64)
    }
}

impl From<&str> for Value {
    fn from(v: &str) -> Self {
        Self::String(v.to_string())
    }
}

impl From<String> for Value {
    fn from(v: String) -> Self {
        Self::String(v)
    }
}

impl<T: Into<Self>> From<Vec<T>> for Value {
    fn from(v: Vec<T>) -> Self {
        Self::Array(v.into_iter().map(Into::into).collect())
    }
}

/// Execution error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExecutionError {
    /// Source not found in data context
    SourceNotFound(String),
    /// Expected an array for this transform
    ExpectedArray,
    /// Expected an object
    ExpectedObject,
    /// Field not found
    FieldNotFound(String),
    /// Type mismatch
    TypeMismatch(String),
    /// Invalid transform
    InvalidTransform(String),
}

impl std::fmt::Display for ExecutionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SourceNotFound(name) => write!(f, "source not found: {name}"),
            Self::ExpectedArray => write!(f, "expected an array"),
            Self::ExpectedObject => write!(f, "expected an object"),
            Self::FieldNotFound(name) => write!(f, "field not found: {name}"),
            Self::TypeMismatch(msg) => write!(f, "type mismatch: {msg}"),
            Self::InvalidTransform(msg) => write!(f, "invalid transform: {msg}"),
        }
    }
}

impl std::error::Error for ExecutionError {}

/// Data context for expression execution.
#[derive(Debug, Clone, Default)]
pub struct DataContext {
    /// Named data sources
    sources: HashMap<String, Value>,
}

impl DataContext {
    /// Create a new empty context.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a data source.
    pub fn insert(&mut self, name: impl Into<String>, value: Value) {
        self.sources.insert(name.into(), value);
    }

    /// Get a data source.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&Value> {
        // Support dotted paths like "data.transactions"
        let parts: Vec<&str> = name.split('.').collect();
        let mut current = self.sources.get(parts[0])?;

        for part in &parts[1..] {
            current = current.get(part)?;
        }

        Some(current)
    }

    /// Check if context has a source.
    #[must_use]
    pub fn contains(&self, name: &str) -> bool {
        self.get(name).is_some()
    }
}

/// Expression executor.
#[derive(Debug, Default)]
pub struct ExpressionExecutor;

impl ExpressionExecutor {
    /// Create a new executor.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    /// Execute an expression against a data context.
    ///
    /// # Errors
    ///
    /// Returns an error if execution fails.
    pub fn execute(&self, expr: &Expression, ctx: &DataContext) -> Result<Value, ExecutionError> {
        // Resolve source
        let mut value = ctx
            .get(&expr.source)
            .cloned()
            .ok_or_else(|| ExecutionError::SourceNotFound(expr.source.clone()))?;

        // Apply transforms
        for transform in &expr.transforms {
            value = self.apply_transform(&value, transform, ctx)?;
        }

        Ok(value)
    }

    fn apply_transform(
        &self,
        value: &Value,
        transform: &Transform,
        ctx: &DataContext,
    ) -> Result<Value, ExecutionError> {
        match transform {
            Transform::Filter {
                field,
                value: match_value,
            } => self.apply_filter(value, field, match_value),
            Transform::Select { fields } => self.apply_select(value, fields),
            Transform::Sort { field, desc } => self.apply_sort(value, field, *desc),
            Transform::Count => Ok(self.apply_count(value)),
            Transform::Sum { field } => self.apply_sum(value, field),
            Transform::Mean { field } => self.apply_mean(value, field),
            Transform::Sample { n } => self.apply_sample(value, *n),
            Transform::Percentage => self.apply_percentage(value),
            Transform::Rate { window } => self.apply_rate(value, window),
            Transform::Join { other, on } => self.apply_join(value, other, on, ctx),
            Transform::GroupBy { field } => self.apply_group_by(value, field),
            Transform::Distinct { field } => self.apply_distinct(value, field.as_deref()),
            Transform::Where {
                field,
                op,
                value: match_value,
            } => self.apply_where(value, field, op, match_value),
            Transform::Offset { n } => self.apply_offset(value, *n),
            Transform::Min { field } => self.apply_min(value, field),
            Transform::Max { field } => self.apply_max(value, field),
            Transform::First { n } | Transform::Limit { n } => self.apply_limit(value, *n),
            Transform::Last { n } => self.apply_last(value, *n),
            Transform::Flatten => self.apply_flatten(value),
            Transform::Reverse => self.apply_reverse(value),
            // New transforms
            Transform::Map { expr } => self.apply_map(value, expr),
            Transform::Reduce { initial, expr } => self.apply_reduce(value, initial, expr),
            Transform::Aggregate { field, op } => self.apply_aggregate(value, field, *op),
            Transform::Pivot {
                row_field,
                col_field,
                value_field,
            } => self.apply_pivot(value, row_field, col_field, value_field),
            Transform::CumulativeSum { field } => self.apply_cumsum(value, field),
            Transform::Rank { field, method } => self.apply_rank(value, field, *method),
            Transform::MovingAverage { field, window } => {
                self.apply_moving_avg(value, field, *window)
            }
            Transform::PercentChange { field } => self.apply_pct_change(value, field),
            Transform::Suggest { prefix, count } => self.apply_suggest(value, prefix, *count),
        }
    }

    fn apply_filter(
        &self,
        value: &Value,
        field: &str,
        match_value: &str,
    ) -> Result<Value, ExecutionError> {
        let arr = value.as_array().ok_or(ExecutionError::ExpectedArray)?;

        let filtered: Vec<Value> = arr
            .iter()
            .filter(|item| {
                if let Some(obj) = item.as_object() {
                    if let Some(val) = obj.get(field) {
                        return self.value_matches(val, match_value);
                    }
                }
                false
            })
            .cloned()
            .collect();

        Ok(Value::Array(filtered))
    }

    fn value_matches(&self, value: &Value, target: &str) -> bool {
        match value {
            Value::String(s) => s == target,
            Value::Number(n) => {
                if let Ok(t) = target.parse::<f64>() {
                    (*n - t).abs() < f64::EPSILON
                } else {
                    false
                }
            }
            Value::Bool(b) => {
                matches!((b, target), (true, "true") | (false, "false"))
            }
            _ => false,
        }
    }

    fn apply_select(&self, value: &Value, fields: &[String]) -> Result<Value, ExecutionError> {
        let arr = value.as_array().ok_or(ExecutionError::ExpectedArray)?;

        let selected: Vec<Value> = arr
            .iter()
            .map(|item| {
                if let Some(obj) = item.as_object() {
                    let mut new_obj = HashMap::new();
                    for field in fields {
                        if let Some(val) = obj.get(field) {
                            new_obj.insert(field.clone(), val.clone());
                        }
                    }
                    Value::Object(new_obj)
                } else {
                    item.clone()
                }
            })
            .collect();

        Ok(Value::Array(selected))
    }

    fn apply_sort(&self, value: &Value, field: &str, desc: bool) -> Result<Value, ExecutionError> {
        let arr = value.as_array().ok_or(ExecutionError::ExpectedArray)?;
        let mut sorted = arr.clone();

        sorted.sort_by(|a, b| {
            let a_val = a.get(field);
            let b_val = b.get(field);

            let cmp = match (a_val, b_val) {
                (Some(Value::Number(a)), Some(Value::Number(b))) => {
                    a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal)
                }
                (Some(Value::String(a)), Some(Value::String(b))) => a.cmp(b),
                _ => std::cmp::Ordering::Equal,
            };

            if desc {
                cmp.reverse()
            } else {
                cmp
            }
        });

        Ok(Value::Array(sorted))
    }

    fn apply_limit(&self, value: &Value, n: usize) -> Result<Value, ExecutionError> {
        let arr = value.as_array().ok_or(ExecutionError::ExpectedArray)?;
        Ok(Value::Array(arr.iter().take(n).cloned().collect()))
    }

    fn apply_count(&self, value: &Value) -> Value {
        match value {
            Value::Array(arr) => Value::Number(arr.len() as f64),
            Value::Object(obj) => Value::Number(obj.len() as f64),
            Value::String(s) => Value::Number(s.len() as f64),
            _ => Value::Number(0.0),
        }
    }

    fn apply_sum(&self, value: &Value, field: &str) -> Result<Value, ExecutionError> {
        let arr = value.as_array().ok_or(ExecutionError::ExpectedArray)?;

        let sum: f64 = arr
            .iter()
            .filter_map(|item| item.get(field)?.as_number())
            .sum();

        Ok(Value::Number(sum))
    }

    fn apply_mean(&self, value: &Value, field: &str) -> Result<Value, ExecutionError> {
        let arr = value.as_array().ok_or(ExecutionError::ExpectedArray)?;

        let values: Vec<f64> = arr
            .iter()
            .filter_map(|item| item.get(field)?.as_number())
            .collect();

        if values.is_empty() {
            return Ok(Value::Number(0.0));
        }

        let sum: f64 = values.iter().sum();
        let mean = sum / values.len() as f64;

        Ok(Value::Number(mean))
    }

    fn apply_sample(&self, value: &Value, n: usize) -> Result<Value, ExecutionError> {
        let arr = value.as_array().ok_or(ExecutionError::ExpectedArray)?;

        // Simple deterministic "sampling" - just take first n elements
        // Real implementation would use random sampling
        Ok(Value::Array(arr.iter().take(n).cloned().collect()))
    }

    fn apply_percentage(&self, value: &Value) -> Result<Value, ExecutionError> {
        match value {
            Value::Number(n) => Ok(Value::Number(n * 100.0)),
            _ => Err(ExecutionError::TypeMismatch(
                "percentage requires a number".to_string(),
            )),
        }
    }

    fn apply_rate(&self, value: &Value, window: &str) -> Result<Value, ExecutionError> {
        let arr = value.as_array().ok_or(ExecutionError::ExpectedArray)?;

        // Parse window (e.g., "1m", "5m", "1h")
        let window_ms = self.parse_window(window)?;

        // For rate calculation, we need timestamped data
        // Look for a "timestamp" or "time" field
        let mut values_with_time: Vec<(f64, f64)> = arr
            .iter()
            .filter_map(|item| {
                let obj = item.as_object()?;
                let time = obj
                    .get("timestamp")
                    .or_else(|| obj.get("time"))
                    .and_then(Value::as_number)?;
                let val = obj
                    .get("value")
                    .or_else(|| obj.get("count"))
                    .and_then(Value::as_number)
                    .unwrap_or(1.0);
                Some((time, val))
            })
            .collect();

        if values_with_time.len() < 2 {
            return Ok(Value::Number(0.0));
        }

        // Sort by time
        values_with_time.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));

        // Calculate rate over the window
        let window_ms_f64 = window_ms as f64;
        let last_time = values_with_time.last().map_or(0.0, |v| v.0);
        let window_start = last_time - window_ms_f64;

        let sum_in_window: f64 = values_with_time
            .iter()
            .filter(|(t, _)| *t >= window_start)
            .map(|(_, v)| v)
            .sum();

        // Rate per second
        let rate = sum_in_window / (window_ms_f64 / 1000.0);

        Ok(Value::Number(rate))
    }

    fn parse_window(&self, window: &str) -> Result<u64, ExecutionError> {
        let window = window.trim();
        if window.is_empty() {
            return Err(ExecutionError::InvalidTransform("empty window".to_string()));
        }

        let (num_str, unit) = if let Some(s) = window.strip_suffix("ms") {
            (s, "ms")
        } else if let Some(s) = window.strip_suffix('s') {
            (s, "s")
        } else if let Some(s) = window.strip_suffix('m') {
            (s, "m")
        } else if let Some(s) = window.strip_suffix('h') {
            (s, "h")
        } else if let Some(s) = window.strip_suffix('d') {
            (s, "d")
        } else {
            // Assume milliseconds if no unit
            (window, "ms")
        };

        let num: u64 = num_str
            .parse()
            .map_err(|_| ExecutionError::InvalidTransform(format!("invalid window: {window}")))?;

        let ms = match unit {
            "s" => num * 1000,
            "m" => num * 60 * 1000,
            "h" => num * 60 * 60 * 1000,
            "d" => num * 24 * 60 * 60 * 1000,
            // "ms" and any other unit default to num (milliseconds)
            _ => num,
        };

        Ok(ms)
    }

    fn apply_join(
        &self,
        value: &Value,
        other: &str,
        on: &str,
        ctx: &DataContext,
    ) -> Result<Value, ExecutionError> {
        let left_arr = value.as_array().ok_or(ExecutionError::ExpectedArray)?;

        // Get the other dataset from context
        let right_value = ctx
            .get(other)
            .ok_or_else(|| ExecutionError::SourceNotFound(other.to_string()))?;
        let right_arr = right_value
            .as_array()
            .ok_or(ExecutionError::ExpectedArray)?;

        // Build a lookup map for the right side (keyed by the join field)
        let mut right_lookup: HashMap<String, Vec<&Value>> = HashMap::new();
        for item in right_arr {
            if let Some(obj) = item.as_object() {
                if let Some(key_val) = obj.get(on) {
                    let key = self.value_to_string(key_val);
                    right_lookup.entry(key).or_default().push(item);
                }
            }
        }

        // Perform the join (left join - keeps all left items)
        let mut result = Vec::new();
        for left_item in left_arr {
            if let Some(left_obj) = left_item.as_object() {
                if let Some(key_val) = left_obj.get(on) {
                    let key = self.value_to_string(key_val);
                    if let Some(right_items) = right_lookup.get(&key) {
                        // Join with each matching right item
                        for right_item in right_items {
                            if let Some(right_obj) = right_item.as_object() {
                                // Merge left and right objects
                                let mut merged = left_obj.clone();
                                for (k, v) in right_obj {
                                    // Don't overwrite left values, prefix with source name
                                    if merged.contains_key(k) && k != on {
                                        merged.insert(format!("{other}_{k}"), v.clone());
                                    } else if k != on {
                                        merged.insert(k.clone(), v.clone());
                                    }
                                }
                                result.push(Value::Object(merged));
                            }
                        }
                    } else {
                        // No match, keep left item as-is (left join behavior)
                        result.push(left_item.clone());
                    }
                } else {
                    // No join key, keep as-is
                    result.push(left_item.clone());
                }
            } else {
                // Not an object, keep as-is
                result.push(left_item.clone());
            }
        }

        Ok(Value::Array(result))
    }

    fn apply_group_by(&self, value: &Value, field: &str) -> Result<Value, ExecutionError> {
        let arr = value.as_array().ok_or(ExecutionError::ExpectedArray)?;

        let mut groups: HashMap<String, Vec<Value>> = HashMap::new();

        for item in arr {
            let key = if let Some(obj) = item.as_object() {
                if let Some(val) = obj.get(field) {
                    self.value_to_string(val)
                } else {
                    "_null".to_string()
                }
            } else {
                "_null".to_string()
            };

            groups.entry(key).or_default().push(item.clone());
        }

        // Convert to array of objects with key and items
        let result: Vec<Value> = groups
            .into_iter()
            .map(|(key, items)| {
                let mut obj = HashMap::new();
                obj.insert("key".to_string(), Value::String(key));
                obj.insert("items".to_string(), Value::Array(items.clone()));
                obj.insert("count".to_string(), Value::Number(items.len() as f64));
                Value::Object(obj)
            })
            .collect();

        Ok(Value::Array(result))
    }

    fn value_to_string(&self, value: &Value) -> String {
        match value {
            Value::Null => "_null".to_string(),
            Value::Bool(b) => b.to_string(),
            Value::Number(n) => n.to_string(),
            Value::String(s) => s.clone(),
            Value::Array(_) => "_array".to_string(),
            Value::Object(_) => "_object".to_string(),
        }
    }

    fn apply_distinct(&self, value: &Value, field: Option<&str>) -> Result<Value, ExecutionError> {
        let arr = value.as_array().ok_or(ExecutionError::ExpectedArray)?;

        let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
        let mut result = Vec::new();

        for item in arr {
            let key = if let Some(f) = field {
                if let Some(obj) = item.as_object() {
                    obj.get(f)
                        .map(|v| self.value_to_string(v))
                        .unwrap_or_default()
                } else {
                    self.value_to_string(item)
                }
            } else {
                self.value_to_string(item)
            };

            if seen.insert(key) {
                result.push(item.clone());
            }
        }

        Ok(Value::Array(result))
    }

    fn apply_where(
        &self,
        value: &Value,
        field: &str,
        op: &str,
        match_value: &str,
    ) -> Result<Value, ExecutionError> {
        let arr = value.as_array().ok_or(ExecutionError::ExpectedArray)?;

        let filtered: Vec<Value> = arr
            .iter()
            .filter(|item| {
                if let Some(obj) = item.as_object() {
                    if let Some(val) = obj.get(field) {
                        return self.compare_values(val, op, match_value);
                    }
                }
                false
            })
            .cloned()
            .collect();

        Ok(Value::Array(filtered))
    }

    fn compare_values(&self, value: &Value, op: &str, target: &str) -> bool {
        match op {
            "eq" | "==" | "=" => self.value_matches(value, target),
            "ne" | "!=" | "<>" => !self.value_matches(value, target),
            "gt" | ">" => {
                if let (Some(v), Ok(t)) = (value.as_number(), target.parse::<f64>()) {
                    v > t
                } else {
                    false
                }
            }
            "lt" | "<" => {
                if let (Some(v), Ok(t)) = (value.as_number(), target.parse::<f64>()) {
                    v < t
                } else {
                    false
                }
            }
            "gte" | ">=" => {
                if let (Some(v), Ok(t)) = (value.as_number(), target.parse::<f64>()) {
                    v >= t
                } else {
                    false
                }
            }
            "lte" | "<=" => {
                if let (Some(v), Ok(t)) = (value.as_number(), target.parse::<f64>()) {
                    v <= t
                } else {
                    false
                }
            }
            "contains" => {
                if let Some(s) = value.as_str() {
                    s.contains(target)
                } else {
                    false
                }
            }
            "starts_with" => {
                if let Some(s) = value.as_str() {
                    s.starts_with(target)
                } else {
                    false
                }
            }
            "ends_with" => {
                if let Some(s) = value.as_str() {
                    s.ends_with(target)
                } else {
                    false
                }
            }
            _ => false,
        }
    }

    fn apply_offset(&self, value: &Value, n: usize) -> Result<Value, ExecutionError> {
        let arr = value.as_array().ok_or(ExecutionError::ExpectedArray)?;
        Ok(Value::Array(arr.iter().skip(n).cloned().collect()))
    }

    fn apply_min(&self, value: &Value, field: &str) -> Result<Value, ExecutionError> {
        let arr = value.as_array().ok_or(ExecutionError::ExpectedArray)?;

        let min = arr
            .iter()
            .filter_map(|item| item.get(field)?.as_number())
            .fold(f64::INFINITY, f64::min);

        if min.is_infinite() {
            Ok(Value::Null)
        } else {
            Ok(Value::Number(min))
        }
    }

    fn apply_max(&self, value: &Value, field: &str) -> Result<Value, ExecutionError> {
        let arr = value.as_array().ok_or(ExecutionError::ExpectedArray)?;

        let max = arr
            .iter()
            .filter_map(|item| item.get(field)?.as_number())
            .fold(f64::NEG_INFINITY, f64::max);

        if max.is_infinite() {
            Ok(Value::Null)
        } else {
            Ok(Value::Number(max))
        }
    }

    fn apply_last(&self, value: &Value, n: usize) -> Result<Value, ExecutionError> {
        let arr = value.as_array().ok_or(ExecutionError::ExpectedArray)?;
        let len = arr.len();
        let skip = len.saturating_sub(n);
        Ok(Value::Array(arr.iter().skip(skip).cloned().collect()))
    }

    fn apply_flatten(&self, value: &Value) -> Result<Value, ExecutionError> {
        let arr = value.as_array().ok_or(ExecutionError::ExpectedArray)?;

        let mut result = Vec::new();
        for item in arr {
            if let Some(inner) = item.as_array() {
                result.extend(inner.iter().cloned());
            } else {
                result.push(item.clone());
            }
        }

        Ok(Value::Array(result))
    }

    fn apply_reverse(&self, value: &Value) -> Result<Value, ExecutionError> {
        let arr = value.as_array().ok_or(ExecutionError::ExpectedArray)?;
        let mut reversed = arr.clone();
        reversed.reverse();
        Ok(Value::Array(reversed))
    }

    // =========================================================================
    // New Transform Implementations
    // =========================================================================

    fn apply_map(&self, value: &Value, expr: &str) -> Result<Value, ExecutionError> {
        let arr = value.as_array().ok_or(ExecutionError::ExpectedArray)?;

        // Simple expression evaluation: extract field if expr is "item.field"
        // For complex expressions, this would need a proper expression evaluator
        let mapped: Vec<Value> = arr
            .iter()
            .map(|item| {
                // Handle simple field access like "item.field"
                if let Some(field) = expr.strip_prefix("item.") {
                    if let Some(obj) = item.as_object() {
                        obj.get(field).cloned().unwrap_or(Value::Null)
                    } else {
                        item.clone()
                    }
                } else {
                    // Return item unchanged if we can't parse the expression
                    item.clone()
                }
            })
            .collect();

        Ok(Value::Array(mapped))
    }

    fn apply_reduce(
        &self,
        value: &Value,
        initial: &str,
        _expr: &str,
    ) -> Result<Value, ExecutionError> {
        let arr = value.as_array().ok_or(ExecutionError::ExpectedArray)?;

        // Parse initial value
        let mut acc: f64 = initial.parse().unwrap_or(0.0);

        // Simple sum reduction (a proper implementation would evaluate the expr)
        for item in arr {
            if let Some(n) = item.as_number() {
                acc += n;
            }
        }

        Ok(Value::Number(acc))
    }

    fn apply_aggregate(
        &self,
        value: &Value,
        field: &str,
        op: AggregateOp,
    ) -> Result<Value, ExecutionError> {
        let arr = value.as_array().ok_or(ExecutionError::ExpectedArray)?;

        // For grouped data, expect array of {key: ..., values: [...]}
        // For ungrouped data, operate on the field directly

        let values: Vec<f64> = arr
            .iter()
            .filter_map(|item| {
                if let Some(obj) = item.as_object() {
                    // If this is a group with "values" key
                    if let Some(Value::Array(group_values)) = obj.get("values") {
                        return Some(
                            group_values
                                .iter()
                                .filter_map(|v| v.get(field)?.as_number())
                                .collect::<Vec<_>>(),
                        );
                    }
                    // Direct field access
                    obj.get(field)?.as_number().map(|n| vec![n])
                } else {
                    None
                }
            })
            .flatten()
            .collect();

        let result = match op {
            AggregateOp::Sum => values.iter().sum(),
            AggregateOp::Count => values.len() as f64,
            AggregateOp::Mean => {
                if values.is_empty() {
                    0.0
                } else {
                    values.iter().sum::<f64>() / values.len() as f64
                }
            }
            AggregateOp::Min => values.iter().cloned().fold(f64::INFINITY, f64::min),
            AggregateOp::Max => values.iter().cloned().fold(f64::NEG_INFINITY, f64::max),
            AggregateOp::First => values.first().copied().unwrap_or(0.0),
            AggregateOp::Last => values.last().copied().unwrap_or(0.0),
        };

        Ok(Value::Number(result))
    }

    fn apply_pivot(
        &self,
        value: &Value,
        row_field: &str,
        col_field: &str,
        value_field: &str,
    ) -> Result<Value, ExecutionError> {
        let arr = value.as_array().ok_or(ExecutionError::ExpectedArray)?;

        // Build pivot table
        let mut rows: HashMap<String, HashMap<String, f64>> = HashMap::new();

        for item in arr {
            if let Some(obj) = item.as_object() {
                let row_key = obj
                    .get(row_field)
                    .map(|v| self.value_to_string(v))
                    .unwrap_or_default();
                let col_key = obj
                    .get(col_field)
                    .map(|v| self.value_to_string(v))
                    .unwrap_or_default();
                let val = obj
                    .get(value_field)
                    .and_then(|v| v.as_number())
                    .unwrap_or(0.0);

                rows.entry(row_key)
                    .or_default()
                    .entry(col_key)
                    .and_modify(|v| *v += val)
                    .or_insert(val);
            }
        }

        // Convert to array of objects
        let result: Vec<Value> = rows
            .into_iter()
            .map(|(row_key, cols)| {
                let mut obj = HashMap::new();
                obj.insert(row_field.to_string(), Value::String(row_key));
                for (col_key, val) in cols {
                    obj.insert(col_key, Value::Number(val));
                }
                Value::Object(obj)
            })
            .collect();

        Ok(Value::Array(result))
    }

    fn apply_cumsum(&self, value: &Value, field: &str) -> Result<Value, ExecutionError> {
        let arr = value.as_array().ok_or(ExecutionError::ExpectedArray)?;

        let mut running_sum = 0.0;
        let result: Vec<Value> = arr
            .iter()
            .map(|item| {
                if let Some(obj) = item.as_object() {
                    let val = obj.get(field).and_then(|v| v.as_number()).unwrap_or(0.0);
                    running_sum += val;

                    let mut new_obj = obj.clone();
                    new_obj.insert(format!("{field}_cumsum"), Value::Number(running_sum));
                    Value::Object(new_obj)
                } else {
                    item.clone()
                }
            })
            .collect();

        Ok(Value::Array(result))
    }

    fn apply_rank(
        &self,
        value: &Value,
        field: &str,
        method: RankMethod,
    ) -> Result<Value, ExecutionError> {
        let arr = value.as_array().ok_or(ExecutionError::ExpectedArray)?;

        // Extract values with indices
        let mut indexed: Vec<(usize, f64)> = arr
            .iter()
            .enumerate()
            .filter_map(|(i, item)| item.as_object()?.get(field)?.as_number().map(|n| (i, n)))
            .collect();

        // Sort by value (descending for ranking)
        indexed.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        // Assign ranks based on method
        let mut ranks = vec![0.0; arr.len()];
        match method {
            RankMethod::Dense => {
                let mut rank = 0;
                let mut prev_val: Option<f64> = None;
                for (i, val) in indexed {
                    if prev_val != Some(val) {
                        rank += 1;
                    }
                    ranks[i] = rank as f64;
                    prev_val = Some(val);
                }
            }
            RankMethod::Ordinal => {
                for (rank, (i, _)) in indexed.iter().enumerate() {
                    ranks[*i] = (rank + 1) as f64;
                }
            }
            RankMethod::Average => {
                let mut i = 0;
                while i < indexed.len() {
                    let val = indexed[i].1;
                    let start = i;
                    while i < indexed.len() && (indexed[i].1 - val).abs() < f64::EPSILON {
                        i += 1;
                    }
                    let avg_rank =
                        (start + 1..=i).map(|r| r as f64).sum::<f64>() / (i - start) as f64;
                    for j in start..i {
                        ranks[indexed[j].0] = avg_rank;
                    }
                }
            }
        }

        // Add rank to each object
        let result: Vec<Value> = arr
            .iter()
            .enumerate()
            .map(|(i, item)| {
                if let Some(obj) = item.as_object() {
                    let mut new_obj = obj.clone();
                    new_obj.insert(format!("{field}_rank"), Value::Number(ranks[i]));
                    Value::Object(new_obj)
                } else {
                    item.clone()
                }
            })
            .collect();

        Ok(Value::Array(result))
    }

    fn apply_moving_avg(
        &self,
        value: &Value,
        field: &str,
        window: usize,
    ) -> Result<Value, ExecutionError> {
        let arr = value.as_array().ok_or(ExecutionError::ExpectedArray)?;

        let values: Vec<f64> = arr
            .iter()
            .filter_map(|item| item.as_object()?.get(field)?.as_number())
            .collect();

        let result: Vec<Value> = arr
            .iter()
            .enumerate()
            .map(|(i, item)| {
                if let Some(obj) = item.as_object() {
                    let start = i.saturating_sub(window - 1);
                    let window_values = &values[start..=i.min(values.len() - 1)];
                    let ma = if window_values.is_empty() {
                        0.0
                    } else {
                        window_values.iter().sum::<f64>() / window_values.len() as f64
                    };

                    let mut new_obj = obj.clone();
                    new_obj.insert(format!("{field}_ma{window}"), Value::Number(ma));
                    Value::Object(new_obj)
                } else {
                    item.clone()
                }
            })
            .collect();

        Ok(Value::Array(result))
    }

    fn apply_pct_change(&self, value: &Value, field: &str) -> Result<Value, ExecutionError> {
        let arr = value.as_array().ok_or(ExecutionError::ExpectedArray)?;

        let values: Vec<f64> = arr
            .iter()
            .filter_map(|item| item.as_object()?.get(field)?.as_number())
            .collect();

        let result: Vec<Value> = arr
            .iter()
            .enumerate()
            .map(|(i, item)| {
                if let Some(obj) = item.as_object() {
                    let pct = if i == 0 || values.get(i - 1).map_or(true, |&prev| prev == 0.0) {
                        0.0
                    } else {
                        let prev = values[i - 1];
                        let curr = values.get(i).copied().unwrap_or(prev);
                        (curr - prev) / prev * 100.0
                    };

                    let mut new_obj = obj.clone();
                    new_obj.insert(format!("{field}_pct_change"), Value::Number(pct));
                    Value::Object(new_obj)
                } else {
                    item.clone()
                }
            })
            .collect();

        Ok(Value::Array(result))
    }

    /// Apply suggestion transform for N-gram/autocomplete models.
    ///
    /// The input `value` should be a model object with a `model_type` field.
    /// This is a stub implementation - actual model inference is handled by
    /// the runtime layer which injects results into the context.
    ///
    /// In production, the runtime loads the .apr model and provides suggestions
    /// through a callback or pre-computed context value.
    #[allow(clippy::unnecessary_wraps)] // Returns Result for API consistency with other transforms
    fn apply_suggest(
        &self,
        value: &Value,
        prefix: &str,
        count: usize,
    ) -> Result<Value, ExecutionError> {
        // Check if value is a model object with pre-computed suggestions
        if let Some(obj) = value.as_object() {
            // If the model has pre-computed suggestions for this prefix, use them
            if let Some(suggestions) = obj.get("_suggestions") {
                if let Some(arr) = suggestions.as_array() {
                    return Ok(Value::Array(arr.iter().take(count).cloned().collect()));
                }
            }

            // If this is a model reference, return placeholder suggestions
            // The runtime layer should populate _suggestions before execution
            if obj.contains_key("model_type") || obj.contains_key("source") {
                // Return empty array - runtime should inject actual suggestions
                return Ok(Value::Array(vec![]));
            }
        }

        // For testing/demo: if value is an array of suggestion objects, filter by prefix
        if let Some(arr) = value.as_array() {
            let filtered: Vec<Value> = arr
                .iter()
                .filter(|item| {
                    if let Some(obj) = item.as_object() {
                        if let Some(text) = obj.get("text").and_then(|v| v.as_str()) {
                            return text.starts_with(prefix);
                        }
                    }
                    false
                })
                .take(count)
                .cloned()
                .collect();
            return Ok(Value::Array(filtered));
        }

        // Fallback: return empty suggestions
        Ok(Value::Array(vec![]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::expression::ExpressionParser;

    // ===== Value Tests =====

    #[test]
    fn test_value_null() {
        let v = Value::null();
        assert!(v.is_null());
        assert!(!v.is_bool());
    }

    #[test]
    fn test_value_bool() {
        let v = Value::bool(true);
        assert!(v.is_bool());
        assert_eq!(v.as_bool(), Some(true));
    }

    #[test]
    fn test_value_number() {
        let v = Value::number(42.5);
        assert!(v.is_number());
        assert_eq!(v.as_number(), Some(42.5));
    }

    #[test]
    fn test_value_string() {
        let v = Value::string("hello");
        assert!(v.is_string());
        assert_eq!(v.as_str(), Some("hello"));
    }

    #[test]
    fn test_value_array() {
        let v = Value::array(vec![Value::number(1.0), Value::number(2.0)]);
        assert!(v.is_array());
        assert_eq!(v.len(), 2);
    }

    #[test]
    fn test_value_object() {
        let mut map = HashMap::new();
        map.insert("name".to_string(), Value::string("test"));
        let v = Value::object(map);
        assert!(v.is_object());
        assert_eq!(v.get("name").unwrap().as_str(), Some("test"));
    }

    #[test]
    fn test_value_from_bool() {
        let v: Value = true.into();
        assert_eq!(v, Value::Bool(true));
    }

    #[test]
    fn test_value_from_number() {
        let v: Value = 42.0f64.into();
        assert_eq!(v, Value::Number(42.0));
    }

    #[test]
    fn test_value_from_i32() {
        let v: Value = 42i32.into();
        assert_eq!(v, Value::Number(42.0));
    }

    #[test]
    fn test_value_from_str() {
        let v: Value = "hello".into();
        assert_eq!(v, Value::String("hello".to_string()));
    }

    #[test]
    fn test_value_default() {
        assert_eq!(Value::default(), Value::Null);
    }

    #[test]
    fn test_value_is_empty() {
        assert!(Value::array(vec![]).is_empty());
        assert!(!Value::array(vec![Value::Null]).is_empty());
    }

    // ===== DataContext Tests =====

    #[test]
    fn test_context_new() {
        let ctx = DataContext::new();
        assert!(!ctx.contains("foo"));
    }

    #[test]
    fn test_context_insert_get() {
        let mut ctx = DataContext::new();
        ctx.insert("users", Value::array(vec![]));
        assert!(ctx.contains("users"));
    }

    #[test]
    fn test_context_dotted_path() {
        let mut ctx = DataContext::new();
        let mut data = HashMap::new();
        data.insert("transactions".to_string(), Value::array(vec![]));
        ctx.insert("data", Value::object(data));

        assert!(ctx.contains("data.transactions"));
    }

    // ===== ExecutionError Tests =====

    #[test]
    fn test_error_display() {
        assert_eq!(
            ExecutionError::SourceNotFound("foo".to_string()).to_string(),
            "source not found: foo"
        );
        assert_eq!(
            ExecutionError::ExpectedArray.to_string(),
            "expected an array"
        );
    }

    // ===== Executor Tests =====

    fn make_test_data() -> DataContext {
        let mut ctx = DataContext::new();

        // Create test transactions
        let transactions: Vec<Value> = vec![
            {
                let mut t = HashMap::new();
                t.insert("id".to_string(), Value::number(1.0));
                t.insert("status".to_string(), Value::string("completed"));
                t.insert("amount".to_string(), Value::number(100.0));
                Value::Object(t)
            },
            {
                let mut t = HashMap::new();
                t.insert("id".to_string(), Value::number(2.0));
                t.insert("status".to_string(), Value::string("pending"));
                t.insert("amount".to_string(), Value::number(50.0));
                Value::Object(t)
            },
            {
                let mut t = HashMap::new();
                t.insert("id".to_string(), Value::number(3.0));
                t.insert("status".to_string(), Value::string("completed"));
                t.insert("amount".to_string(), Value::number(75.0));
                Value::Object(t)
            },
        ];

        let mut data = HashMap::new();
        data.insert("transactions".to_string(), Value::Array(transactions));
        ctx.insert("data", Value::object(data));

        ctx
    }

    #[test]
    fn test_execute_simple_source() {
        let ctx = make_test_data();
        let parser = ExpressionParser::new();
        let executor = ExpressionExecutor::new();

        let expr = parser.parse("data.transactions").unwrap();
        let result = executor.execute(&expr, &ctx).unwrap();

        assert!(result.is_array());
        assert_eq!(result.len(), 3);
    }

    #[test]
    fn test_execute_source_not_found() {
        let ctx = DataContext::new();
        let parser = ExpressionParser::new();
        let executor = ExpressionExecutor::new();

        let expr = parser.parse("nonexistent").unwrap();
        let result = executor.execute(&expr, &ctx);

        assert!(matches!(result, Err(ExecutionError::SourceNotFound(_))));
    }

    #[test]
    fn test_execute_filter() {
        let ctx = make_test_data();
        let parser = ExpressionParser::new();
        let executor = ExpressionExecutor::new();

        let expr = parser
            .parse("{{ data.transactions | filter(status=completed) }}")
            .unwrap();
        let result = executor.execute(&expr, &ctx).unwrap();

        assert!(result.is_array());
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_execute_count() {
        let ctx = make_test_data();
        let parser = ExpressionParser::new();
        let executor = ExpressionExecutor::new();

        let expr = parser.parse("{{ data.transactions | count }}").unwrap();
        let result = executor.execute(&expr, &ctx).unwrap();

        assert_eq!(result.as_number(), Some(3.0));
    }

    #[test]
    fn test_execute_filter_then_count() {
        let ctx = make_test_data();
        let parser = ExpressionParser::new();
        let executor = ExpressionExecutor::new();

        let expr = parser
            .parse("{{ data.transactions | filter(status=completed) | count }}")
            .unwrap();
        let result = executor.execute(&expr, &ctx).unwrap();

        assert_eq!(result.as_number(), Some(2.0));
    }

    #[test]
    fn test_execute_select() {
        let ctx = make_test_data();
        let parser = ExpressionParser::new();
        let executor = ExpressionExecutor::new();

        let expr = parser
            .parse("{{ data.transactions | select(id, status) }}")
            .unwrap();
        let result = executor.execute(&expr, &ctx).unwrap();

        let arr = result.as_array().unwrap();
        assert_eq!(arr.len(), 3);

        // First item should only have id and status
        let first = arr[0].as_object().unwrap();
        assert!(first.contains_key("id"));
        assert!(first.contains_key("status"));
        assert!(!first.contains_key("amount"));
    }

    #[test]
    fn test_execute_sort_asc() {
        let ctx = make_test_data();
        let parser = ExpressionParser::new();
        let executor = ExpressionExecutor::new();

        let expr = parser
            .parse("{{ data.transactions | sort(amount) }}")
            .unwrap();
        let result = executor.execute(&expr, &ctx).unwrap();

        let arr = result.as_array().unwrap();
        let amounts: Vec<f64> = arr
            .iter()
            .filter_map(|v| v.get("amount")?.as_number())
            .collect();
        assert_eq!(amounts, vec![50.0, 75.0, 100.0]);
    }

    #[test]
    fn test_execute_sort_desc() {
        let ctx = make_test_data();
        let parser = ExpressionParser::new();
        let executor = ExpressionExecutor::new();

        let expr = parser
            .parse("{{ data.transactions | sort(amount, desc=true) }}")
            .unwrap();
        let result = executor.execute(&expr, &ctx).unwrap();

        let arr = result.as_array().unwrap();
        let amounts: Vec<f64> = arr
            .iter()
            .filter_map(|v| v.get("amount")?.as_number())
            .collect();
        assert_eq!(amounts, vec![100.0, 75.0, 50.0]);
    }

    #[test]
    fn test_execute_limit() {
        let ctx = make_test_data();
        let parser = ExpressionParser::new();
        let executor = ExpressionExecutor::new();

        let expr = parser.parse("{{ data.transactions | limit(2) }}").unwrap();
        let result = executor.execute(&expr, &ctx).unwrap();

        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_execute_sum() {
        let ctx = make_test_data();
        let parser = ExpressionParser::new();
        let executor = ExpressionExecutor::new();

        let expr = parser
            .parse("{{ data.transactions | sum(amount) }}")
            .unwrap();
        let result = executor.execute(&expr, &ctx).unwrap();

        assert_eq!(result.as_number(), Some(225.0)); // 100 + 50 + 75
    }

    #[test]
    fn test_execute_mean() {
        let ctx = make_test_data();
        let parser = ExpressionParser::new();
        let executor = ExpressionExecutor::new();

        let expr = parser
            .parse("{{ data.transactions | mean(amount) }}")
            .unwrap();
        let result = executor.execute(&expr, &ctx).unwrap();

        assert_eq!(result.as_number(), Some(75.0)); // 225 / 3
    }

    #[test]
    fn test_execute_sample() {
        let ctx = make_test_data();
        let parser = ExpressionParser::new();
        let executor = ExpressionExecutor::new();

        let expr = parser.parse("{{ data.transactions | sample(2) }}").unwrap();
        let result = executor.execute(&expr, &ctx).unwrap();

        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_execute_percentage() {
        let mut ctx = DataContext::new();
        ctx.insert("ratio", Value::number(0.75));

        let parser = ExpressionParser::new();
        let executor = ExpressionExecutor::new();

        let expr = parser.parse("{{ ratio | percentage }}").unwrap();
        let result = executor.execute(&expr, &ctx).unwrap();

        assert_eq!(result.as_number(), Some(75.0));
    }

    #[test]
    fn test_execute_chain() {
        let ctx = make_test_data();
        let parser = ExpressionParser::new();
        let executor = ExpressionExecutor::new();

        let expr = parser
            .parse("{{ data.transactions | filter(status=completed) | sort(amount, desc=true) | limit(1) }}")
            .unwrap();
        let result = executor.execute(&expr, &ctx).unwrap();

        let arr = result.as_array().unwrap();
        assert_eq!(arr.len(), 1);
        assert_eq!(arr[0].get("amount").unwrap().as_number(), Some(100.0));
    }

    #[test]
    fn test_filter_numeric_match() {
        let ctx = make_test_data();
        let parser = ExpressionParser::new();
        let executor = ExpressionExecutor::new();

        let expr = parser
            .parse("{{ data.transactions | filter(amount=100) }}")
            .unwrap();
        let result = executor.execute(&expr, &ctx).unwrap();

        assert_eq!(result.len(), 1);
    }

    #[test]
    fn test_execute_on_empty_array() {
        let mut ctx = DataContext::new();
        ctx.insert("items", Value::array(vec![]));

        let parser = ExpressionParser::new();
        let executor = ExpressionExecutor::new();

        let expr = parser.parse("{{ items | count }}").unwrap();
        let result = executor.execute(&expr, &ctx).unwrap();

        assert_eq!(result.as_number(), Some(0.0));
    }

    #[test]
    fn test_execute_mean_empty_array() {
        let mut ctx = DataContext::new();
        ctx.insert("items", Value::array(vec![]));

        let parser = ExpressionParser::new();
        let executor = ExpressionExecutor::new();

        let expr = parser.parse("{{ items | mean(value) }}").unwrap();
        let result = executor.execute(&expr, &ctx).unwrap();

        assert_eq!(result.as_number(), Some(0.0));
    }

    // ===== Rate Transform Tests =====

    fn make_time_series_data() -> DataContext {
        let mut ctx = DataContext::new();

        let events: Vec<Value> = vec![
            {
                let mut e = HashMap::new();
                e.insert("timestamp".to_string(), Value::number(1000.0));
                e.insert("value".to_string(), Value::number(10.0));
                Value::Object(e)
            },
            {
                let mut e = HashMap::new();
                e.insert("timestamp".to_string(), Value::number(2000.0));
                e.insert("value".to_string(), Value::number(20.0));
                Value::Object(e)
            },
            {
                let mut e = HashMap::new();
                e.insert("timestamp".to_string(), Value::number(3000.0));
                e.insert("value".to_string(), Value::number(30.0));
                Value::Object(e)
            },
        ];

        ctx.insert("events", Value::Array(events));
        ctx
    }

    #[test]
    fn test_execute_rate() {
        let ctx = make_time_series_data();
        let parser = ExpressionParser::new();
        let executor = ExpressionExecutor::new();

        let expr = parser.parse("{{ events | rate(5s) }}").unwrap();
        let result = executor.execute(&expr, &ctx).unwrap();

        // Rate should be sum of values in window / window in seconds
        // Window is 5s (5000ms), all 3 values sum to 60, rate = 60 / 5 = 12
        assert!(result.is_number());
        let rate = result.as_number().unwrap();
        assert!(rate > 0.0);
    }

    #[test]
    fn test_execute_rate_minute_window() {
        let ctx = make_time_series_data();
        let parser = ExpressionParser::new();
        let executor = ExpressionExecutor::new();

        let expr = parser.parse("{{ events | rate(1m) }}").unwrap();
        let result = executor.execute(&expr, &ctx).unwrap();

        assert!(result.is_number());
    }

    // ===== Group By Tests =====

    #[test]
    fn test_execute_group_by() {
        let ctx = make_test_data();
        let parser = ExpressionParser::new();
        let executor = ExpressionExecutor::new();

        let expr = parser
            .parse("{{ data.transactions | group_by(status) }}")
            .unwrap();
        let result = executor.execute(&expr, &ctx).unwrap();

        let arr = result.as_array().unwrap();
        // Should have 2 groups: "completed" (2 items) and "pending" (1 item)
        assert_eq!(arr.len(), 2);

        for group in arr {
            let obj = group.as_object().unwrap();
            assert!(obj.contains_key("key"));
            assert!(obj.contains_key("items"));
            assert!(obj.contains_key("count"));
        }
    }

    // ===== Distinct Tests =====

    #[test]
    fn test_execute_distinct_field() {
        let ctx = make_test_data();
        let parser = ExpressionParser::new();
        let executor = ExpressionExecutor::new();

        let expr = parser
            .parse("{{ data.transactions | distinct(status) }}")
            .unwrap();
        let result = executor.execute(&expr, &ctx).unwrap();

        // Should get first of each distinct status
        assert_eq!(result.len(), 2); // "completed" and "pending"
    }

    #[test]
    fn test_execute_distinct_no_field() {
        let mut ctx = DataContext::new();
        ctx.insert(
            "items",
            Value::array(vec![
                Value::string("a"),
                Value::string("b"),
                Value::string("a"),
                Value::string("c"),
            ]),
        );

        let parser = ExpressionParser::new();
        let executor = ExpressionExecutor::new();

        let expr = parser.parse("{{ items | distinct }}").unwrap();
        let result = executor.execute(&expr, &ctx).unwrap();

        assert_eq!(result.len(), 3); // "a", "b", "c"
    }

    // ===== Where Tests =====

    #[test]
    fn test_execute_where_gt() {
        let ctx = make_test_data();
        let parser = ExpressionParser::new();
        let executor = ExpressionExecutor::new();

        let expr = parser
            .parse("{{ data.transactions | where(amount, gt, 60) }}")
            .unwrap();
        let result = executor.execute(&expr, &ctx).unwrap();

        // Should get 2 items: amount=100 and amount=75
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_execute_where_lt() {
        let ctx = make_test_data();
        let parser = ExpressionParser::new();
        let executor = ExpressionExecutor::new();

        let expr = parser
            .parse("{{ data.transactions | where(amount, lt, 80) }}")
            .unwrap();
        let result = executor.execute(&expr, &ctx).unwrap();

        // Should get 2 items: amount=50 and amount=75
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_execute_where_eq() {
        let ctx = make_test_data();
        let parser = ExpressionParser::new();
        let executor = ExpressionExecutor::new();

        let expr = parser
            .parse("{{ data.transactions | where(status, eq, pending) }}")
            .unwrap();
        let result = executor.execute(&expr, &ctx).unwrap();

        assert_eq!(result.len(), 1);
    }

    #[test]
    fn test_execute_where_ne() {
        let ctx = make_test_data();
        let parser = ExpressionParser::new();
        let executor = ExpressionExecutor::new();

        let expr = parser
            .parse("{{ data.transactions | where(status, ne, pending) }}")
            .unwrap();
        let result = executor.execute(&expr, &ctx).unwrap();

        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_execute_where_contains() {
        let mut ctx = DataContext::new();
        let items: Vec<Value> = vec![
            {
                let mut t = HashMap::new();
                t.insert("name".to_string(), Value::string("hello world"));
                Value::Object(t)
            },
            {
                let mut t = HashMap::new();
                t.insert("name".to_string(), Value::string("goodbye"));
                Value::Object(t)
            },
        ];
        ctx.insert("items", Value::Array(items));

        let parser = ExpressionParser::new();
        let executor = ExpressionExecutor::new();

        let expr = parser
            .parse("{{ items | where(name, contains, world) }}")
            .unwrap();
        let result = executor.execute(&expr, &ctx).unwrap();

        assert_eq!(result.len(), 1);
    }

    // ===== Offset Tests =====

    #[test]
    fn test_execute_offset() {
        let ctx = make_test_data();
        let parser = ExpressionParser::new();
        let executor = ExpressionExecutor::new();

        let expr = parser.parse("{{ data.transactions | offset(1) }}").unwrap();
        let result = executor.execute(&expr, &ctx).unwrap();

        assert_eq!(result.len(), 2); // Original 3, skip 1
    }

    #[test]
    fn test_execute_offset_with_limit() {
        let ctx = make_test_data();
        let parser = ExpressionParser::new();
        let executor = ExpressionExecutor::new();

        let expr = parser
            .parse("{{ data.transactions | offset(1) | limit(1) }}")
            .unwrap();
        let result = executor.execute(&expr, &ctx).unwrap();

        assert_eq!(result.len(), 1);
    }

    // ===== Min/Max Tests =====

    #[test]
    fn test_execute_min() {
        let ctx = make_test_data();
        let parser = ExpressionParser::new();
        let executor = ExpressionExecutor::new();

        let expr = parser
            .parse("{{ data.transactions | min(amount) }}")
            .unwrap();
        let result = executor.execute(&expr, &ctx).unwrap();

        assert_eq!(result.as_number(), Some(50.0));
    }

    #[test]
    fn test_execute_max() {
        let ctx = make_test_data();
        let parser = ExpressionParser::new();
        let executor = ExpressionExecutor::new();

        let expr = parser
            .parse("{{ data.transactions | max(amount) }}")
            .unwrap();
        let result = executor.execute(&expr, &ctx).unwrap();

        assert_eq!(result.as_number(), Some(100.0));
    }

    #[test]
    fn test_execute_min_empty() {
        let mut ctx = DataContext::new();
        ctx.insert("items", Value::array(vec![]));

        let parser = ExpressionParser::new();
        let executor = ExpressionExecutor::new();

        let expr = parser.parse("{{ items | min(value) }}").unwrap();
        let result = executor.execute(&expr, &ctx).unwrap();

        assert!(result.is_null());
    }

    // ===== First/Last Tests =====

    #[test]
    fn test_execute_first() {
        let ctx = make_test_data();
        let parser = ExpressionParser::new();
        let executor = ExpressionExecutor::new();

        let expr = parser.parse("{{ data.transactions | first(2) }}").unwrap();
        let result = executor.execute(&expr, &ctx).unwrap();

        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_execute_last() {
        let ctx = make_test_data();
        let parser = ExpressionParser::new();
        let executor = ExpressionExecutor::new();

        let expr = parser.parse("{{ data.transactions | last(2) }}").unwrap();
        let result = executor.execute(&expr, &ctx).unwrap();

        let arr = result.as_array().unwrap();
        assert_eq!(arr.len(), 2);

        // Last item should be id=3
        assert_eq!(arr[1].get("id").unwrap().as_number(), Some(3.0));
    }

    // ===== Flatten Tests =====

    #[test]
    fn test_execute_flatten() {
        let mut ctx = DataContext::new();
        ctx.insert(
            "nested",
            Value::array(vec![
                Value::array(vec![Value::number(1.0), Value::number(2.0)]),
                Value::array(vec![Value::number(3.0), Value::number(4.0)]),
            ]),
        );

        let parser = ExpressionParser::new();
        let executor = ExpressionExecutor::new();

        let expr = parser.parse("{{ nested | flatten }}").unwrap();
        let result = executor.execute(&expr, &ctx).unwrap();

        let arr = result.as_array().unwrap();
        assert_eq!(arr.len(), 4);
        assert_eq!(arr[0].as_number(), Some(1.0));
        assert_eq!(arr[3].as_number(), Some(4.0));
    }

    #[test]
    fn test_execute_flatten_mixed() {
        let mut ctx = DataContext::new();
        ctx.insert(
            "items",
            Value::array(vec![
                Value::number(1.0),
                Value::array(vec![Value::number(2.0), Value::number(3.0)]),
                Value::number(4.0),
            ]),
        );

        let parser = ExpressionParser::new();
        let executor = ExpressionExecutor::new();

        let expr = parser.parse("{{ items | flatten }}").unwrap();
        let result = executor.execute(&expr, &ctx).unwrap();

        assert_eq!(result.len(), 4);
    }

    // ===== Reverse Tests =====

    #[test]
    fn test_execute_reverse() {
        let ctx = make_test_data();
        let parser = ExpressionParser::new();
        let executor = ExpressionExecutor::new();

        let expr = parser.parse("{{ data.transactions | reverse }}").unwrap();
        let result = executor.execute(&expr, &ctx).unwrap();

        let arr = result.as_array().unwrap();
        // First item should now be the last one (id=3)
        assert_eq!(arr[0].get("id").unwrap().as_number(), Some(3.0));
    }

    // ===== Complex Chain Tests =====

    #[test]
    fn test_execute_complex_chain() {
        let ctx = make_test_data();
        let parser = ExpressionParser::new();
        let executor = ExpressionExecutor::new();

        // Filter by status, sort by amount descending, get first 1
        let expr = parser
            .parse("{{ data.transactions | where(status, eq, completed) | sort(amount, desc=true) | first(1) }}")
            .unwrap();
        let result = executor.execute(&expr, &ctx).unwrap();

        let arr = result.as_array().unwrap();
        assert_eq!(arr.len(), 1);
        // Should be the completed transaction with highest amount (100)
        assert_eq!(arr[0].get("amount").unwrap().as_number(), Some(100.0));
    }

    #[test]
    fn test_execute_group_then_count() {
        let ctx = make_test_data();
        let parser = ExpressionParser::new();
        let executor = ExpressionExecutor::new();

        let expr = parser
            .parse("{{ data.transactions | group_by(status) | count }}")
            .unwrap();
        let result = executor.execute(&expr, &ctx).unwrap();

        // Should count the groups (2)
        assert_eq!(result.as_number(), Some(2.0));
    }

    // ===== Join Tests =====

    fn make_join_test_data() -> DataContext {
        let mut ctx = DataContext::new();

        // Orders dataset
        let orders = Value::Array(vec![
            {
                let mut obj = HashMap::new();
                obj.insert("id".to_string(), Value::Number(1.0));
                obj.insert("customer_id".to_string(), Value::Number(100.0));
                obj.insert("amount".to_string(), Value::Number(50.0));
                Value::Object(obj)
            },
            {
                let mut obj = HashMap::new();
                obj.insert("id".to_string(), Value::Number(2.0));
                obj.insert("customer_id".to_string(), Value::Number(101.0));
                obj.insert("amount".to_string(), Value::Number(75.0));
                Value::Object(obj)
            },
            {
                let mut obj = HashMap::new();
                obj.insert("id".to_string(), Value::Number(3.0));
                obj.insert("customer_id".to_string(), Value::Number(100.0));
                obj.insert("amount".to_string(), Value::Number(25.0));
                Value::Object(obj)
            },
            {
                let mut obj = HashMap::new();
                obj.insert("id".to_string(), Value::Number(4.0));
                obj.insert("customer_id".to_string(), Value::Number(999.0)); // No matching customer
                obj.insert("amount".to_string(), Value::Number(10.0));
                Value::Object(obj)
            },
        ]);

        // Customers dataset
        let customers = Value::Array(vec![
            {
                let mut obj = HashMap::new();
                obj.insert("customer_id".to_string(), Value::Number(100.0));
                obj.insert("name".to_string(), Value::String("Alice".to_string()));
                obj.insert("tier".to_string(), Value::String("gold".to_string()));
                Value::Object(obj)
            },
            {
                let mut obj = HashMap::new();
                obj.insert("customer_id".to_string(), Value::Number(101.0));
                obj.insert("name".to_string(), Value::String("Bob".to_string()));
                obj.insert("tier".to_string(), Value::String("silver".to_string()));
                Value::Object(obj)
            },
        ]);

        ctx.insert("orders", orders);
        ctx.insert("customers", customers);
        ctx
    }

    #[test]
    fn test_execute_join_basic() {
        let ctx = make_join_test_data();
        let parser = ExpressionParser::new();
        let executor = ExpressionExecutor::new();

        let expr = parser
            .parse("{{ orders | join(customers, on=customer_id) }}")
            .unwrap();
        let result = executor.execute(&expr, &ctx).unwrap();

        let arr = result.as_array().unwrap();
        // 4 orders total - 3 with matches, 1 without
        assert_eq!(arr.len(), 4);

        // Check first order (customer 100 - Alice)
        let first = arr[0].as_object().unwrap();
        assert_eq!(first.get("id").unwrap().as_number(), Some(1.0));
        assert_eq!(first.get("name").unwrap().as_str(), Some("Alice"));
        assert_eq!(first.get("tier").unwrap().as_str(), Some("gold"));
    }

    #[test]
    fn test_execute_join_multiple_matches() {
        let ctx = make_join_test_data();
        let executor = ExpressionExecutor::new();

        // Create expression manually for testing
        let expr = Expression {
            source: "orders".to_string(),
            transforms: vec![Transform::Join {
                other: "customers".to_string(),
                on: "customer_id".to_string(),
            }],
        };

        let result = executor.execute(&expr, &ctx).unwrap();
        let arr = result.as_array().unwrap();

        // Count orders for customer 100 (should have 2 orders: id 1 and id 3)
        let alice_orders: Vec<_> = arr
            .iter()
            .filter(|v| {
                v.as_object()
                    .and_then(|o| o.get("name"))
                    .and_then(|n| n.as_str())
                    == Some("Alice")
            })
            .collect();
        assert_eq!(alice_orders.len(), 2);
    }

    #[test]
    fn test_execute_join_no_match_keeps_left() {
        let ctx = make_join_test_data();
        let executor = ExpressionExecutor::new();

        let expr = Expression {
            source: "orders".to_string(),
            transforms: vec![Transform::Join {
                other: "customers".to_string(),
                on: "customer_id".to_string(),
            }],
        };

        let result = executor.execute(&expr, &ctx).unwrap();
        let arr = result.as_array().unwrap();

        // Find order 4 (customer 999, no match)
        let order4: Vec<_> = arr
            .iter()
            .filter(|v| {
                v.as_object()
                    .and_then(|o| o.get("id"))
                    .and_then(|n| n.as_number())
                    == Some(4.0)
            })
            .collect();
        assert_eq!(order4.len(), 1);

        // Should still have original fields but no name/tier
        let obj = order4[0].as_object().unwrap();
        assert_eq!(obj.get("customer_id").unwrap().as_number(), Some(999.0));
        assert!(obj.get("name").is_none());
    }

    #[test]
    fn test_execute_join_other_source_not_found() {
        let ctx = make_join_test_data();
        let executor = ExpressionExecutor::new();

        let expr = Expression {
            source: "orders".to_string(),
            transforms: vec![Transform::Join {
                other: "nonexistent".to_string(),
                on: "customer_id".to_string(),
            }],
        };

        let result = executor.execute(&expr, &ctx);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ExecutionError::SourceNotFound(_)
        ));
    }

    #[test]
    fn test_execute_join_chained_with_filter() {
        let ctx = make_join_test_data();
        let parser = ExpressionParser::new();
        let executor = ExpressionExecutor::new();

        // Join and then filter to only gold tier customers
        let expr = parser
            .parse("{{ orders | join(customers, on=customer_id) | filter(tier=gold) }}")
            .unwrap();
        let result = executor.execute(&expr, &ctx).unwrap();

        let arr = result.as_array().unwrap();
        // Only orders from customer 100 (Alice, gold tier) - 2 orders
        assert_eq!(arr.len(), 2);
    }

    #[test]
    fn test_execute_join_empty_left() {
        let mut ctx = DataContext::new();
        ctx.insert("empty", Value::Array(vec![]));
        ctx.insert(
            "other",
            Value::Array(vec![{
                let mut obj = HashMap::new();
                obj.insert("id".to_string(), Value::Number(1.0));
                Value::Object(obj)
            }]),
        );

        let executor = ExpressionExecutor::new();
        let expr = Expression {
            source: "empty".to_string(),
            transforms: vec![Transform::Join {
                other: "other".to_string(),
                on: "id".to_string(),
            }],
        };

        let result = executor.execute(&expr, &ctx).unwrap();
        let arr = result.as_array().unwrap();
        assert!(arr.is_empty());
    }

    #[test]
    fn test_execute_join_empty_right() {
        let mut ctx = DataContext::new();
        ctx.insert(
            "orders",
            Value::Array(vec![{
                let mut obj = HashMap::new();
                obj.insert("id".to_string(), Value::Number(1.0));
                Value::Object(obj)
            }]),
        );
        ctx.insert("empty", Value::Array(vec![]));

        let executor = ExpressionExecutor::new();
        let expr = Expression {
            source: "orders".to_string(),
            transforms: vec![Transform::Join {
                other: "empty".to_string(),
                on: "id".to_string(),
            }],
        };

        let result = executor.execute(&expr, &ctx).unwrap();
        let arr = result.as_array().unwrap();
        // Left join keeps left items even with no matches
        assert_eq!(arr.len(), 1);
    }

    #[test]
    fn test_execute_join_conflicting_field_names() {
        let mut ctx = DataContext::new();

        // Both have "value" field
        ctx.insert(
            "left",
            Value::Array(vec![{
                let mut obj = HashMap::new();
                obj.insert("id".to_string(), Value::Number(1.0));
                obj.insert("value".to_string(), Value::String("left_val".to_string()));
                Value::Object(obj)
            }]),
        );
        ctx.insert(
            "right",
            Value::Array(vec![{
                let mut obj = HashMap::new();
                obj.insert("id".to_string(), Value::Number(1.0));
                obj.insert("value".to_string(), Value::String("right_val".to_string()));
                obj.insert("extra".to_string(), Value::String("extra_val".to_string()));
                Value::Object(obj)
            }]),
        );

        let executor = ExpressionExecutor::new();
        let expr = Expression {
            source: "left".to_string(),
            transforms: vec![Transform::Join {
                other: "right".to_string(),
                on: "id".to_string(),
            }],
        };

        let result = executor.execute(&expr, &ctx).unwrap();
        let arr = result.as_array().unwrap();
        assert_eq!(arr.len(), 1);

        let obj = arr[0].as_object().unwrap();
        // Original left value should be preserved
        assert_eq!(obj.get("value").unwrap().as_str(), Some("left_val"));
        // Right value should be prefixed
        assert_eq!(obj.get("right_value").unwrap().as_str(), Some("right_val"));
        // Non-conflicting fields should be added directly
        assert_eq!(obj.get("extra").unwrap().as_str(), Some("extra_val"));
    }

    #[test]
    fn test_execute_join_with_sum() {
        let ctx = make_join_test_data();
        let parser = ExpressionParser::new();
        let executor = ExpressionExecutor::new();

        // Join, filter to gold tier, sum amounts
        let expr = parser
            .parse(
                "{{ orders | join(customers, on=customer_id) | filter(tier=gold) | sum(amount) }}",
            )
            .unwrap();
        let result = executor.execute(&expr, &ctx).unwrap();

        // Alice has orders 1 (50) and 3 (25) = 75
        assert_eq!(result.as_number(), Some(75.0));
    }

    // ===== Suggest Transform Tests =====

    #[test]
    fn test_execute_suggest_with_array() {
        let mut ctx = DataContext::new();

        // Create suggestion data (simulating pre-computed suggestions)
        let suggestions = Value::Array(vec![
            {
                let mut obj = HashMap::new();
                obj.insert("text".to_string(), Value::String("git status".to_string()));
                obj.insert("score".to_string(), Value::Number(0.15));
                Value::Object(obj)
            },
            {
                let mut obj = HashMap::new();
                obj.insert("text".to_string(), Value::String("git commit".to_string()));
                obj.insert("score".to_string(), Value::Number(0.12));
                Value::Object(obj)
            },
            {
                let mut obj = HashMap::new();
                obj.insert("text".to_string(), Value::String("cargo build".to_string()));
                obj.insert("score".to_string(), Value::Number(0.10));
                Value::Object(obj)
            },
        ]);
        ctx.insert("suggestions", suggestions);

        let parser = ExpressionParser::new();
        let executor = ExpressionExecutor::new();

        // Filter suggestions starting with "git"
        let expr = parser.parse("{{ suggestions | suggest(git, 5) }}").unwrap();
        let result = executor.execute(&expr, &ctx).unwrap();

        let arr = result.as_array().unwrap();
        assert_eq!(arr.len(), 2); // Only git status and git commit
    }

    #[test]
    fn test_execute_suggest_with_model_object() {
        let mut ctx = DataContext::new();

        // Create a model object with pre-computed suggestions
        let mut model = HashMap::new();
        model.insert(
            "model_type".to_string(),
            Value::String("ngram_lm".to_string()),
        );
        model.insert(
            "source".to_string(),
            Value::String("./model.apr".to_string()),
        );
        model.insert(
            "_suggestions".to_string(),
            Value::Array(vec![
                {
                    let mut obj = HashMap::new();
                    obj.insert("text".to_string(), Value::String("git status".to_string()));
                    obj.insert("score".to_string(), Value::Number(0.15));
                    Value::Object(obj)
                },
                {
                    let mut obj = HashMap::new();
                    obj.insert("text".to_string(), Value::String("git commit".to_string()));
                    obj.insert("score".to_string(), Value::Number(0.12));
                    Value::Object(obj)
                },
            ]),
        );
        ctx.insert("model", Value::Object(model));

        let parser = ExpressionParser::new();
        let executor = ExpressionExecutor::new();

        let expr = parser.parse("{{ model | suggest(git, 5) }}").unwrap();
        let result = executor.execute(&expr, &ctx).unwrap();

        let arr = result.as_array().unwrap();
        assert_eq!(arr.len(), 2); // Pre-computed suggestions
    }

    #[test]
    fn test_execute_suggest_empty_model() {
        let mut ctx = DataContext::new();

        // Model without pre-computed suggestions
        let mut model = HashMap::new();
        model.insert(
            "model_type".to_string(),
            Value::String("ngram_lm".to_string()),
        );
        ctx.insert("model", Value::Object(model));

        let parser = ExpressionParser::new();
        let executor = ExpressionExecutor::new();

        let expr = parser.parse("{{ model | suggest(git, 5) }}").unwrap();
        let result = executor.execute(&expr, &ctx).unwrap();

        let arr = result.as_array().unwrap();
        assert!(arr.is_empty()); // No suggestions when not pre-computed
    }

    #[test]
    fn test_parse_suggest() {
        let parser = ExpressionParser::new();
        let expr = parser.parse("{{ model | suggest(git, 8) }}").unwrap();

        assert_eq!(expr.source, "model");
        assert_eq!(expr.transforms.len(), 1);
        assert!(matches!(
            &expr.transforms[0],
            Transform::Suggest { prefix, count } if prefix == "git" && *count == 8
        ));
    }
}
