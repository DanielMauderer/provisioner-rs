use provisioner::{Provision, ProvisionConfig, error::ParseError};

#[derive(Provision)]
struct TestConfig {
    name: heapless::String<32>,
    value: bool,
}

#[test]
fn html_is_empty_stub() {
    assert_eq!(TestConfig::HTML, "");
}

#[test]
fn invalid_utf8_returns_encoding_error() {
    let result = TestConfig::from_form(&[0xFF, 0xFE]);
    assert!(matches!(result, Err(ParseError::InvalidEncoding)));
}

#[test]
fn empty_body_returns_first_missing_field() {
    // Form decoder stub returns empty iterator, so all fields are missing.
    let result = TestConfig::from_form(b"");
    assert!(matches!(result, Err(ParseError::MissingField("name"))));
}

#[derive(Provision)]
struct AttrsConfig {
    ssid: heapless::String<32>,
    #[provision(secret)]
    password: heapless::String<64>,
    #[provision(input_type = "number")]
    port: heapless::String<8>,
    #[provision(default = "true")]
    use_dhcp: bool,
}

#[test]
fn field_attrs_compile() {
    // Compile-time check: the derive with field attributes must expand without error.
    assert_eq!(AttrsConfig::HTML, "");
}

#[derive(Provision)]
#[provision(css = "body{}", header = "<h1>Test</h1>")]
struct ContainerAttrsConfig {
    ssid: heapless::String<32>,
}

#[test]
fn container_attrs_compile() {
    assert_eq!(ContainerAttrsConfig::HTML, "");
}
