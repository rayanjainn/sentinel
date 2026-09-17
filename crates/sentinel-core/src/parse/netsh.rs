//! `netsh advfirewall` output.

/// `netsh advfirewall show currentprofile state` → whether the active profile is on. The
/// "State" label is localized, so the value is taken from the line whose last token is ON/OFF.
pub fn profile_state(output: &str) -> Option<bool> {
    output.lines().find_map(|line| {
        let mut tokens = line.split_whitespace();
        let first = tokens.next()?;
        let last = tokens.last()?;
        if first.starts_with('-') {
            return None;
        }
        match last.to_ascii_uppercase().as_str() {
            "ON" => Some(true),
            "OFF" => Some(false),
            _ => None,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_profile_state() {
        let on = "\r\nPrivate Profile Settings: \r\n----------------------------------------------------------------------\r\nState                                 ON\r\nOk.\r\n";
        assert_eq!(profile_state(on), Some(true));
        let off =
            "Domain Profile Settings:\n------\nZustand                               OFF\nOk.\n";
        assert_eq!(profile_state(off), Some(false));
        assert_eq!(
            profile_state("The requested operation requires elevation."),
            None
        );
    }
}
