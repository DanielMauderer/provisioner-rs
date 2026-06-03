use crate::error::ParseError;

pub trait ProvisionConfig: Sized {
    const HTML: &'static str;
    fn from_form(body: &[u8]) -> Result<Self, ParseError>;
    fn to_bytes(&self, buf: &mut [u8]) -> Result<usize, ParseError>;
    fn from_bytes(buf: &[u8]) -> Result<Self, ParseError>;
}
