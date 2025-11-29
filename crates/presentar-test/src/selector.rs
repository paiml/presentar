//! CSS-like selector parsing for widget queries.
//!
//! Supports:
//! - `"Button"` - by widget type
//! - `"#submit-btn"` - by ID
//! - `"[data-testid='login']"` - by test ID

use presentar_core::Widget;

/// Parsed selector.
#[derive(Debug, Clone, PartialEq)]
pub enum Selector {
    /// Match by widget type name
    Type(String),
    /// Match by ID (e.g., `#my-id`)
    Id(String),
    /// Match by test ID (e.g., `[data-testid='foo']`)
    TestId(String),
    /// Match by class (e.g., `.my-class`)
    Class(String),
    /// Match by attribute (e.g., `[aria-label='foo']`)
    Attribute { name: String, value: String },
    /// Descendant combinator (e.g., `Container Button`)
    Descendant(Box<Selector>, Box<Selector>),
    /// Child combinator (e.g., `Row > Button`)
    Child(Box<Selector>, Box<Selector>),
}

impl Selector {
    /// Parse a selector string.
    ///
    /// # Errors
    ///
    /// Returns an error if the selector is invalid.
    pub fn parse(input: &str) -> Result<Self, SelectorError> {
        SelectorParser::new(input).parse()
    }

    /// Check if this selector matches a widget.
    #[must_use]
    pub fn matches(&self, widget: &dyn Widget) -> bool {
        match self {
            Self::Type(name) => {
                // Simplified type matching - would compare actual TypeId
                // For now, always false since we can't easily get type names
                name.is_empty()
            }
            Self::Id(_id) => {
                // Would need widget.id() method
                false
            }
            Self::TestId(id) => widget.test_id() == Some(id.as_str()),
            Self::Class(_class) => {
                // Would need widget.classes() method
                false
            }
            Self::Attribute { name, value } => {
                if name == "data-testid" {
                    widget.test_id() == Some(value.as_str())
                } else if name == "aria-label" {
                    widget.accessible_name() == Some(value.as_str())
                } else {
                    false
                }
            }
            Self::Descendant(_, _) | Self::Child(_, _) => {
                // Would need parent context
                false
            }
        }
    }
}

/// Selector parser.
pub struct SelectorParser<'a> {
    input: &'a str,
    pos: usize,
}

impl<'a> SelectorParser<'a> {
    /// Create a new parser.
    #[must_use]
    pub fn new(input: &'a str) -> Self {
        Self { input, pos: 0 }
    }

    /// Parse the selector.
    pub fn parse(&mut self) -> Result<Selector, SelectorError> {
        self.skip_whitespace();

        if self.input.is_empty() {
            return Err(SelectorError::Empty);
        }

        self.parse_selector()
    }

    fn parse_selector(&mut self) -> Result<Selector, SelectorError> {
        let first = self.peek_char().ok_or(SelectorError::Empty)?;

        match first {
            '#' => self.parse_id(),
            '.' => self.parse_class(),
            '[' => self.parse_attribute(),
            _ if first.is_alphabetic() => self.parse_type(),
            _ => Err(SelectorError::UnexpectedChar(first)),
        }
    }

    fn parse_id(&mut self) -> Result<Selector, SelectorError> {
        self.advance(); // Skip '#'
        let id = self.read_identifier()?;
        Ok(Selector::Id(id))
    }

    fn parse_class(&mut self) -> Result<Selector, SelectorError> {
        self.advance(); // Skip '.'
        let class = self.read_identifier()?;
        Ok(Selector::Class(class))
    }

    fn parse_type(&mut self) -> Result<Selector, SelectorError> {
        let name = self.read_identifier()?;
        Ok(Selector::Type(name))
    }

    fn parse_attribute(&mut self) -> Result<Selector, SelectorError> {
        self.advance(); // Skip '['

        let name = self.read_until('=');
        if name.is_empty() {
            return Err(SelectorError::InvalidAttribute);
        }

        self.advance(); // Skip '='

        // Skip optional quote
        let quote = self.peek_char();
        if quote == Some('\'') || quote == Some('"') {
            self.advance();
        }

        let value = self.read_until_any(&['\'', '"', ']']);

        // Skip closing quote if present
        if self.peek_char() == Some('\'') || self.peek_char() == Some('"') {
            self.advance();
        }

        // Skip ']'
        if self.peek_char() != Some(']') {
            return Err(SelectorError::UnclosedAttribute);
        }
        self.advance();

        // Special case for data-testid
        if name == "data-testid" {
            Ok(Selector::TestId(value))
        } else {
            Ok(Selector::Attribute { name, value })
        }
    }

