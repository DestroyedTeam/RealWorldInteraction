//! Native key construction and bounded text/chord parsing.
use super::*;

pub(super) fn source() -> Result<CGEventSource> {
    CGEventSource::new(CGEventSourceStateID::Private).map_err(|_| {
        fault(
            "input_source_unavailable",
            "Native event source is unavailable.",
        )
    })
}
pub(super) fn key_events(
    key: u16,
    flags: CGEventFlags,
    text: Option<&[u16]>,
) -> Result<(CGEvent, CGEvent)> {
    let down = CGEvent::new_keyboard_event(source()?, key, true)
        .map_err(|_| fault("input_allocation_failed", "Key event was not allocated."))?;
    let up = CGEvent::new_keyboard_event(source()?, key, false)
        .map_err(|_| fault("input_allocation_failed", "Key event was not allocated."))?;
    for event in [&down, &up] {
        event.set_flags(flags);
        if let Some(text) = text {
            event.set_string_from_utf16_unchecked(text);
        }
    }
    Ok((down, up))
}
pub(super) fn utf16_chunks(text: &str, limit: usize) -> Vec<Vec<u16>> {
    let mut out = Vec::new();
    let mut chunk = Vec::new();
    for c in text.chars() {
        let mut buf = [0u16; 2];
        let units = c.encode_utf16(&mut buf);
        if chunk.len() + units.len() > limit && !chunk.is_empty() {
            out.push(std::mem::take(&mut chunk));
        }
        chunk.extend_from_slice(units);
    }
    if !chunk.is_empty() {
        out.push(chunk);
    }
    out
}
/// One Unicode scalar per text pair: one or two UTF-16 units, never a split
/// surrogate. This preserves code units, not grapheme atomicity or normalization.
pub(super) fn utf16_scalar_chunks(text: &str) -> Vec<Vec<u16>> {
    utf16_chunks(text, 1)
}

pub(super) fn parse_chord(value: &str) -> Result<(u16, CGEventFlags)> {
    let mut flags = CGEventFlags::empty();
    let mut key = None;
    for token in value.split('+') {
        let code = match token.to_ascii_lowercase().as_str() {
            "cmd" | "command" | "super" | "meta" => {
                flags |= CGEventFlags::CGEventFlagCommand;
                continue;
            }
            "shift" => {
                flags |= CGEventFlags::CGEventFlagShift;
                continue;
            }
            "ctrl" | "control" => {
                flags |= CGEventFlags::CGEventFlagControl;
                continue;
            }
            "alt" | "option" => {
                flags |= CGEventFlags::CGEventFlagAlternate;
                continue;
            }
            "a" => 0,
            "s" => 1,
            "d" => 2,
            "f" => 3,
            "h" => 4,
            "g" => 5,
            "z" => 6,
            "x" => 7,
            "c" => 8,
            "v" => 9,
            "b" => 11,
            "q" => 12,
            "w" => 13,
            "e" => 14,
            "r" => 15,
            "y" => 16,
            "t" => 17,
            "o" => 31,
            "u" => 32,
            "i" => 34,
            "p" => 35,
            "l" => 37,
            "j" => 38,
            "k" => 40,
            "n" => 45,
            "m" => 46,
            "enter" | "return" => 36,
            "tab" => 48,
            "space" => 49,
            "backspace" => 51,
            "escape" | "esc" => 53,
            "left" => 123,
            "right" => 124,
            "down" => 125,
            "up" => 126,
            "minus" | "-" => 27,
            _ => {
                return Err(fault(
                    "unsupported_key",
                    "Use documented physical key names; do not substitute shell commands.",
                ));
            }
        };
        if key.replace(code).is_some() {
            return Err(fault(
                "invalid_chord",
                "Exactly one non-modifier key is required.",
            ));
        }
    }
    Ok((
        key.ok_or_else(|| fault("invalid_chord", "Chord contains no key."))?,
        flags,
    ))
}

