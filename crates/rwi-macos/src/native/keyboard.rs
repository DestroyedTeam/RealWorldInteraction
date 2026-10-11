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