    fn read_identifier(&mut self) -> Result<String, SelectorError> {
        let start = self.pos;
        while let Some(c) = self.peek_char() {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                self.advance();
            } else {
                break;
            }
        }

        if self.pos == start {
            return Err(SelectorError::ExpectedIdentifier);
        }

        Ok(self.input[start..self.pos].to_string())
    }

    fn read_until(&mut self, stop: char) -> String {
        let start = self.pos;
        while let Some(c) = self.peek_char() {
            if c == stop {
                break;
            }
            self.advance();
        }
        self.input[start..self.pos].to_string()
    }

    fn read_until_any(&mut self, stops: &[char]) -> String {
        let start = self.pos;
        while let Some(c) = self.peek_char() {
            if stops.contains(&c) {
                break;
            }
            self.advance();
        }
        self.input[start..self.pos].to_string()
    }

    fn skip_whitespace(&mut self) {
        while let Some(c) = self.peek_char() {
            if c.is_whitespace() {
                self.advance();
            } else {
                break;
            }
        }
    }

    fn peek_char(&self) -> Option<char> {
        self.input[self.pos..].chars().next()
    }

    fn advance(&mut self) {
        if let Some(c) = self.peek_char() {
            self.pos += c.len_utf8();
        }
    }
}

/// Selector parsing error.
#[derive(Debug, Clone, PartialEq)]
pub enum SelectorError {
    /// Empty selector
    Empty,
    /// Unexpected character
    UnexpectedChar(char),
    /// Expected identifier
    ExpectedIdentifier,
    /// Invalid attribute syntax
    InvalidAttribute,
    /// Unclosed attribute bracket
    UnclosedAttribute,
}

impl std::fmt::Display for SelectorError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Empty => write!(f, "empty selector"),
            Self::UnexpectedChar(c) => write!(f, "unexpected character: '{c}'"),
            Self::ExpectedIdentifier => write!(f, "expected identifier"),
            Self::InvalidAttribute => write!(f, "invalid attribute syntax"),
            Self::UnclosedAttribute => write!(f, "unclosed attribute bracket"),
        }
    }
}

impl std::error::Error for SelectorError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_type() {
        let sel = Selector::parse("Button").unwrap();
        assert_eq!(sel, Selector::Type("Button".to_string()));
    }

    #[test]
    fn test_parse_id() {
        let sel = Selector::parse("#submit-btn").unwrap();
        assert_eq!(sel, Selector::Id("submit-btn".to_string()));
    }

    #[test]
    fn test_parse_class() {
        let sel = Selector::parse(".primary").unwrap();
        assert_eq!(sel, Selector::Class("primary".to_string()));
    }

    #[test]
    fn test_parse_test_id() {
        let sel = Selector::parse("[data-testid='login']").unwrap();
        assert_eq!(sel, Selector::TestId("login".to_string()));
    }

    #[test]
    fn test_parse_test_id_double_quotes() {
        let sel = Selector::parse("[data-testid=\"login\"]").unwrap();
        assert_eq!(sel, Selector::TestId("login".to_string()));
    }

    #[test]
    fn test_parse_attribute() {
        let sel = Selector::parse("[aria-label='Close']").unwrap();
        assert_eq!(
            sel,
            Selector::Attribute {
                name: "aria-label".to_string(),
                value: "Close".to_string(),
            }
        );
    }

    #[test]
    fn test_parse_empty_error() {
        let result = Selector::parse("");
        assert_eq!(result, Err(SelectorError::Empty));
    }

    #[test]
    fn test_parse_whitespace() {
        let sel = Selector::parse("  Button  ").unwrap();
        assert_eq!(sel, Selector::Type("Button".to_string()));
    }

    #[test]
    fn test_selector_error_display() {
        assert_eq!(SelectorError::Empty.to_string(), "empty selector");
        assert_eq!(
            SelectorError::UnexpectedChar('@').to_string(),
            "unexpected character: '@'"
        );
    }
}
