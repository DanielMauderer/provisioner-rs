#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum ParseError {
    MissingField(&'static str),
    InvalidValue(&'static str),
    /// A value in the form body is not valid UTF-8.
    InvalidEncoding,
    /// Percent-encoding is malformed (truncated `%`, non-hex digits, etc.).
    MalformedEncoding,
}

impl ParseError {
    /// Returns a short, human-readable description of the error variant.
    ///
    /// This is useful for logging or rendering the error on a web page
    /// without exposing the internal field name when one is present.
    pub fn as_str(&self) -> &'static str {
        match self {
            ParseError::MissingField(_) => "missing field",
            ParseError::InvalidValue(_) => "invalid value",
            ParseError::InvalidEncoding => "invalid encoding",
            ParseError::MalformedEncoding => "malformed encoding",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_as_str_returns_expected_descriptions() {
        assert_eq!(ParseError::MissingField("ssid").as_str(), "missing field");
        assert_eq!(
            ParseError::InvalidValue("use_dhcp").as_str(),
            "invalid value"
        );
        assert_eq!(ParseError::InvalidEncoding.as_str(), "invalid encoding");
        assert_eq!(ParseError::MalformedEncoding.as_str(), "malformed encoding");
    }
}
