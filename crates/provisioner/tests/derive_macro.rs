//! Integration tests for the `#[derive(Provision)]` macro.
//!
//! These test that the generated `ProvisionConfig` implementation correctly
//! parses form bodies and serialises/deserialises to bytes.

use provisioner::Provision;
use provisioner::config::ProvisionConfig;
use provisioner::error::ParseError;

/// A minimal config struct exercising common field types.
#[derive(Debug, PartialEq, Provision)]
struct TestConfig {
    ssid: heapless::String<32>,
    password: heapless::String<64>,
    use_dhcp: bool,
}

#[test]
fn from_form_parses_all_fields() {
    let body = b"ssid=MyWiFi&password=s3cr3t&use_dhcp=true";
    let cfg = TestConfig::from_form(body).unwrap();

    assert_eq!(cfg.ssid, "MyWiFi");
    assert_eq!(cfg.password, "s3cr3t");
    assert!(cfg.use_dhcp);
}

#[test]
fn from_form_missing_field_is_error() {
    let body = b"ssid=MyWiFi&use_dhcp=true";
    let err = TestConfig::from_form(body).unwrap_err();
    assert_eq!(err, ParseError::MissingField("password"));
}

#[test]
fn from_form_invalid_value_is_error() {
    let body = b"ssid=MyWiFi&password=s3cr3t&use_dhcp=notabool";
    let err = TestConfig::from_form(body).unwrap_err();
    assert_eq!(err, ParseError::InvalidValue("use_dhcp"));
}

#[test]
fn from_form_invalid_utf8_is_error() {
    // 0xFF is never valid UTF-8
    let body = b"ssid=MyWiFi&password=s3cr3t&use_dhcp=\xFF";
    let err = TestConfig::from_form(body).unwrap_err();
    assert_eq!(err, ParseError::InvalidEncoding);
}

#[test]
fn to_bytes_roundtrip() {
    let cfg = TestConfig {
        ssid: heapless::String::try_from("HomeNet").unwrap(),
        password: heapless::String::try_from("p4ss!").unwrap(),
        use_dhcp: false,
    };

    let mut buf = [0u8; 256];
    let n = cfg.to_bytes(&mut buf).unwrap();
    let serialised = &buf[..n];

    let restored = TestConfig::from_bytes(serialised).unwrap();
    assert_eq!(restored.ssid, "HomeNet");
    assert_eq!(restored.password, "p4ss!");
    assert!(!restored.use_dhcp);
}

#[test]
fn to_bytes_roundtrip_with_special_chars() {
    let cfg = TestConfig {
        ssid: heapless::String::try_from("WiFi&Co=Fun").unwrap(),
        password: heapless::String::try_from("a&b=c").unwrap(),
        use_dhcp: true,
    };

    let mut buf = [0u8; 256];
    let n = cfg.to_bytes(&mut buf).unwrap();

    let restored = TestConfig::from_bytes(&buf[..n]).unwrap();
    assert_eq!(restored.ssid, "WiFi&Co=Fun");
    assert_eq!(restored.password, "a&b=c");
    assert!(restored.use_dhcp);
}

#[test]
fn html_is_not_empty() {
    let html = TestConfig::HTML;
    assert!(!html.is_empty());
    assert!(html.contains("<form"));
    assert!(html.contains("ssid"));
    assert!(html.contains("password"));
    assert!(html.contains("use_dhcp"));
}
