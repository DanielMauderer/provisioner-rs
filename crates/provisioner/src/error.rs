#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    MissingField(&'static str),
    InvalidValue(&'static str),
    /// A value in the form body is not valid UTF-8.
    InvalidEncoding,
    /// Percent-encoding is malformed (truncated `%`, non-hex digits, etc.).
    MalformedEncoding,
}
