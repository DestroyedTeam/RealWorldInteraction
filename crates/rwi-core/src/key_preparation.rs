//! Explicit keyboard preparation envelope. Legacy chord strings stay byte-identical.
use crate::{Result, fault};
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum KeyPreparation {
    /// macOS private legacy-248 record; opt-in, ABI/effect not attested.
    TargetKeyWindowRecord248Experimental,
}
#[derive(Debug, PartialEq, Eq)]
pub struct KeyChordPayload {
    pub key: String,
    pub preparation: Option<KeyPreparation>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Envelope {
    key: String,
    preparation: KeyPreparation,
}
fn bounded_key(key: &str) -> Result<()> {
    if key.is_empty() || key.len() > 128 || key.contains('\0') {
        return Err(fault(
            "invalid_key_preparation",
            "Explicit preparation requires key 1..128 UTF-8 bytes without NUL.",
        ));
    }
    Ok(())
}
impl KeyChordPayload {
    pub fn parse(value: &str) -> Result<Self> {
        if value.trim_start().starts_with('{') {
            let e: Envelope = serde_json::from_str(value).map_err(|_| {
                fault(
                    "invalid_key_preparation",
                    "Use one exact key/preparation envelope; no unknown fields or fallback.",
                )
            })?;
            bounded_key(&e.key)?;
            Ok(Self {
                key: e.key,
                preparation: Some(e.preparation),
            })
        } else {
            // Native platform chord validation remains authoritative for legacy calls.
            Ok(Self {
                key: value.into(),
                preparation: None,
            })
        }
    }
    pub fn encode(key: String, preparation: Option<KeyPreparation>) -> Result<String> {
        match preparation {
            None => Ok(key),
            Some(preparation) => {
                bounded_key(&key)?;
                serde_json::to_string(&Envelope { key, preparation }).map_err(|_| {
                    fault(
                        "invalid_key_preparation",
                        "Could not encode exact key preparation.",
                    )
                })
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    const P: KeyPreparation = KeyPreparation::TargetKeyWindowRecord248Experimental;
    #[test]
    fn f293_legacy_encoding_is_exact() {
        for key in ["n", "cmd+s", "", "ctrl+alt+delete"] {
            assert_eq!(KeyChordPayload::encode(key.into(), None).unwrap(), key);
        }
    }
    #[test]
    fn f293_legacy_parser_does_not_invent_preparation() {
        let p = KeyChordPayload::parse("cmd+s").unwrap();
        assert_eq!(p.key, "cmd+s");
        assert_eq!(p.preparation, None);
    }
    #[test]
    fn f293_explicit_roundtrip() {
        let s = KeyChordPayload::encode("n".into(), Some(P)).unwrap();
        let p = KeyChordPayload::parse(&s).unwrap();
        assert_eq!(p.key, "n");
        assert_eq!(p.preparation, Some(P));
    }
    #[test]
    fn f293_unknown_preparation_refuses() {
        assert!(KeyChordPayload::parse(r#"{"key":"n","preparation":"other"}"#).is_err());
    }
    #[test]
    fn f293_extra_fields_refuse() {
        assert!(KeyChordPayload::parse(r#"{"key":"n","preparation":"target_key_window_record248_experimental","fallback":true}"#).is_err());
    }
    #[test]
    fn f293_null_missing_or_wrong_type_refuse() {
        for s in [
            r#"{"key":"n"}"#,
            r#"{"key":"n","preparation":null}"#,
            r#"{"key":5,"preparation":"target_key_window_record248_experimental"}"#,
        ] {
            assert!(KeyChordPayload::parse(s).is_err());
        }
    }
    #[test]
    fn f293_explicit_key_is_bounded() {
        for k in [String::new(), "n".repeat(129), "n\0".into()] {
            assert!(KeyChordPayload::encode(k, Some(P)).is_err());
        }
    }
    #[test]
    fn f293_boundary_and_payload_are_not_goal_evidence() {
        let s = KeyChordPayload::encode("n".repeat(128), Some(P)).unwrap();
        assert_eq!(KeyChordPayload::parse(&s).unwrap().key.len(), 128);
        assert!(!s.contains("goal_verified"));
    }
}
