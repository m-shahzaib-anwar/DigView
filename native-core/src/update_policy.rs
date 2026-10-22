//! Reject unsafe update locations before any network request or installer work.
use crate::db::Result;
use base64::Engine as _;
fn decoded(value: &str) -> Result<String> {
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(value.trim())
        .map_err(|_| "Invalid update signature encoding")?;
    String::from_utf8(bytes).map_err(|_| "Invalid update signature encoding".into())
}
pub fn valid_public_key(value: &str) -> bool {
    decoded(value)
        .ok()
        .is_some_and(|text| minisign_verify::PublicKey::decode(&text).is_ok())
}
pub fn verify_artifact(bytes: &[u8], signature: &str, key: &str, version: &str) -> Result<()> {
    let key = minisign_verify::PublicKey::decode(&decoded(key)?)
        .map_err(|_| "Invalid update public key")?;
    let signature = minisign_verify::Signature::decode(&decoded(signature)?)
        .map_err(|_| "Invalid update signature")?;
    key.verify(bytes, &signature, true)
        .map_err(|_| "Update signature verification failed")?;
    let signed = signature
        .trusted_comment()
        .split('\t')
        .find_map(|s| s.strip_prefix("version:"));
    if signed != Some(version) {
        return Err("Update signature does not match the announced version".into());
    }
    Ok(())
}
pub fn public_https(value: &str) -> Result<url::Url> {
    let u = public_artifact_url(value)?;
    if u.query().is_some() {
        return Err("Update manifest URL must not contain a query".into());
    }
    Ok(u)
}
pub fn public_artifact_url(value: &str) -> Result<url::Url> {
    let u = url::Url::parse(value).map_err(|_| "Invalid update URL")?;
    let host = u.host_str().ok_or("Update URL needs a host")?;
    if value.len() > 2048
        || u.scheme() != "https"
        || !u.username().is_empty()
        || u.password().is_some()
        || u.fragment().is_some()
        || u.port().is_some_and(|p| p != 443)
        || !host.contains('.')
        || host.ends_with('.')
        || host.ends_with(".local")
        || host.ends_with(".localhost")
        || host.parse::<std::net::IpAddr>().is_ok()
        || host.starts_with('[')
    {
        return Err("Updates require a public HTTPS URL without credentials or fragments".into());
    }
    Ok(u)
}
pub fn configured(endpoint: &str, key: &str, development: bool) -> bool {
    !development
        && public_https(endpoint).is_ok()
        && (32..=4096).contains(&key.len())
        && !key.chars().any(char::is_control)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn actual_signed_release_fixture_refuses_tampering_unsigned_and_version_spoofing() {
        let bytes = include_bytes!("../tests/fixtures/update-artifact.txt");
        let key = include_str!("../tests/fixtures/update-public.txt").trim();
        let signature = include_str!("../tests/fixtures/update-artifact.sig.txt").trim();
        assert!(valid_public_key(key));
        assert!(!valid_public_key("not a key"));
        verify_artifact(bytes, signature, key, "0.2.0").unwrap();
        assert!(verify_artifact(b"tampered", signature, key, "0.2.0").is_err());
        assert!(verify_artifact(bytes, signature, key, "0.3.0").is_err());
        assert!(verify_artifact(bytes, "", key, "0.2.0").is_err());
        let altered = decoded(signature)
            .unwrap()
            .replace("version:0.2.0", "version:0.3.0");
        let forged = base64::engine::general_purpose::STANDARD.encode(altered);
        assert!(verify_artifact(bytes, &forged, key, "0.3.0").is_err());
        assert!(public_artifact_url("https://cdn.example.com/update.exe?download=1").is_ok());
        assert!(public_artifact_url("http://cdn.example.com/update.exe").is_err());
    }
    #[test]
    fn update_locations_and_development_gate_fail_closed() {
        let good = "https://releases.example.com/latest.json";
        let key = "A".repeat(120);
        assert!(configured(good, &key, false));
        assert!(!configured(good, &key, true));
        assert!(!configured(good, "", false));
        for bad in [
            "http://example.com/a",
            "file:///a",
            "https://user:pass@example.com/a",
            "https://127.0.0.1/a",
            "https://[::1]/a",
            "https://localhost/a",
            "https://x.local/a",
            "https://example.com/a?key=x",
            "https://example.com/a#x",
            "https://example.com:8443/a",
        ] {
            assert!(public_https(bad).is_err(), "{bad}");
        }
    }
}
