#[derive(Debug)]
pub enum ParseError {
    MissingField(&'static str),
    InvalidValue(&'static str),
}
