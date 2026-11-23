//! One checkpoint line from an apply log.
//!
//! A config hook prints
//! `essential-mise-checkpoint: {"check":"google-accounts","accounts":["user@example.com"]}`
//! and exits 0. The console keeps those accounts. This module does not spawn
//! a process, does not open a database, and does not pass the line to a shell.

use std::collections::HashSet;

/// Text that starts a checkpoint line, including the trailing space.
pub(crate) const PREFIX: &str = "essential-mise-checkpoint: ";

/// What `observe` has seen so far.
///
/// The first well-formed line stays. A later well-formed line does not
/// replace it. Any malformed checkpoint line becomes [`Record::Error`] and
/// stays there, even when a good line was seen earlier.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) enum Record {
    /// No checkpoint line yet.
    #[default]
    None,
    /// The first accounts list that parsed.
    Ready(Sentinel),
    /// A checkpoint line that did not parse. Still blocks.
    Error(String),
}

/// Accounts from a well-formed checkpoint line. Already ASCII-lowercased.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Sentinel {
    /// One to sixteen addresses, in line order, with no duplicates.
    pub(crate) accounts: Vec<String>,
}

/// Fold one apply-log line into `record`.
pub(crate) fn observe(record: &mut Record, line: &str) {
    let Some(parsed) = parse_line(line) else {
        return;
    };
    match parsed {
        Err(message) => {
            if !matches!(record, Record::Error(_)) {
                *record = Record::Error(message);
            }
        }
        Ok(sentinel) => {
            if matches!(record, Record::None) {
                *record = Record::Ready(sentinel);
            }
        }
    }
}

/// Parse one line. `None` when it is not a checkpoint line.
pub(crate) fn parse_line(line: &str) -> Option<Result<Sentinel, String>> {
    let body = line.strip_prefix(PREFIX)?;
    Some(parse_body(body))
}

fn parse_body(body: &str) -> Result<Sentinel, String> {
    let value: serde_json::Value = serde_json::from_str(body)
        .map_err(|_| "checkpoint line is not a JSON object".to_string())?;
    let serde_json::Value::Object(map) = value else {
        return Err("checkpoint line is not a JSON object".to_string());
    };
    if map.len() != 2 || !map.contains_key("check") || !map.contains_key("accounts") {
        return Err("checkpoint line must contain only check and accounts".to_string());
    }
    let Some(check) = map.get("check").and_then(serde_json::Value::as_str) else {
        return Err("checkpoint check is not a string".to_string());
    };
    if check != "google-accounts" {
        return Err("checkpoint check is not google-accounts".to_string());
    }
    let Some(list) = map.get("accounts").and_then(serde_json::Value::as_array) else {
        return Err("checkpoint accounts is not an array".to_string());
    };
    if list.is_empty() || list.len() > 16 {
        return Err("checkpoint accounts must list 1 to 16 addresses".to_string());
    }
    let mut accounts = Vec::with_capacity(list.len());
    let mut seen = HashSet::with_capacity(list.len());
    for item in list {
        let Some(raw) = item.as_str() else {
            return Err("checkpoint account is not a string".to_string());
        };
        let address = ascii_lower(raw);
        if !address_ok(&address) {
            return Err(format!("checkpoint account is not an email address: {raw}"));
        }
        if !seen.insert(address.clone()) {
            return Err(format!("checkpoint account is duplicated: {address}"));
        }
        accounts.push(address);
    }
    Ok(Sentinel { accounts })
}

fn ascii_lower(value: &str) -> String {
    value.chars().map(|c| c.to_ascii_lowercase()).collect()
}

/// `^[a-z0-9._%+-]+@[a-z0-9.-]+\.[a-z]{2,}$` on an already lowercased string.
fn address_ok(value: &str) -> bool {
    let bytes = value.as_bytes();
    let Some(at) = bytes.iter().position(|byte| *byte == b'@') else {
        return false;
    };
    if at == 0 {
        return false;
    }
    let local = &bytes[..at];
    let domain = &bytes[at + 1..];
    if local.is_empty()
        || domain.is_empty()
        || !local.iter().copied().all(local_byte)
        || domain.contains(&b'@')
    {
        return false;
    }
    let Some(dot) = domain.iter().rposition(|byte| *byte == b'.') else {
        return false;
    };
    let name = &domain[..dot];
    let tld = &domain[dot + 1..];
    !name.is_empty()
        && tld.len() >= 2
        && name.iter().copied().all(domain_byte)
        && tld.iter().copied().all(|byte| byte.is_ascii_lowercase())
}