#[cfg(test)]
mod scalar_text_partition_tests {
    use super::*;

    #[test]
    fn scalar_text_partition_preserves_exact_utf16_and_bounded_scalar_payloads() {
        let long = "λ🙂".repeat(512);
        for text in [
            "RWI-S17-b07fb1046",
            "RWI-DIAG-focus-diag-native-9bced78ebf-唯一",
            "中文漢字",
            "🙂𝄞𐐷",
            "",
            long.as_str(),
        ] {
            let chunks = utf16_scalar_chunks(text);
            assert_eq!(chunks.len(), text.chars().count());
            assert_eq!(chunks.concat(), text.encode_utf16().collect::<Vec<_>>());
            assert!(chunks.iter().all(|chunk| {
                (1..=2).contains(&chunk.len())
                    && String::from_utf16(chunk).is_ok_and(|text| text.chars().count() == 1)
            }));
        }
    }

    #[test]
    fn scalar_text_partition_keeps_surrogates_at_the_old_twenty_unit_boundary() {
        let text = "1234567890123456789🙂B";
        let chunks = utf16_scalar_chunks(text);
        assert_eq!(chunks.len(), 21);
        assert_eq!(chunks[19], vec![0xd83d, 0xde42]);
        assert_eq!(String::from_utf16(&chunks.concat()).unwrap(), text);
    }

    #[test]
    fn scalar_text_partition_is_not_grapheme_segmentation_or_normalization() {
        for (text, scalars) in [("e\u{301}", 2), ("👩\u{200d}👩\u{200d}👧\u{200d}👦", 7)] {
            let chunks = utf16_scalar_chunks(text);
            assert_eq!(chunks.len(), scalars);
            assert_eq!(String::from_utf16(&chunks.concat()).unwrap(), text);
        }
        assert_ne!(utf16_scalar_chunks("é"), utf16_scalar_chunks("e\u{301}"));
    }

    #[test]
    fn scalar_text_partition_does_not_change_old_helper_zero_limit_semantics() {
        let expected = vec![vec![65], vec![0xd83d, 0xde42], vec![66]];
        assert_eq!(utf16_chunks("A🙂B", 0), expected);
        assert_eq!(utf16_chunks("A🙂B", 1), expected);
    }
}

#[cfg(test)]
mod f27_key_constructor_memory_tests {
    use super::*;

    #[test]
    fn f27_key_constructor_memory_actual_default_pair_and_public_factory_matrix() {
        let mut reports = Vec::new();
        for text in ["A", "后", "\u{301}", "🙂", "𝄞", "👩", "\u{200d}", "💻"] {
            let units: Vec<u16> = text.encode_utf16().collect();
            let (down, up) = key_events(0, CGEventFlags::empty(), Some(&units)).unwrap();
            for (kind, event) in [(10, &down), (11, &up)] {
                let report = crate::nsevent_probe::probe_key_constructor(event, text).unwrap();
                assert_eq!(report["public_nsevent"]["constructed"]["event_type"], kind);
                assert_eq!(report["expected_utf16"], serde_json::json!(units));
                assert_eq!(report["post_calls"], 0);
                assert_eq!(report["window_created"], false);
                assert_eq!(report["receiver_effect_verified"], false);
                reports.push(report);
            }
        }
        assert_eq!(reports.len(), 16);
        println!("F27_KEY_MEMORY {}", serde_json::json!({"cases":reports}));
    }

    #[test]
    fn f27_key_constructor_memory_invalid_payloads_refused_before_factory() {
        let (down, _) = key_events(0, CGEventFlags::empty(), Some(&[65])).unwrap();
        for text in ["", "AB", "🙂🙂"] {
            assert_eq!(
                crate::nsevent_probe::probe_key_constructor(&down, text),
                Err("invalid_key_constructor_fixture")
            );
        }
        assert_eq!(
            crate::nsevent_probe::probe_key_constructor(&down, "后"),
            Err("key_probe_current_payload_differs")
        );
    }
}
