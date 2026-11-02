/*
 * SPDX-License-Identifier: Apache-2.0
 */

//! Canonical username form shared by all core components: surrounding whitespace removed,
//! ASCII letters uppercased, and restricted to `^[A-Z0-9._-]{1,64}$`.

const MAX_LEN: usize = 64;

/// Returns the canonical form of `input`, or `None` if it does not match the username format.
pub fn normalize(input: &str) -> Option<String> {
    let canonical = input.trim().to_ascii_uppercase();
    let valid = !canonical.is_empty()
        && canonical.len() <= MAX_LEN
        && canonical
            .bytes()
            .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || matches!(b, b'.' | b'_' | b'-'));
    valid.then_some(canonical)
}

#[cfg(test)]
mod tests {
    use super::normalize;

    #[test]
    fn uppercases_and_trims() {
        assert_eq!(normalize(" q225265 ").as_deref(), Some("Q225265"));
        assert_eq!(normalize("Q225265").as_deref(), Some("Q225265"));
        assert_eq!(normalize("max.muster_mann-1").as_deref(), Some("MAX.MUSTER_MANN-1"));
    }

    #[test]
    fn rejects_invalid_format() {
        assert_eq!(normalize(""), None);
        assert_eq!(normalize("   "), None);
        assert_eq!(normalize("max mustermann"), None);
        assert_eq!(normalize("user@example.com"), None);
        assert_eq!(normalize("müller"), None);
        assert_eq!(normalize(&"A".repeat(65)), None);
        assert_eq!(normalize(&"a".repeat(64)).map(|s| s.len()), Some(64));
    }
}