fn local_byte(byte: u8) -> bool {
    byte.is_ascii_lowercase()
        || byte.is_ascii_digit()
        || matches!(byte, b'.' | b'_' | b'%' | b'+' | b'-')
}

fn domain_byte(byte: u8) -> bool {
    byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'.' | b'-')
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(accounts: &str) -> String {
        format!(r#"{PREFIX}{{"check":"google-accounts","accounts":[{accounts}]}}"#)
    }

    fn one(address: &str) -> String {
        line(&format!("\"{address}\""))
    }

    #[test]
    fn parses_accounts_and_ignores_key_order() {
        let sentinel = parse_line(&one("User@Example.com"))
            .expect("checkpoint")
            .expect("ok");
        assert_eq!(sentinel.accounts, vec!["user@example.com".to_string()]);

        let swapped = format!(
            r#"{PREFIX}{{"accounts":["a@example.com","b@example.com"],"check":"google-accounts"}}"#
        );
        let sentinel = parse_line(&swapped).expect("checkpoint").expect("ok");
        assert_eq!(
            sentinel.accounts,
            vec!["a@example.com".to_string(), "b@example.com".to_string()]
        );
    }

    #[test]
    fn ignores_a_line_without_the_prefix() {
        assert!(parse_line("phase 1: base").is_none());
        assert!(
            parse_line(
                "essential-mise-checkpoint:{\"check\":\"google-accounts\",\"accounts\":[\"user@example.com\"]}"
            )
            .is_none()
        );
    }

    #[test]
    fn rejects_a_malformed_checkpoint_line() {
        let mut many = Vec::new();
        for index in 0..17 {
            many.push(format!("\"user{index}@example.com\""));
        }
        let cases = [
            one("not-an-email"),
            line("\"user@example.com\",\"user@example.com\""),
            line("\"User@Example.com\",\"user@example.com\""),
            format!("{PREFIX}{{\"check\":\"google-accounts\",\"accounts\":[]}}"),
            format!(
                "{PREFIX}{{\"check\":\"google-accounts\",\"accounts\":[{}]}}",
                many.join(",")
            ),
            format!("{PREFIX}{{\"check\":\"google-accounts\",\"accounts\":\"user@example.com\"}}"),
            format!(
                "{PREFIX}{{\"check\":\"google-accounts\",\"accounts\":[\"user@example.com\"],\"script\":\"bin/example.ts\"}}"
            ),
            format!(r#"{PREFIX}{{"script":"bin/example.ts"}}"#),
            format!(r#"{PREFIX}{{"check":"other","accounts":["user@example.com"]}}"#),
            format!(r#"{PREFIX}{{"accounts":["user@example.com"]}}"#),
            format!("{PREFIX}not-json"),
            format!("{PREFIX}[]"),
            format!("{PREFIX}{{\"check\":\"google-accounts\",\"accounts\":[1]}}"),
            format!(
                "{PREFIX}{{\"check\":\"google-accounts\",\"accounts\":[\"user@example.com\"]}} trailing"
            ),
        ];
        for case in cases {
            let err = parse_line(&case).expect("prefix").expect_err(&case);
            assert!(!err.is_empty(), "{case}");
        }
    }

    #[test]
    fn first_line_wins_and_a_bad_line_still_blocks() {
        let mut record = Record::None;
        observe(&mut record, &one("user@example.com"));
        observe(&mut record, &one("other@example.com"));
        assert_eq!(
            record,
            Record::Ready(Sentinel {
                accounts: vec!["user@example.com".into()],
            })
        );
        observe(&mut record, &format!("{PREFIX}{{}}"));
        assert!(matches!(record, Record::Error(_)));
        let error = record.clone();
        observe(&mut record, &one("user@example.com"));
        assert_eq!(record, error);

        let mut only_bad = Record::None;
        observe(&mut only_bad, &format!("{PREFIX}{{}}"));
        observe(&mut only_bad, &one("user@example.com"));
        assert!(matches!(only_bad, Record::Error(_)));
    }
}
