//! Order keys for lists the owner sorts by hand.
//!
//! A key is a string of base-36 digits that never ends in `0`. Between any two keys there is
//! always another one, so moving an item rewrites only that item's key. This is the midpoint step
//! of the well-known "fractional indexing" scheme, without its integer header: keys only grow
//! longer where the owner keeps inserting into the same gap.

const DIGITS: &[u8] = b"0123456789abcdefghijklmnopqrstuvwxyz";

/// A key strictly between `before` (or the start) and `after` (or the end).
///
/// # Panics
/// When `before >= after`, or when either ends in `0`: both mean the caller passed keys that did
/// not come from this function.
pub fn between(before: Option<&str>, after: Option<&str>) -> String {
    let a = before.unwrap_or("").as_bytes();
    let b = after.map(str::as_bytes);
    assert!(a.last() != Some(&b'0'), "order key ends in 0");
    if let Some(b) = b {
        assert!(b.last() != Some(&b'0'), "order key ends in 0");
        assert!(a < b, "order keys out of order");
    }
    let key = midpoint(a, b);
    String::from_utf8(key).expect("order keys are ASCII")
}

/// A key after `last`, for appending to a list.
pub fn after(last: Option<&str>) -> String {
    between(last, None)
}

fn digit(byte: u8) -> usize {
    DIGITS
        .iter()
        .position(|d| *d == byte)
        .expect("order keys use base-36 digits")
}

fn midpoint(a: &[u8], b: Option<&[u8]>) -> Vec<u8> {
    if let Some(b) = b {
        // Shared prefix (a padded with zeros): keep it and find the midpoint of the rest.
        let mut n = 0;
        while n < b.len() && a.get(n).copied().unwrap_or(b'0') == b[n] {
            n += 1;
        }
        if n > 0 {
            let mut key = b[..n].to_vec();
            key.extend(midpoint(a.get(n..).unwrap_or(&[]), Some(&b[n..])));
            return key;
        }
    }
    let digit_a = a.first().map_or(0, |d| digit(*d));
    let digit_b = b.map_or(DIGITS.len(), |b| digit(b[0]));
    if digit_b - digit_a > 1 {
        vec![DIGITS[(digit_a + digit_b).div_ceil(2)]]
    } else if let Some(b) = b.filter(|b| b.len() > 1) {
        vec![b[0]]
    } else {
        let mut key = vec![DIGITS[digit_a]];
        key.extend(midpoint(a.get(1..).unwrap_or(&[]), None));
        key
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn appends_grow_in_order() {
        let mut last: Option<String> = None;
        let mut keys = Vec::new();
        for _ in 0..200 {
            let key = after(last.as_deref());
            if let Some(previous) = &last {
                assert!(previous < &key);
            }
            keys.push(key.clone());
            last = Some(key);
        }
        assert!(keys.iter().all(|k| !k.ends_with('0')));
    }

    #[test]
    fn there_is_always_room_between() {
        let mut low = after(None);
        let high = after(Some(&low));
        // Keep inserting just above `low`: the gap never closes.
        for _ in 0..100 {
            let mid = between(Some(&low), Some(&high));
            assert!(low < mid && mid < high, "{low} < {mid} < {high}");
            assert!(!mid.ends_with('0'));
            low = mid;
        }
        let mut high = high;
        let low = after(None);
        for _ in 0..100 {
            let mid = between(Some(&low), Some(&high));
            assert!(low < mid && mid < high, "{low} < {mid} < {high}");
            high = mid;
        }
    }

    #[test]
    fn a_key_before_the_first() {
        let first = after(None);
        let before = between(None, Some(&first));
        assert!(before < first);
    }

    #[test]
    #[should_panic(expected = "out of order")]
    fn refuses_reversed_bounds() {
        between(Some("b"), Some("a"));
    }
}
