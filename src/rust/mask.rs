//! Masking an address for anything that renders on screen.
//!
//!   someone.long@example.com  ->  som**ong@ex**e.com
//!   joe@mail.example.com      ->  j**@m**.example.com
//!
//! Duplicated as `maskEmail` in src/panel/mask.ts, which cannot shell out per
//! row. A test asserts the two agree.

/// Reveals less of a short part, so nothing under three characters leaks.
fn part(s: &str) -> String {
    let chars: Vec<char> = s.chars().collect();
    let n = chars.len();
    let take = |from: usize, to: usize| -> String { chars[from..to].iter().collect() };
    if n <= 2 {
        "**".into()
    } else if n <= 5 {
        format!("{}**", take(0, 1))
    } else if n <= 8 {
        format!("{}**{}", take(0, 2), take(n - 1, n))
    } else {
        format!("{}**{}", take(0, 3), take(n - 3, n))
    }
}

pub fn email(raw: &str) -> String {
    if raw.is_empty() {
        return String::new();
    }
    let Some(at) = raw.rfind('@') else {
        return part(raw);
    };
    if at == 0 {
        return format!("@{}", part(&raw[1..]));
    }
    let local = &raw[..at];
    let domain = &raw[at + 1..];
    let (host, suffix) = match domain.find('.') {
        Some(dot) if dot > 0 => (&domain[..dot], &domain[dot..]),
        _ => (domain, ""),
    };
    format!("{}@{}{}", part(local), part(host), suffix)
}
