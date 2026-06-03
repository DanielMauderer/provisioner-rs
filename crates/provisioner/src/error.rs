#[derive(Debug, PartialEq)]
pub enum ParseError {
    MissingField(&'static str),
    InvalidValue(&'static str),
    InvalidEncoding,
}
