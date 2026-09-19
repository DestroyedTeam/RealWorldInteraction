//! Bounded keyboard chord parsing; desktop policy is supplied by the native backend.
use rwi_core::{Result, fault};

#[cfg(test)]
pub(super) fn chord(value: &str) -> Result<Vec<u16>> {
    chord_with_policy(value, false)
}
pub(super) fn chord_with_policy(value: &str, desktop_keys: bool) -> Result<Vec<u16>> {
    let parts = value.split('+').collect::<Vec<_>>();
    if parts.is_empty() || parts.len() > 5 {
        return Err(fault(
            "unsupported_chord",
            "Use nonduplicate Ctrl/Alt/Shift modifiers plus one key; Win and desktop switching require desktop policy.",
        ));
    }
    let mut keys = Vec::new();
    for part in &parts[..parts.len() - 1] {
        let key = match part.to_ascii_lowercase().as_str() {
            "ctrl" => 0x11,
            "alt" => 0x12,
            "shift" => 0x10,
            "win" if desktop_keys => 0x5b,
            _ => {
                return Err(fault(
                    "unsupported_chord",
                    "Unknown modifier or desktop shortcut outside desktop policy.",
                ));
            }
        };
        if keys.contains(&key) {
            return Err(fault(
                "unsupported_chord",
                "Duplicate modifiers are rejected.",
            ));
        }
        keys.push(key);
    }
    let last = parts.last().unwrap().to_ascii_lowercase();
    let key = match last.as_str() {
        "win" if desktop_keys => 0x5b,
        "enter" => 0x0d,
        "tab" => 0x09,
        "escape" => 0x1b,
        "space" => 0x20,
        "backspace" => 0x08,
        "delete" => 0x2e,
        "left" => 0x25,
        "up" => 0x26,
        "right" => 0x27,
        "down" => 0x28,
        "home" => 0x24,
        "end" => 0x23,
        "pageup" => 0x21,
        "pagedown" => 0x22,
        s if s.strip_prefix('f').is_some_and(|n| {
            !n.starts_with('0') && n.parse::<u16>().is_ok_and(|n| (1..=24).contains(&n))
        }) =>
        {
            0x70 + s[1..].parse::<u16>().expect("validated function key") - 1
        }
        s if s.len() == 1 && s.as_bytes()[0].is_ascii_alphanumeric() => {
            u16::from(s.as_bytes()[0].to_ascii_uppercase())
        }
        _ => {
            return Err(fault(
                "unsupported_chord",
                "Supported: letters/digits, Enter/Tab/Escape/Space, editing/navigation keys, F1..F24.",
            ));
        }
    };
    // Secure attention is not SendInput. Ordinary shell/window switching is
    // available only under desktop policy; reobserve after the one balanced chord.
    if (key == 0x2e && keys.contains(&0x11) && keys.contains(&0x12))
        || (!desktop_keys
            && ((key == 0x09 && keys.contains(&0x12)) || (key == 0x1b && keys.contains(&0x11))))
        || keys.contains(&key)
    {
        return Err(fault(
            "unsupported_chord",
            "Secure attention, duplicate keys, or desktop switching outside desktop policy are unsupported.",
        ));
    }
    keys.push(key);
    Ok(keys)
}
