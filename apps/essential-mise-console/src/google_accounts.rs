//! Read-only check of Google Mail and Calendar in the Accounts database.
//!
//! The console runs this in-process. It does not start a shell, and it does
//! not format an address or any other external string into SQL.

use std::collections::{HashMap, HashSet};
use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

use rusqlite::types::Value;
use rusqlite::{Connection, ErrorCode, OpenFlags, params};
use serde::Serialize;

/// Shown above the account lines.
pub(crate) const TITLE: &str = "Google accounts";

/// Internet Accounts pane. Passed as one argv to `/usr/bin/open`.
pub(crate) const ACCOUNTS_PANE: &str =
    "x-apple.systempreferences:com.apple.Internet-Accounts-Settings.extension";

const GOOGLE: &str = "com.apple.account.Google";
const MAIL: &str = "com.apple.Dataclass.Mail";
const CALENDARS: &str = "com.apple.Dataclass.Calendars";
const PLUTIL: &str = "/usr/bin/plutil";
const OPEN: &str = "/usr/bin/open";

const ACCOUNT_SQL: &str = "SELECT a.Z_PK, a.ZUSERNAME, a.ZACTIVE, a.ZPARENTACCOUNT, t.ZACCOUNTTYPEDESCRIPTION, t.ZIDENTIFIER
FROM ZACCOUNT AS a JOIN ZACCOUNTTYPE AS t ON t.Z_PK = a.ZACCOUNTTYPE
WHERE lower(a.ZUSERNAME) = ?";

const ENABLED_SQL: &str = "SELECT d.ZNAME FROM Z_2ENABLEDDATACLASSES AS e
JOIN ZDATACLASS AS d ON d.Z_PK = e.Z_7ENABLEDDATACLASSES
WHERE e.Z_2ENABLEDACCOUNTS = ?";

const REQUIRED_TABLES: [&str; 4] = [
    "ZACCOUNT",
    "ZACCOUNTTYPE",
    "ZDATACLASS",
    "Z_2ENABLEDDATACLASSES",
];

const REQUIRED_COLUMNS: [(&str, &str); 9] = [
    ("ZACCOUNT", "ZUSERNAME"),
    ("ZACCOUNT", "ZACCOUNTTYPE"),
    ("ZACCOUNT", "ZPARENTACCOUNT"),
    ("ZACCOUNT", "ZACTIVE"),
    ("ZACCOUNTTYPE", "ZIDENTIFIER"),
    ("ZACCOUNTTYPE", "ZACCOUNTTYPEDESCRIPTION"),
    ("ZDATACLASS", "ZNAME"),
    ("Z_2ENABLEDDATACLASSES", "Z_2ENABLEDACCOUNTS"),
    ("Z_2ENABLEDDATACLASSES", "Z_7ENABLEDDATACLASSES"),
];

const UNREADABLE: &[&str] = &[
    "SQLITE_CANTOPEN",
    "SQLITE_NOTADB",
    "SQLITE_PERM",
    "SQLITE_AUTH",
    "SQLITE_BUSY",
    "SQLITE_LOCKED",
    "SQLITE_IOERR",
    "SQLITE_CORRUPT",
];

const ENTITIES: &[(&str, char)] = &[
    ("lt", '<'),
    ("gt", '>'),
    ("amp", '&'),
    ("quot", '"'),
    ("apos", '\''),
];

/// How far the check got for one address.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum AccountState {
    /// An active Google parent has Mail and Calendar on.
    Ok,
    /// No row uses this address.
    Missing,
    /// The address is present, but not as a Google parent.
    PresentButNotGmail,
    /// The Google parent is marked inactive.
    Inactive,
    /// Mail is off on the chosen Google parent. Both services off use this too.
    MailOff,
    /// Calendar is off and Mail is on.
    CalendarOff,
    /// Mail and Calendar are on different Google parents.
    Split,
}

/// What stopped the check, when it was not a per-account gap.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum ReportStatus {
    /// Every account is [`AccountState::Ok`].
    Ok,
    /// At least one account is not ok. The database was readable.
    Gaps,
    /// The database is not in the shape this check understands.
    SchemaMismatch,
    /// The database could not be read. Accounts were not scored as missing.
    Unreadable,
}

/// One row the check saw for an address.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct Found {
    /// `ZACCOUNTTYPEDESCRIPTION`, which may be localized.
    pub(crate) description: String,
    /// `ZIDENTIFIER` of the account type.
    pub(crate) identifier: String,
    /// True when `ZPARENTACCOUNT` is null.
    pub(crate) parent: bool,
    /// True when `ZACTIVE` is 1.
    pub(crate) active: bool,
    /// Mail is enabled on this row.
    pub(crate) mail: bool,
    /// Calendar is enabled on this row.
    pub(crate) calendars: bool,
}

/// One requested address after scoring.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct Account {
    /// ASCII-lowercased address.
    pub(crate) address: String,
    /// Scoring result.
    pub(crate) state: AccountState,
    /// Mail is on for the chosen row. False when the chosen row is not a parent.
    pub(crate) mail: bool,
    /// Calendar is on for the chosen row. False when the chosen row is not a parent.
    pub(crate) calendars: bool,
    /// Every matching row, ordered by `Z_PK`.
    pub(crate) found: Vec<Found>,
}

/// The checker's report. `reason` is omitted from JSON when absent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct Report {
    /// True only when every account is ok.
    pub(crate) ok: bool,
    /// `ok`, `gaps`, `schema-mismatch`, or `unreadable`.
    pub(crate) status: ReportStatus,
    /// Always [`TITLE`].
    pub(crate) title: String,
    /// Sentences for the screen. Empty when the report is satisfied.
    pub(crate) lines: Vec<String>,
    /// One entry per requested address, or empty when the database was not scored.
    pub(crate) accounts: Vec<Account>,
    /// Schema or unreadable detail. Absent on ok and gaps.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) reason: Option<String>,
}

impl Report {
    /// Exactly satisfied: `ok` is true and the status is `ok`.
    pub(crate) fn satisfied(&self) -> bool {
        self.ok && self.status == ReportStatus::Ok
    }

    fn gaps(&self) -> bool {
        self.status == ReportStatus::Gaps
    }
}

struct LoadedRow {
    pk: i64,
    description: String,
    identifier: String,
    parent_pk: Option<i64>,
    active: bool,
    mail: bool,
    calendars: bool,
}

struct Scored {
    address: String,
    state: AccountState,
    mail: bool,
    calendars: bool,
    found: Vec<Found>,
    line: Option<String>,
}

/// `$HOME/Library/Accounts/Accounts4.sqlite`.
pub(crate) fn default_db_path() -> PathBuf {
    let mut path = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_default();
    path.push("Library");
    path.push("Accounts");
    path.push("Accounts4.sqlite");
    path
}

/// ASCII-lowercase `value` and accept `^[a-z0-9._%+-]+@[a-z0-9.-]+\.[a-z]{2,}$`.
pub(crate) fn normalize_address(value: &str) -> Option<String> {
    let address: String = value.chars().map(|c| c.to_ascii_lowercase()).collect();
    if address_ok(&address) {
        Some(address)
    } else {
        None
    }
}

/// Split a comma-separated `--accounts` value. Empty segments are rejected.
pub(crate) fn parse_accounts(value: &str) -> Result<Vec<String>, String> {
    if value.is_empty() {
        return Err("google-accounts: --accounts needs a list".to_string());
    }
    let mut accounts = Vec::new();
    let mut seen = HashSet::new();
    for part in value.split(',') {
        if part.is_empty() {
            return Err(format!("google-accounts: not an email address: {part}"));
        }
        let Some(address) = normalize_address(part) else {
            return Err(format!("google-accounts: not an email address: {part}"));
        };
        if !seen.insert(address.clone()) {
            return Err(format!("google-accounts: duplicated address: {address}"));
        }
        accounts.push(address);
    }
    Ok(accounts)
}

/// Score `accounts` against the database at `db`. A missing file is gaps.
pub(crate) fn check(db: &Path, accounts: &[String]) -> Report {
    match std::fs::metadata(db) {
        Ok(meta) if meta.is_file() => {}
        Ok(_) => return unreadable("SQLITE_CANTOPEN"),
        Err(err) if err.kind() == io::ErrorKind::NotFound => {
            return scored_report(
                accounts
                    .iter()
                    .map(|address| missing_account(address))
                    .collect(),
            );
        }
        Err(err) => return unreadable(&io_code(&err)),
    }
    let conn = match Connection::open_with_flags(db, OpenFlags::SQLITE_OPEN_READ_ONLY) {
        Ok(conn) => conn,
        Err(err) => return report_for_sqlite(err),
    };
    if let Err(err) = prepare_connection(&conn) {
        return report_for_sqlite(err);
    }
    match evaluate(&conn, accounts) {
        Ok(report) => report,
        Err(err) => report_for_sqlite(err),
    }
}

/// Open the Internet Accounts pane. No shell.
pub(crate) fn open_accounts_pane() -> Result<(), String> {
    run_open(Path::new(OPEN))
}

/// Run `program` with the accounts pane as its only argument.
pub(crate) fn run_open(program: &Path) -> Result<(), String> {
    let output = Command::new(program)
        .arg(ACCOUNTS_PANE)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .output()
        .map_err(|err| format!("could not open System Settings: {err}"))?;
    if output.status.success() {
        Ok(())
    } else {
        let code = output
            .status
            .code()
            .map(|code| code.to_string())
            .unwrap_or_else(|| "no status".to_string());
        Err(format!("could not open System Settings ({code})"))
    }
}

/// Where `guide` reads, opens, and writes. The reader is injected in tests.
pub(crate) struct GuidePorts<'a> {
    /// One line, or `Ok(None)` at EOF.
    pub(crate) read_line: &'a mut dyn FnMut() -> io::Result<Option<String>>,
    /// Opens System Settings. Called at most once, on the transition into gaps.
    pub(crate) open_pane: &'a mut dyn FnMut() -> Result<(), String>,
    /// Prompt stream.
    pub(crate) stdout: &'a mut dyn Write,
    /// Failure lines. No checkpoint sentinel is written here.
    pub(crate) stderr: &'a mut dyn Write,
}

/// Wait until the report is satisfied. Two consecutive EOFs exit 1.
pub(crate) fn guide(db: &Path, accounts: &[String], ports: &mut GuidePorts<'_>) -> u8 {
    let mut opened = false;
    let mut open_error: Option<String> = None;
    let mut eofs = 0;
    loop {
        let report = check(db, accounts);
        if report.satisfied() {
            return 0;
        }
        if report.gaps() && !opened {
            opened = true;
            if let Err(err) = (ports.open_pane)() {
                open_error = Some(err);
            }
        }
        write_prompt(ports.stdout, &report, open_error.as_deref());
        loop {
            match (ports.read_line)() {
                Ok(None) => {
                    eofs += 1;
                    if eofs >= 2 {
                        write_failure(ports.stderr, &report, open_error.as_deref());
                        return 1;
                    }
                    write_prompt(ports.stdout, &report, open_error.as_deref());
                }
                Ok(Some(line)) => {
                    eofs = 0;
                    if line.trim().is_empty() {
                        break;
                    }
                    write_prompt(ports.stdout, &report, open_error.as_deref());
                }
                Err(err) => {
                    let _ = writeln!(ports.stderr, "google-accounts: {err}");
                    write_failure(ports.stderr, &report, open_error.as_deref());
                    return 1;
                }
            }
        }
    }
}

/// Read guide input from `/dev/tty`. SIGINT is left to the process default.
pub(crate) fn guide_from_tty(db: &Path, accounts: &[String]) -> u8 {
    let file = match std::fs::File::open("/dev/tty") {
        Ok(file) => file,
        Err(err) => {
            eprintln!("google-accounts: could not read the terminal: {err}");
            return 1;
        }
    };
    let mut reader = io::BufReader::new(file);
    let mut read_line = || {
        let mut line = String::new();
        match reader.read_line(&mut line) {
            Ok(0) => Ok(None),
            Ok(_) => Ok(Some(line)),
            Err(err) => Err(err),
        }
    };
    let mut open_pane = open_accounts_pane;
    let stdout = io::stdout();
    let mut stdout = stdout.lock();
    let stderr = io::stderr();
    let mut stderr = stderr.lock();
    let mut ports = GuidePorts {
        read_line: &mut read_line,
        open_pane: &mut open_pane,
        stdout: &mut stdout,
        stderr: &mut stderr,
    };
    guide(db, accounts, &mut ports)
}

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
    if local.is_empty() || domain.is_empty() || !local.iter().copied().all(local_byte) {
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
        && tld.iter().all(|byte| byte.is_ascii_lowercase())
}

fn local_byte(byte: u8) -> bool {
    byte.is_ascii_lowercase()
        || byte.is_ascii_digit()
        || matches!(byte, b'.' | b'_' | b'%' | b'+' | b'-')
}

fn domain_byte(byte: u8) -> bool {
    byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'.' | b'-')
}

fn prepare_connection(conn: &Connection) -> Result<(), rusqlite::Error> {
    conn.execute_batch("PRAGMA query_only = ON;")?;
    conn.busy_timeout(Duration::from_millis(2000))?;
    Ok(())
}

fn evaluate(conn: &Connection, accounts: &[String]) -> Result<Report, rusqlite::Error> {
    let tables = table_names(conn)?;
    for table in REQUIRED_TABLES {
        if !tables.contains(table) {
            return Ok(schema_report(format!("missing-table:{table}")));
        }
    }
    for (table, column) in REQUIRED_COLUMNS {
        if !column_names(conn, table)?.contains(column) {
            return Ok(schema_report(format!("missing-column:{table}.{column}")));
        }
    }
    let blobs = match dataclass_catalog(conn)? {
        Ok(blobs) => blobs,
        Err(report) => return Ok(report),
    };
    let mut google = 0;
    let mut stmt = conn.prepare("SELECT ZIDENTIFIER FROM ZACCOUNTTYPE")?;
    let rows = stmt.query_map([], |row| row.get::<_, Option<String>>(0))?;
    for row in rows {
        if row?.as_deref() == Some(GOOGLE) {
            google += 1;
        }
    }
    if google != 1 {
        return Ok(schema_report("google-type"));
    }
    let mut scored = Vec::with_capacity(accounts.len());
    for address in accounts {
        let rows = load_rows(conn, address, &blobs)?;
        scored.push(score_address(address, &rows));
    }
    Ok(scored_report(scored))
}

fn table_names(conn: &Connection) -> Result<HashSet<String>, rusqlite::Error> {
    let mut stmt = conn.prepare("SELECT name FROM sqlite_master WHERE type = 'table'")?;
    let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
    rows.collect()
}

fn column_names(conn: &Connection, table: &str) -> Result<HashSet<String>, rusqlite::Error> {
    let sql = match table {
        "ZACCOUNT" => "PRAGMA table_info(ZACCOUNT)",
        "ZACCOUNTTYPE" => "PRAGMA table_info(ZACCOUNTTYPE)",
        "ZDATACLASS" => "PRAGMA table_info(ZDATACLASS)",
        "Z_2ENABLEDDATACLASSES" => "PRAGMA table_info(Z_2ENABLEDDATACLASSES)",
        _ => return Ok(HashSet::new()),
    };
    let mut stmt = conn.prepare(sql)?;
    let rows = stmt.query_map([], |row| row.get::<_, String>(1))?;
    rows.collect()
}

fn dataclass_catalog(
    conn: &Connection,
) -> Result<Result<HashMap<Vec<u8>, String>, Report>, rusqlite::Error> {
    let mut stmt = conn.prepare("SELECT ZNAME FROM ZDATACLASS")?;
    let rows = stmt.query_map([], |row| row.get::<_, Value>(0))?;
    let mut by_blob = HashMap::new();
    let mut counts: HashMap<String, usize> = HashMap::new();
    for row in rows {
        let Some(bytes) = value_bytes(row?) else {
            return Ok(Err(schema_report("dataclass-archive")));
        };
        let identifier = match dataclass_identifier(&bytes) {
            Ok(identifier) => identifier,
            Err(reason) => return Ok(Err(schema_report(reason))),
        };
        *counts.entry(identifier.clone()).or_insert(0) += 1;
        by_blob.insert(bytes, identifier);
    }
    if counts.get(MAIL).copied() != Some(1) || counts.get(CALENDARS).copied() != Some(1) {
        return Ok(Err(schema_report("dataclass-catalog")));
    }
    Ok(Ok(by_blob))
}

fn value_bytes(value: Value) -> Option<Vec<u8>> {
    match value {
        Value::Blob(bytes) => Some(bytes),
        Value::Text(text) => Some(text.into_bytes()),
        _ => None,
    }
}

fn dataclass_identifier(bytes: &[u8]) -> Result<String, &'static str> {
    match plist_kind(bytes) {
        PlistKind::Xml => {
            let xml = std::str::from_utf8(bytes).map_err(|_| "dataclass-archive")?;
            dataclass_identifier_from_xml(xml).ok_or("dataclass-archive")
        }
        PlistKind::Binary => {
            let xml = plutil_xml(bytes)?;
            dataclass_identifier_from_xml(&xml).ok_or("dataclass-archive")
        }
        PlistKind::Other => Err("dataclass-archive"),
    }
}

enum PlistKind {
    Xml,
    Binary,
    Other,
}

fn plist_kind(bytes: &[u8]) -> PlistKind {
    let rest = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(bytes);
    let rest = trim_ascii_start(rest);
    if rest.starts_with(b"bplist") {
        return PlistKind::Binary;
    }
    let Ok(text) = std::str::from_utf8(bytes) else {
        return PlistKind::Other;
    };
    let trimmed = text.trim_start_matches(|c: char| c == '\u{feff}' || c.is_whitespace());
    if trimmed.starts_with('<') {
        PlistKind::Xml
    } else if trimmed.starts_with("bplist") {
        PlistKind::Binary
    } else {
        PlistKind::Other
    }
}

fn trim_ascii_start(bytes: &[u8]) -> &[u8] {
    let start = bytes
        .iter()
        .position(|byte| !byte.is_ascii_whitespace())
        .unwrap_or(bytes.len());
    &bytes[start..]
}

fn plutil_xml(bytes: &[u8]) -> Result<String, &'static str> {
    if !Path::new(PLUTIL).is_file() {
        return Err("plutil-missing");
    }
    let mut child = Command::new(PLUTIL)
        .args(["-convert", "xml1", "-o", "-", "-"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|_| "dataclass-archive")?;
    {
        let mut stdin = child.stdin.take().ok_or("dataclass-archive")?;
        stdin.write_all(bytes).map_err(|_| "dataclass-archive")?;
    }
    let output = child.wait_with_output().map_err(|_| "dataclass-archive")?;
    if !output.status.success() {
        return Err("dataclass-archive");
    }
    String::from_utf8(output.stdout).map_err(|_| "dataclass-archive")
}

fn load_rows(
    conn: &Connection,
    address: &str,
    blobs: &HashMap<Vec<u8>, String>,
) -> Result<Vec<LoadedRow>, rusqlite::Error> {
    let mut stmt = conn.prepare(ACCOUNT_SQL)?;
    let rows = stmt.query_map(params![address], |row| {
        Ok((
            row.get::<_, i64>(0)?,
            row.get::<_, Option<i64>>(2)?,
            row.get::<_, Option<i64>>(3)?,
            row.get::<_, Option<String>>(4)?,
            row.get::<_, Option<String>>(5)?,
        ))
    })?;
    let mut loaded = Vec::new();
    for row in rows {
        let (pk, active, parent, description, identifier) = row?;
        let enabled = enabled_identifiers(conn, pk, blobs)?;
        loaded.push(LoadedRow {
            pk,
            description: description.unwrap_or_default(),
            identifier: identifier.unwrap_or_default(),
            parent_pk: parent,
            active: active == Some(1),
            mail: enabled.contains(MAIL),
            calendars: enabled.contains(CALENDARS),
        });
    }
    Ok(loaded)
}

fn enabled_identifiers(
    conn: &Connection,
    pk: i64,
    blobs: &HashMap<Vec<u8>, String>,
) -> Result<HashSet<String>, rusqlite::Error> {
    let mut stmt = conn.prepare(ENABLED_SQL)?;
    let rows = stmt.query_map(params![pk], |row| row.get::<_, Value>(0))?;
    let mut identifiers = HashSet::new();
    for row in rows {
        let Some(bytes) = value_bytes(row?) else {
            continue;
        };
        if let Some(identifier) = blobs.get(&bytes) {
            identifiers.insert(identifier.clone());
        }
    }
    Ok(identifiers)
}

fn score_address(address: &str, rows: &[LoadedRow]) -> Scored {
    let mut ordered: Vec<&LoadedRow> = rows.iter().collect();
    ordered.sort_by_key(|row| row.pk);
    let found = ordered.iter().copied().map(to_found).collect();
    if rows
        .iter()
        .any(|row| is_google_parent(row) && row.active && row.mail && row.calendars)
    {
        return Scored {
            address: address.to_string(),
            state: AccountState::Ok,
            mail: true,
            calendars: true,
            found,
            line: None,
        };
    }
    if rows.is_empty() {
        let mut missing = missing_account(address);
        missing.found = found;
        return missing;
    }
    let google_parents: Vec<&LoadedRow> = rows.iter().filter(|row| is_google_parent(row)).collect();
    if google_parents.is_empty() {
        let named = name_wrong_type(rows);
        let from_parent = named.parent_pk.is_none();
        return Scored {
            address: address.to_string(),
            state: AccountState::PresentButNotGmail,
            mail: from_parent && named.mail,
            calendars: from_parent && named.calendars,
            found,
            line: Some(wrong_type_line(address, named)),
        };
    }
    let active: Vec<&LoadedRow> = google_parents
        .into_iter()
        .filter(|row| row.active)
        .collect();
    if active.is_empty() {
        return Scored {
            address: address.to_string(),
            state: AccountState::Inactive,
            mail: false,
            calendars: false,
            found,
            line: Some(format!(
                "{address} is a Gmail account but macOS marks it inactive."
            )),
        };
    }
    let chosen = closest(&active);
    let split = active.iter().any(|row| row.mail && !row.calendars)
        && active.iter().any(|row| row.calendars && !row.mail);
    if split {
        return Scored {
            address: address.to_string(),
            state: AccountState::Split,
            mail: chosen.mail,
            calendars: chosen.calendars,
            found,
            line: Some(format!(
                "{address} is on more than one Gmail account, and no single one has both Mail and Calendar on."
            )),
        };
    }
    Scored {
        address: address.to_string(),
        state: if chosen.mail {
            AccountState::CalendarOff
        } else {
            AccountState::MailOff
        },
        mail: chosen.mail,
        calendars: chosen.calendars,
        found,
        line: Some(service_line(address, chosen)),
    }
}

fn service_count(row: &LoadedRow) -> i32 {
    i32::from(row.mail) + i32::from(row.calendars)
}

fn closest<'a>(rows: &[&'a LoadedRow]) -> &'a LoadedRow {
    let mut best = rows[0];
    for row in &rows[1..] {
        let difference = service_count(row) - service_count(best);
        if difference > 0 || (difference == 0 && row.pk < best.pk) {
            best = row;
        }
    }
    best
}

fn is_google_parent(row: &LoadedRow) -> bool {
    row.identifier == GOOGLE && row.parent_pk.is_none()
}

fn to_found(row: &LoadedRow) -> Found {
    Found {
        description: row.description.clone(),
        identifier: row.identifier.clone(),
        parent: row.parent_pk.is_none(),
        active: row.active,
        mail: row.mail,
        calendars: row.calendars,
    }
}

fn name_wrong_type(rows: &[LoadedRow]) -> &LoadedRow {
    let parents: Vec<&LoadedRow> = rows.iter().filter(|row| row.parent_pk.is_none()).collect();
    let pool: Vec<&LoadedRow> = if parents.is_empty() {
        rows.iter().collect()
    } else {
        parents
    };
    let with_service: Vec<&LoadedRow> = pool
        .iter()
        .copied()
        .filter(|row| row.mail || row.calendars)
        .collect();
    if with_service.is_empty() {
        closest(&pool)
    } else {
        closest(&with_service)
    }
}

fn wrong_type_line(address: &str, named: &LoadedRow) -> String {
    if named.parent_pk.is_none() && named.mail && named.calendars {
        format!(
            "{address} is on this Mac as {} ({}), and Mail and Calendar are on for that account. That does not count. Add {address} again and choose Google. The account type has to be Gmail.",
            named.description, named.identifier
        )
    } else {
        format!(
            "{address} is on this Mac as {} ({}), not as a Gmail account. Add it again and choose Google.",
            named.description, named.identifier
        )
    }
}

fn service_line(address: &str, row: &LoadedRow) -> String {
    if !row.mail && !row.calendars {
        format!("{address} is a Gmail account, but Mail and Calendar are off.")
    } else if !row.mail {
        format!("{address} is a Gmail account, but Mail is off.")
    } else {
        format!("{address} is a Gmail account, but Calendar is off.")
    }
}

fn missing_account(address: &str) -> Scored {
    Scored {
        address: address.to_string(),
        state: AccountState::Missing,
        mail: false,
        calendars: false,
        found: Vec::new(),
        line: Some(format!(
            "{address} is not an Internet Account on this Mac. Add it and choose Google. Turn Mail and Calendar on."
        )),
    }
}

fn scored_report(scored: Vec<Scored>) -> Report {
    let ok = scored
        .iter()
        .all(|account| account.state == AccountState::Ok);
    let lines = scored
        .iter()
        .filter_map(|account| account.line.clone())
        .collect();
    let accounts = scored
        .into_iter()
        .map(|account| Account {
            address: account.address,
            state: account.state,
            mail: account.mail,
            calendars: account.calendars,
            found: account.found,
        })
        .collect();
    Report {
        ok,
        status: if ok {
            ReportStatus::Ok
        } else {
            ReportStatus::Gaps
        },
        title: TITLE.to_string(),
        lines,
        accounts,
        reason: None,
    }
}

fn schema_report(reason: impl Into<String>) -> Report {
    let reason = reason.into();
    Report {
        status: ReportStatus::SchemaMismatch,
        ok: false,
        title: TITLE.to_string(),
        lines: vec![format!(
            "The Accounts database is not in the shape this check understands ({reason}). Nothing was treated as missing. System Settings was not opened."
        )],
        accounts: Vec::new(),
        reason: Some(reason),
    }
}

fn unreadable(code: &str) -> Report {
    Report {
        status: ReportStatus::Unreadable,
        ok: false,
        title: TITLE.to_string(),
        lines: vec![format!(
            "The Accounts database could not be read ({code}). The accounts were not treated as missing."
        )],
        accounts: Vec::new(),
        reason: Some(code.to_string()),
    }
}

fn io_code(err: &io::Error) -> String {
    match err.kind() {
        io::ErrorKind::PermissionDenied => "EACCES".to_string(),
        io::ErrorKind::NotFound => "ENOENT".to_string(),
        _ => "EIO".to_string(),
    }
}

fn report_for_sqlite(err: rusqlite::Error) -> Report {
    let name = err
        .sqlite_error_code()
        .map(sqlite_name)
        .unwrap_or("SQLITE_ERROR");
    if UNREADABLE.contains(&name) {
        unreadable(name)
    } else {
        schema_report(name)
    }
}

fn sqlite_name(code: ErrorCode) -> &'static str {
    match code {
        ErrorCode::InternalMalfunction => "SQLITE_INTERNAL",
        ErrorCode::PermissionDenied => "SQLITE_PERM",
        ErrorCode::OperationAborted => "SQLITE_ABORT",
        ErrorCode::DatabaseBusy => "SQLITE_BUSY",
        ErrorCode::DatabaseLocked => "SQLITE_LOCKED",
        ErrorCode::OutOfMemory => "SQLITE_NOMEM",
        ErrorCode::ReadOnly => "SQLITE_READONLY",
        ErrorCode::OperationInterrupted => "SQLITE_INTERRUPT",
        ErrorCode::SystemIoFailure => "SQLITE_IOERR",
        ErrorCode::DatabaseCorrupt => "SQLITE_CORRUPT",
        ErrorCode::NotFound => "SQLITE_NOTFOUND",
        ErrorCode::DiskFull => "SQLITE_FULL",
        ErrorCode::CannotOpen => "SQLITE_CANTOPEN",
        ErrorCode::FileLockingProtocolFailed => "SQLITE_PROTOCOL",
        ErrorCode::SchemaChanged => "SQLITE_SCHEMA",
        ErrorCode::TooBig => "SQLITE_TOOBIG",
        ErrorCode::ConstraintViolation => "SQLITE_CONSTRAINT",
        ErrorCode::TypeMismatch => "SQLITE_MISMATCH",
        ErrorCode::ApiMisuse => "SQLITE_MISUSE",
        ErrorCode::NoLargeFileSupport => "SQLITE_NOLFS",
        ErrorCode::AuthorizationForStatementDenied => "SQLITE_AUTH",
        ErrorCode::ParameterOutOfRange => "SQLITE_RANGE",
        ErrorCode::NotADatabase => "SQLITE_NOTADB",
        ErrorCode::Unknown => "SQLITE_ERROR",
        _ => "SQLITE_ERROR",
    }
}

fn write_prompt(out: &mut dyn Write, report: &Report, open_error: Option<&str>) {
    let _ = writeln!(out, "{}", report.title);
    if report.lines.is_empty() {
        let _ = writeln!(out, "{}", crate::checkpoint::EMPTY_REPORT);
    }
    for line in &report.lines {
        let _ = writeln!(out, "{line}");
    }
    if let Some(error) = open_error {
        let _ = writeln!(out, "{error}");
    }
    let _ = writeln!(out, "Press Enter to check again.");
    let _ = out.flush();
}

fn write_failure(err: &mut dyn Write, report: &Report, open_error: Option<&str>) {
    let lines = if report.lines.is_empty() {
        vec![crate::checkpoint::EMPTY_REPORT.to_string()]
    } else {
        report.lines.clone()
    };
    for line in lines {
        let _ = writeln!(err, "{line}");
    }
    if let Some(error) = open_error {
        let _ = writeln!(err, "{error}");
    }
}

struct Element {
    name: String,
    text: String,
    children: Vec<Element>,
}

/// Read a dataclass identifier from an XML property list. `None` when the
/// archive is not the shape this check accepts.
pub(crate) fn dataclass_identifier_from_xml(xml: &str) -> Option<String> {
    let root = PlistReader::new(strip_preamble(xml)).read_document()?;
    if root.name != "plist" || root.children.len() != 1 {
        return None;
    }
    let dict = &root.children[0];
    if dict.name != "dict" {
        return None;
    }
    let entries = dict_entries(dict)?;
    if entries.len() != 4 {
        return None;
    }
    for key in ["$archiver", "$objects", "$top", "$version"] {
        if !entries.contains_key(key) {
            return None;
        }
    }
    if string_value(entries.get("$archiver").copied())? != "NSKeyedArchiver" {
        return None;
    }
    if integer_value(entries.get("$version").copied())? != 100_000 {
        return None;
    }
    let objects = entries.get("$objects").copied()?;
    if objects.name != "array" || objects.children.len() != 2 {
        return None;
    }
    if string_value(Some(&objects.children[0]))? != "$null" {
        return None;
    }
    let identifier = string_value(Some(&objects.children[1]))?;
    if !dataclass_id_ok(&identifier) {
        return None;
    }
    let top = entries.get("$top").copied()?;
    if top.name != "dict" {
        return None;
    }
    let top_entries = dict_entries(top)?;
    if top_entries.len() != 1 || !top_entries.contains_key("root") {
        return None;
    }
    let root_uid = top_entries.get("root")?;
    if root_uid.name != "dict" {
        return None;
    }
    let uid_entries = dict_entries(root_uid)?;
    if uid_entries.len() != 1 || integer_value(uid_entries.get("CF$UID").copied())? != 1 {
        return None;
    }
    Some(identifier)
}

fn dataclass_id_ok(value: &str) -> bool {
    let Some(rest) = value.strip_prefix("com.apple.Dataclass.") else {
        return false;
    };
    !rest.is_empty() && rest.bytes().all(|byte| byte.is_ascii_alphanumeric())
}

fn dict_entries(element: &Element) -> Option<HashMap<&str, &Element>> {
    if !element.children.len().is_multiple_of(2) {
        return None;
    }
    let mut entries = HashMap::new();
    for pair in element.children.chunks(2) {
        let key = &pair[0];
        let value = &pair[1];
        if key.name != "key" || !key.children.is_empty() {
            return None;
        }
        if entries.contains_key(key.text.as_str()) {
            return None;
        }
        entries.insert(key.text.as_str(), value);
    }
    Some(entries)
}

fn integer_value(element: Option<&Element>) -> Option<i64> {
    let element = element?;
    if element.name != "integer" || !element.children.is_empty() {
        return None;
    }
    let text = element.text.trim();
    if text.is_empty()
        || !text.bytes().enumerate().all(|(index, byte)| {
            byte.is_ascii_digit() || (index == 0 && byte == b'-' && text.len() > 1)
        })
    {
        return None;
    }
    let value: i64 = text.parse().ok()?;
    const MAX: i64 = 9_007_199_254_740_991;
    if !(-MAX..=MAX).contains(&value) {
        return None;
    }
    Some(value)
}

fn string_value(element: Option<&Element>) -> Option<String> {
    let element = element?;
    if element.name != "string" || !element.children.is_empty() {
        return None;
    }
    Some(element.text.clone())
}

fn strip_preamble(input: &str) -> String {
    let input = input.strip_prefix('\u{feff}').unwrap_or(input);
    let bytes = input.as_bytes();
    let mut out = String::with_capacity(input.len());
    let mut index = 0;
    let mut copy_from = 0;
    while index < bytes.len() {
        if bytes[index] == b'<'
            && let Some(end) = xml_decl_end(bytes, index).or_else(|| doctype_end(bytes, index))
        {
            out.push_str(&input[copy_from..index]);
            index = end;
            copy_from = end;
            continue;
        }
        let Some(ch) = input[index..].chars().next() else {
            break;
        };
        index += ch.len_utf8();
    }
    out.push_str(&input[copy_from..]);
    out
}

fn xml_decl_end(bytes: &[u8], start: usize) -> Option<usize> {
    let rest = &bytes[start..];
    if rest.len() < 5 || !rest[..5].eq_ignore_ascii_case(b"<?xml") {
        return None;
    }
    let after = &rest[5..];
    let close = after.windows(2).position(|window| window == b"?>")?;
    Some(start + 5 + close + 2)
}

fn doctype_end(bytes: &[u8], start: usize) -> Option<usize> {
    let rest = &bytes[start..];
    const PREFIX: &[u8] = b"<!DOCTYPE";
    if rest.len() < PREFIX.len() + 1 || !rest[..PREFIX.len()].eq_ignore_ascii_case(PREFIX) {
        return None;
    }
    if !rest[PREFIX.len()].is_ascii_whitespace() {
        return None;
    }
    let mut index = PREFIX.len();
    while index < rest.len() && rest[index] != b'[' && rest[index] != b'>' {
        index += 1;
    }
    if index >= rest.len() {
        return None;
    }
    if rest[index] == b'>' {
        return Some(start + index + 1);
    }
    let subset = &rest[index + 1..];
    let mut cursor = 0;
    while cursor < subset.len() {
        if subset[cursor] == b']' {
            let mut end = cursor + 1;
            while end < subset.len() && subset[end].is_ascii_whitespace() {
                end += 1;
            }
            if subset.get(end) == Some(&b'>') {
                return Some(start + index + 1 + end + 1);
            }
        }
        cursor += 1;
    }
    None
}

struct PlistReader {
    source: String,
    index: usize,
}

impl PlistReader {
    fn new(source: String) -> Self {
        Self { source, index: 0 }
    }

    fn read_document(&mut self) -> Option<Element> {
        self.skip_space();
        let root = self.read_element()?;
        self.skip_space();
        if self.index == self.source.len() {
            Some(root)
        } else {
            None
        }
    }

    fn rest(&self) -> &str {
        &self.source[self.index..]
    }

    fn peek(&self) -> Option<char> {
        self.rest().chars().next()
    }

    fn skip_space(&mut self) {
        while self.peek().is_some_and(char::is_whitespace) {
            self.bump();
        }
    }

    fn bump(&mut self) {
        if let Some(ch) = self.peek() {
            self.index += ch.len_utf8();
        }
    }

    fn read_name(&mut self) -> Option<String> {
        let start = self.index;
        while self
            .peek()
            .is_some_and(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | ':' | '-'))
        {
            self.bump();
        }
        if self.index > start {
            Some(self.source[start..self.index].to_string())
        } else {
            None
        }
    }

    fn skip_attributes(&mut self) -> bool {
        loop {
            let Some(ch) = self.peek() else {
                return false;
            };
            if ch == '>' || (ch == '/' && self.rest().starts_with("/>")) {
                return true;
            }
            if ch == '"' || ch == '\'' {
                self.bump();
                while self.peek().is_some_and(|next| next != ch) {
                    self.bump();
                }
                if self.peek() != Some(ch) {
                    return false;
                }
                self.bump();
                continue;
            }
            self.bump();
        }
    }

    fn read_close(&mut self, name: &str) -> bool {
        if !self.rest().starts_with("</") {
            return false;
        }
        self.index += 2;
        let Some(got) = self.read_name() else {
            return false;
        };
        if got != name {
            return false;
        }
        self.skip_space();
        if self.peek() != Some('>') {
            return false;
        }
        self.bump();
        true
    }

    fn read_text(&mut self) -> Option<String> {
        let start = self.index;
        while self.peek().is_some_and(|ch| ch != '<') {
            self.bump();
        }
        decode_entities(&self.source[start..self.index])
    }

    fn read_element(&mut self) -> Option<Element> {
        self.skip_space();
        if self.peek() != Some('<') {
            return None;
        }
        self.bump();
        let marker = self.peek()?;
        if matches!(marker, '/' | '!' | '?') {
            return None;
        }
        let name = self.read_name()?;
        if !self.skip_attributes() {
            return None;
        }
        if self.rest().starts_with("/>") {
            self.index += 2;
            return Some(Element {
                name,
                text: String::new(),
                children: Vec::new(),
            });
        }
        if self.peek() != Some('>') {
            return None;
        }
        self.bump();
        if matches!(name.as_str(), "string" | "integer" | "key") {
            let text = self.read_text()?;
            if !self.read_close(&name) {
                return None;
            }
            return Some(Element {
                name,
                text,
                children: Vec::new(),
            });
        }
        let mut children = Vec::new();
        loop {
            self.skip_space();
            if self.rest().starts_with("</") {
                if !self.read_close(&name) {
                    return None;
                }
                return Some(Element {
                    name,
                    text: String::new(),
                    children,
                });
            }
            children.push(self.read_element()?);
        }
    }
}

fn decode_entities(raw: &str) -> Option<String> {
    let mut out = String::new();
    let mut rest = raw;
    while let Some(start) = rest.find('&') {
        out.push_str(&rest[..start]);
        let after = &rest[start + 1..];
        let end = after.find(';')?;
        let body = &after[..end];
        let value = if let Some(hex) = body.strip_prefix("#x").or_else(|| body.strip_prefix("#X")) {
            let code = u32::from_str_radix(hex, 16).ok()?;
            char::from_u32(code)?
        } else if let Some(decimal) = body.strip_prefix('#') {
            let code: u32 = decimal.parse().ok()?;
            char::from_u32(code)?
        } else {
            ENTITIES
                .iter()
                .find(|(name, _)| *name == body)
                .map(|(_, ch)| *ch)?
        };
        out.push(value);
        rest = &after[end + 1..];
    }
    out.push_str(rest);
    Some(out)
}

#[cfg(test)]
mod tests {
    use std::io::Write;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicUsize, Ordering};

    use rusqlite::{Connection, params};
    use serde_json::Value;

    use super::*;

    const APPLE: &str = "com.apple.account.AppleAccount";
    const FORBIDDEN: &[&str] = &[
        "ZACCOUNTPROPERTY",
        "ZCREDENTIALITEM",
        "ZAUTHORIZATION",
        "ZDATACLASSPROPERTIES",
        "Z_2PROVISIONEDDATACLASSES",
    ];

    const MAIL_XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
	<key>$archiver</key>
	<string>NSKeyedArchiver</string>
	<key>$objects</key>
	<array>
		<string>$null</string>
		<string>com.apple.Dataclass.Mail</string>
	</array>
	<key>$top</key>
	<dict>
		<key>root</key>
		<dict>
			<key>CF$UID</key>
			<integer>1</integer>
		</dict>
	</dict>
	<key>$version</key>
	<integer>100000</integer>
</dict>
</plist>
"#;

    const CALENDARS_XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
	<key>$version</key>
	<integer>100000</integer>
	<key>$top</key>
	<dict>
		<key>root</key>
		<dict>
			<key>CF$UID</key>
			<integer>1</integer>
		</dict>
	</dict>
	<key>$objects</key>
	<array>
		<string>$null</string>
		<string>com.apple.Dataclass.Calendars</string>
	</array>
	<key>$archiver</key>
	<string>NSKeyedArchiver</string>
</dict>
</plist>
"#;

    enum ClassName {
        Blob(Vec<u8>),
        Text(String),
    }

    struct AccountRow {
        pk: i64,
        username: &'static str,
        type_pk: i64,
        active: i64,
        parent: Option<i64>,
        services: &'static [&'static str],
    }

    struct Built {
        dir: tempfile::TempDir,
        path: PathBuf,
    }

    struct Options {
        types: Vec<(i64, &'static str, &'static str)>,
        classes: Vec<(i64, ClassName)>,
        accounts: Vec<AccountRow>,
        drop_join: bool,
        drop_description: bool,
    }

    fn options() -> Options {
        Options {
            types: vec![(1, GOOGLE, "Gmail")],
            classes: vec![
                (1, ClassName::Blob(MAIL_XML.as_bytes().to_vec())),
                (2, ClassName::Blob(CALENDARS_XML.as_bytes().to_vec())),
            ],
            accounts: Vec::new(),
            drop_join: false,
            drop_description: false,
        }
    }

    fn account(pk: i64, username: &'static str) -> AccountRow {
        AccountRow {
            pk,
            username,
            type_pk: 1,
            active: 1,
            parent: None,
            services: &["mail", "calendars"],
        }
    }

    fn build(options: Options) -> Built {
        let dir = tempfile::tempdir().expect("temp");
        let path = dir.path().join("accounts.sqlite");
        let conn = Connection::open(&path).expect("create");
        conn.execute_batch(
            "CREATE TABLE ZACCOUNT (
                Z_PK INTEGER PRIMARY KEY,
                ZUSERNAME TEXT,
                ZACTIVE INTEGER,
                ZPARENTACCOUNT INTEGER,
                ZACCOUNTTYPE INTEGER,
                ZEXTRA TEXT
            );",
        )
        .expect("account table");
        if options.drop_description {
            conn.execute_batch(
                "CREATE TABLE ZACCOUNTTYPE (Z_PK INTEGER PRIMARY KEY, ZIDENTIFIER TEXT);",
            )
            .expect("type table");
        } else {
            conn.execute_batch(
                "CREATE TABLE ZACCOUNTTYPE (
                    Z_PK INTEGER PRIMARY KEY,
                    ZIDENTIFIER TEXT,
                    ZACCOUNTTYPEDESCRIPTION TEXT
                );",
            )
            .expect("type table");
        }
        conn.execute_batch("CREATE TABLE ZDATACLASS (Z_PK INTEGER PRIMARY KEY, ZNAME BLOB);")
            .expect("class table");
        if !options.drop_join {
            conn.execute_batch(
                "CREATE TABLE Z_2ENABLEDDATACLASSES (
                    Z_2ENABLEDACCOUNTS INTEGER,
                    Z_7ENABLEDDATACLASSES INTEGER
                );",
            )
            .expect("join table");
        }
        for (pk, identifier, description) in &options.types {
            if options.drop_description {
                conn.execute(
                    "INSERT INTO ZACCOUNTTYPE (Z_PK, ZIDENTIFIER) VALUES (?1, ?2)",
                    params![pk, identifier],
                )
                .expect("type");
            } else {
                conn.execute(
                    "INSERT INTO ZACCOUNTTYPE (Z_PK, ZIDENTIFIER, ZACCOUNTTYPEDESCRIPTION) VALUES (?1, ?2, ?3)",
                    params![pk, identifier, description],
                )
                .expect("type");
            }
        }
        for (pk, name) in &options.classes {
            match name {
                ClassName::Blob(bytes) => conn.execute(
                    "INSERT INTO ZDATACLASS (Z_PK, ZNAME) VALUES (?1, ?2)",
                    params![pk, bytes],
                ),
                ClassName::Text(text) => conn.execute(
                    "INSERT INTO ZDATACLASS (Z_PK, ZNAME) VALUES (?1, ?2)",
                    params![pk, text],
                ),
            }
            .expect("class");
        }
        for row in &options.accounts {
            conn.execute(
                "INSERT INTO ZACCOUNT (Z_PK, ZUSERNAME, ZACTIVE, ZPARENTACCOUNT, ZACCOUNTTYPE, ZEXTRA)
                 VALUES (?1, ?2, ?3, ?4, ?5, NULL)",
                params![row.pk, row.username, row.active, row.parent, row.type_pk],
            )
            .expect("account");
            if options.drop_join {
                continue;
            }
            for service in row.services {
                let class = if *service == "mail" { 1 } else { 2 };
                conn.execute(
                    "INSERT INTO Z_2ENABLEDDATACLASSES (Z_2ENABLEDACCOUNTS, Z_7ENABLEDDATACLASSES) VALUES (?1, ?2)",
                    params![row.pk, class],
                )
                .expect("service");
            }
        }
        let mut names = Vec::new();
        let mut stmt = conn
            .prepare("SELECT name FROM sqlite_master")
            .expect("names");
        let rows = stmt
            .query_map([], |row| row.get::<_, String>(0))
            .expect("query");
        for name in rows {
            names.push(name.expect("name"));
        }
        drop(stmt);
        for name in FORBIDDEN {
            assert!(!names.iter().any(|got| got == name), "{name}");
        }
        drop(conn);
        assert!(
            !path.ends_with("Library/Accounts/Accounts4.sqlite"),
            "fixture must not be the user database"
        );
        Built { dir, path }
    }

    fn checked(path: &Path, accounts: &str) -> Report {
        let accounts = parse_accounts(accounts).expect("addresses");
        let report = check(path, &accounts);
        let json = serde_json::to_string(&report).expect("json");
        assert!(!json.contains("essential-mise-checkpoint:"), "{json}");
        assert_eq!(report.title, TITLE);
        assert_eq!(report.ok, report.status == ReportStatus::Ok);
        for line in &report.lines {
            assert!(!line.contains("essential-mise-checkpoint:"));
        }
        reject_secret_keys(&serde_json::from_str(&json).expect("value"));
        report
    }

    fn reject_secret_keys(value: &Value) {
        match value {
            Value::Array(items) => {
                for item in items {
                    reject_secret_keys(item);
                }
            }
            Value::Object(map) => {
                for (key, child) in map {
                    let lower = key.to_ascii_lowercase();
                    assert!(
                        !lower.contains("credential")
                            && !lower.contains("password")
                            && !lower.contains("token")
                            && !lower.contains("authorization")
                            && !lower.contains("secret"),
                        "{key}"
                    );
                    reject_secret_keys(child);
                }
            }
            _ => {}
        }
    }

    fn one(report: &Report) -> &Account {
        assert_eq!(report.accounts.len(), 1);
        &report.accounts[0]
    }

    fn archive(identifier: &str) -> Vec<u8> {
        MAIL_XML.replace(MAIL, identifier).into_bytes()
    }

    #[test]
    fn accepts_the_mail_archive_and_a_key_order_variant() {
        assert_eq!(
            dataclass_identifier_from_xml(MAIL_XML).as_deref(),
            Some(MAIL)
        );
        assert_eq!(
            dataclass_identifier_from_xml(CALENDARS_XML).as_deref(),
            Some(CALENDARS)
        );
        let uid_zero = MAIL_XML.replace("<integer>1</integer>", "<integer>0</integer>");
        assert!(dataclass_identifier_from_xml(&uid_zero).is_none());
        assert!(
            dataclass_identifier_from_xml(
                "<plist version=\"1.0\"><string>com.apple.Dataclass.Mail</string></plist>"
            )
            .is_none()
        );
        let encoded = MAIL_XML.replace("com.apple.Dataclass.Mail", "com.apple.Dataclass.Mai&#108;");
        assert_eq!(
            dataclass_identifier_from_xml(&encoded).as_deref(),
            Some(MAIL)
        );
    }

    #[test]
    fn three_google_parents_with_both_services_are_ok() {
        let mut spec = options();
        spec.accounts = vec![
            account(1, "one@example.com"),
            account(2, "two@example.com"),
            account(3, "three@example.com"),
        ];
        let built = build(spec);
        let report = checked(
            &built.path,
            "one@example.com,two@example.com,three@example.com",
        );
        assert_eq!(report.status, ReportStatus::Ok);
        assert!(report.lines.is_empty());
        assert!(
            report
                .accounts
                .iter()
                .all(|account| account.state == AccountState::Ok
                    && account.mail
                    && account.calendars)
        );
        assert_eq!(report.accounts[0].found[0].description, "Gmail");
        let json = serde_json::to_string(&report).expect("json");
        assert!(!json.contains("\"reason\""));
    }

    #[test]
    fn a_catalog_description_stays_out_of_the_lines() {
        let mut spec = options();
        spec.types = vec![(1, GOOGLE, "Courrier")];
        spec.accounts = vec![account(1, "one@example.com")];
        let built = build(spec);
        let report = checked(&built.path, "one@example.com");
        assert_eq!(report.status, ReportStatus::Ok);
        assert_eq!(one(&report).found[0].description, "Courrier");
        assert!(!report.lines.join("\n").contains("Courrier"));
    }

    #[test]
    fn courrier_with_mail_off_uses_the_english_mail_sentence() {
        let mut spec = options();
        spec.types = vec![(1, GOOGLE, "Courrier")];
        let mut row = account(1, "one@example.com");
        row.services = &["calendars"];
        spec.accounts = vec![row];
        let built = build(spec);
        let report = checked(&built.path, "one@example.com");
        let account = one(&report);
        assert_eq!(report.status, ReportStatus::Gaps);
        assert_eq!(account.state, AccountState::MailOff);
        assert_eq!(account.found[0].description, "Courrier");
        assert_eq!(
            report.lines,
            vec!["one@example.com is a Gmail account, but Mail is off.".to_string()]
        );
        assert!(!report.lines.join("\n").contains("Courrier"));
    }

    #[test]
    fn an_icloud_parent_with_both_services_does_not_count() {
        let mut spec = options();
        spec.types = vec![(1, GOOGLE, "Gmail"), (2, APPLE, "iCloud")];
        let mut row = account(1, "one@example.com");
        row.type_pk = 2;
        spec.accounts = vec![row];
        let built = build(spec);
        let report = checked(&built.path, "one@example.com");
        let account = one(&report);
        assert_eq!(account.state, AccountState::PresentButNotGmail);
        assert!(account.mail && account.calendars);
        assert_eq!(
            report.lines,
            vec![
                "one@example.com is on this Mac as iCloud (com.apple.account.AppleAccount), and Mail and Calendar are on for that account. That does not count. Add one@example.com again and choose Google. The account type has to be Gmail."
                    .to_string()
            ]
        );
    }

    #[test]
    fn a_qualifying_google_parent_wins_over_icloud_on_the_same_address() {
        let mut spec = options();
        spec.types = vec![(1, GOOGLE, "Gmail"), (2, APPLE, "iCloud")];
        let mut icloud = account(1, "one@example.com");
        icloud.type_pk = 2;
        let mut google = account(2, "one@example.com");
        google.type_pk = 1;
        spec.accounts = vec![icloud, google];
        let built = build(spec);
        let report = checked(&built.path, "one@example.com");
        assert_eq!(one(&report).state, AccountState::Ok);
        let mut identifiers: Vec<&str> = one(&report)
            .found
            .iter()
            .map(|row| row.identifier.as_str())
            .collect();
        identifiers.sort_unstable();
        assert_eq!(identifiers, vec![APPLE, GOOGLE]);
    }

    #[test]
    fn a_missing_address_is_a_gap() {
        let built = build(options());
        let report = checked(&built.path, "one@example.com");
        let account = one(&report);
        assert_eq!(account.state, AccountState::Missing);
        assert!(!account.mail && !account.calendars);
        assert_eq!(
            report.lines,
            vec![
                "one@example.com is not an Internet Account on this Mac. Add it and choose Google. Turn Mail and Calendar on."
                    .to_string()
            ]
        );
    }

    #[test]
    fn mail_calendar_and_both_off_each_name_the_services_that_are_off() {
        let mut spec = options();
        let mut mail_off = account(1, "mail-off@example.com");
        mail_off.services = &["calendars"];
        let mut calendar_off = account(2, "calendar-off@example.com");
        calendar_off.services = &["mail"];
        let mut both_off = account(3, "both-off@example.com");
        both_off.services = &[];
        spec.accounts = vec![mail_off, calendar_off, both_off];
        let built = build(spec);
        let report = checked(
            &built.path,
            "mail-off@example.com,calendar-off@example.com,both-off@example.com",
        );
        assert_eq!(
            report
                .accounts
                .iter()
                .map(|account| account.state)
                .collect::<Vec<_>>(),
            vec![
                AccountState::MailOff,
                AccountState::CalendarOff,
                AccountState::MailOff
            ]
        );
        assert_eq!(
            report
                .accounts
                .iter()
                .map(|account| (account.mail, account.calendars))
                .collect::<Vec<_>>(),
            vec![(false, true), (true, false), (false, false)]
        );
        assert_eq!(
            report.lines,
            vec![
                "mail-off@example.com is a Gmail account, but Mail is off.".to_string(),
                "calendar-off@example.com is a Gmail account, but Calendar is off.".to_string(),
                "both-off@example.com is a Gmail account, but Mail and Calendar are off."
                    .to_string(),
            ]
        );
    }

    #[test]
    fn mail_on_one_google_parent_and_calendar_on_another_is_a_split() {
        let mut spec = options();
        let mut calendars = account(7, "one@example.com");
        calendars.services = &["calendars"];
        let mut mail = account(2, "one@example.com");
        mail.services = &["mail"];
        spec.accounts = vec![calendars, mail];
        let built = build(spec);
        let report = checked(&built.path, "one@example.com");
        let account = one(&report);
        assert_eq!(account.state, AccountState::Split);
        assert!(account.mail && !account.calendars);
        assert_eq!(
            report.lines,
            vec![
                "one@example.com is on more than one Gmail account, and no single one has both Mail and Calendar on."
                    .to_string()
            ]
        );
        assert!(!report.lines.join("\n").contains("off"));
    }

    #[test]
    fn two_google_parents_with_only_mail_take_calendar_off_from_the_lowest_row() {
        let mut spec = options();
        let mut inactive = account(1, "one@example.com");
        inactive.active = 0;
        let mut higher = account(8, "one@example.com");
        higher.services = &["mail"];
        let mut lower = account(4, "one@example.com");
        lower.services = &["mail"];
        spec.accounts = vec![inactive, higher, lower];
        let built = build(spec);
        let report = checked(&built.path, "one@example.com");
        let account = one(&report);
        assert_eq!(account.state, AccountState::CalendarOff);
        assert!(account.mail && !account.calendars);
        assert_eq!(
            report.lines,
            vec!["one@example.com is a Gmail account, but Calendar is off.".to_string()]
        );
    }

    #[test]
    fn an_inactive_google_parent_is_inactive() {
        let mut spec = options();
        let mut row = account(1, "one@example.com");
        row.active = 0;
        spec.accounts = vec![row];
        let built = build(spec);
        let report = checked(&built.path, "one@example.com");
        let account = one(&report);
        assert_eq!(account.state, AccountState::Inactive);
        assert!(!account.mail && !account.calendars);
        assert!(account.found[0].mail && account.found[0].calendars);
        assert_eq!(
            report.lines,
            vec!["one@example.com is a Gmail account but macOS marks it inactive.".to_string()]
        );
    }

    #[test]
    fn a_caldav_child_does_not_qualify() {
        let mut spec = options();
        spec.types = vec![
            (1, GOOGLE, "Gmail"),
            (3, "com.apple.account.CalDAV", "CalDAV"),
        ];
        let mut row = account(5, "one@example.com");
        row.type_pk = 3;
        row.parent = Some(99);
        row.services = &["calendars"];
        spec.accounts = vec![row];
        let built = build(spec);
        let report = checked(&built.path, "one@example.com");
        let account = one(&report);
        assert_eq!(account.state, AccountState::PresentButNotGmail);
        assert!(!account.mail && !account.calendars);
        assert_eq!(
            report.lines,
            vec![
                "one@example.com is on this Mac as CalDAV (com.apple.account.CalDAV), not as a Gmail account. Add it again and choose Google."
                    .to_string()
            ]
        );
    }

    #[test]
    fn username_case_differs_and_still_matches() {
        let mut spec = options();
        spec.accounts = vec![account(1, "One@Example.com")];
        let built = build(spec);
        assert_eq!(
            one(&checked(&built.path, "one@example.com")).state,
            AccountState::Ok
        );
        let mixed = checked(&built.path, "One@Example.com");
        assert_eq!(one(&mixed).state, AccountState::Ok);
        assert_eq!(one(&mixed).address, "one@example.com");
    }

    #[test]
    fn a_longer_address_that_contains_the_required_address_does_not_match() {
        let mut spec = options();
        spec.accounts = vec![account(1, "one@example.com.extra")];
        let built = build(spec);
        let report = checked(&built.path, "one@example.com");
        assert_eq!(one(&report).state, AccountState::Missing);
        assert!(one(&report).found.is_empty());
    }

    #[test]
    fn the_captured_archives_are_accepted() {
        let mut spec = options();
        spec.accounts = vec![account(1, "one@example.com")];
        let built = build(spec);
        let report = checked(&built.path, "one@example.com");
        assert_eq!(report.status, ReportStatus::Ok);
        assert!(one(&report).mail && one(&report).calendars);
    }

    #[test]
    fn a_dropped_join_table_is_a_schema_mismatch() {
        let mut spec = options();
        spec.drop_join = true;
        spec.accounts = vec![account(1, "one@example.com")];
        let built = build(spec);
        let report = checked(&built.path, "one@example.com");
        assert_eq!(report.status, ReportStatus::SchemaMismatch);
        assert_eq!(
            report.reason.as_deref(),
            Some("missing-table:Z_2ENABLEDDATACLASSES")
        );
        assert!(report.accounts.is_empty());
        assert!(report.lines[0].contains("Nothing was treated as missing"));
    }

    #[test]
    fn a_plain_string_dataclass_name_is_a_schema_mismatch() {
        let mut spec = options();
        spec.classes = vec![
            (1, ClassName::Text(MAIL.to_string())),
            (2, ClassName::Blob(CALENDARS_XML.as_bytes().to_vec())),
        ];
        spec.accounts = vec![account(1, "one@example.com")];
        let built = build(spec);
        let report = checked(&built.path, "one@example.com");
        assert_eq!(report.reason.as_deref(), Some("dataclass-archive"));
        assert!(report.accounts.is_empty());
    }

    #[test]
    fn mail_something_does_not_count_as_mail() {
        let mut spec = options();
        spec.classes = vec![
            (
                1,
                ClassName::Blob(archive("com.apple.Dataclass.MailSomething")),
            ),
            (2, ClassName::Blob(CALENDARS_XML.as_bytes().to_vec())),
        ];
        spec.accounts = vec![account(1, "one@example.com")];
        let built = build(spec);
        let report = checked(&built.path, "one@example.com");
        assert_eq!(report.reason.as_deref(), Some("dataclass-catalog"));
        assert!(report.accounts.is_empty());
        assert!(!report.lines.join("\n").contains("one@example.com"));
    }

    #[test]
    fn zero_or_two_google_catalog_rows_are_a_schema_mismatch() {
        let mut none = options();
        none.types = vec![(2, APPLE, "iCloud")];
        let mut row = account(1, "one@example.com");
        row.type_pk = 2;
        none.accounts = vec![row];
        let built = build(none);
        let report = checked(&built.path, "one@example.com");
        assert_eq!(report.reason.as_deref(), Some("google-type"));
        assert!(report.accounts.is_empty());

        let mut two = options();
        two.types = vec![(1, GOOGLE, "Gmail"), (2, GOOGLE, "Gmail")];
        two.accounts = vec![account(1, "one@example.com")];
        let built = build(two);
        let report = checked(&built.path, "one@example.com");
        assert_eq!(report.reason.as_deref(), Some("google-type"));
        assert!(report.accounts.is_empty());
    }

    #[test]
    fn a_missing_description_column_is_a_schema_mismatch() {
        let mut spec = options();
        spec.drop_description = true;
        let built = build(spec);
        let report = checked(&built.path, "one@example.com");
        assert_eq!(
            report.reason.as_deref(),
            Some("missing-column:ZACCOUNTTYPE.ZACCOUNTTYPEDESCRIPTION")
        );
        assert!(report.accounts.is_empty());
    }

    #[test]
    fn a_missing_database_is_a_fresh_home_of_gaps() {
        let dir = tempfile::tempdir().expect("temp");
        let report = checked(
            &dir.path().join("missing.sqlite"),
            "one@example.com,two@example.com",
        );
        assert_eq!(report.status, ReportStatus::Gaps);
        assert_eq!(
            report
                .accounts
                .iter()
                .map(|account| account.state)
                .collect::<Vec<_>>(),
            vec![AccountState::Missing, AccountState::Missing]
        );
        assert_eq!(report.lines.len(), 2);
    }

    #[test]
    fn an_unreadable_database_is_not_scored_as_missing() {
        let dir = tempfile::tempdir().expect("temp");
        let path = dir.path().join("accounts.sqlite");
        std::fs::write(&path, "not a database").expect("write");
        let report = checked(&path, "one@example.com");
        assert_eq!(report.status, ReportStatus::Unreadable);
        assert!(report.accounts.is_empty());
        assert_eq!(
            report.lines,
            vec![
                "The Accounts database could not be read (SQLITE_NOTADB). The accounts were not treated as missing."
                    .to_string()
            ]
        );
    }

    #[test]
    fn a_directory_is_unreadable() {
        let dir = tempfile::tempdir().expect("temp");
        let report = check(dir.path(), &["one@example.com".to_string()]);
        assert_eq!(report.status, ReportStatus::Unreadable);
        assert_eq!(report.reason.as_deref(), Some("SQLITE_CANTOPEN"));
        assert!(report.accounts.is_empty());
    }

    #[test]
    fn lines_never_contain_the_checkpoint_prefix() {
        let built = build(options());
        let report = checked(&built.path, "one@example.com");
        let json = serde_json::to_string(&report).expect("json");
        assert!(!json.contains("essential-mise-checkpoint:"));
    }

    #[test]
    fn binary_plist_uses_plutil_or_reports_it_missing() {
        let mail = if Path::new(PLUTIL).is_file() {
            bplist(MAIL_XML)
        } else {
            b"bplist00".to_vec()
        };
        let calendars = if Path::new(PLUTIL).is_file() {
            bplist(CALENDARS_XML)
        } else {
            b"bplist00".to_vec()
        };
        let mut spec = options();
        spec.classes = vec![(1, ClassName::Blob(mail)), (2, ClassName::Blob(calendars))];
        spec.accounts = vec![account(1, "one@example.com")];
        let built = build(spec);
        let report = checked(&built.path, "one@example.com");
        if Path::new(PLUTIL).is_file() {
            assert_eq!(report.status, ReportStatus::Ok, "{report:?}");
        } else {
            assert_eq!(report.reason.as_deref(), Some("plutil-missing"));
        }
    }

    fn bplist(xml: &str) -> Vec<u8> {
        let mut child = std::process::Command::new(PLUTIL)
            .args(["-convert", "binary1", "-o", "-", "-"])
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .expect("plutil");
        child
            .stdin
            .take()
            .expect("stdin")
            .write_all(xml.as_bytes())
            .expect("write");
        let output = child.wait_with_output().expect("wait");
        assert!(output.status.success(), "plutil binary");
        assert!(output.stdout.starts_with(b"bplist"));
        output.stdout
    }

    #[test]
    fn open_records_the_pane_without_a_shell() {
        let dir = tempfile::tempdir().expect("temp");
        let script = dir.path().join("open-double");
        let record = dir.path().join("args");
        let quoted = record.display().to_string().replace('\'', r"'\''");
        std::fs::write(
            &script,
            format!("#!/bin/sh\nprintf '%s\\n' \"$1\" > '{quoted}'\n"),
        )
        .expect("script");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755))
                .expect("mode");
        }
        assert!(run_open(&script).is_ok());
        let got = std::fs::read_to_string(&record).expect("args");
        assert_eq!(got, format!("{ACCOUNTS_PANE}\n"));
        assert!(run_open(&script).is_ok());
        let got = std::fs::read_to_string(&record).expect("args");
        assert_eq!(got, format!("{ACCOUNTS_PANE}\n"));
    }

    #[test]
    fn guide_waits_and_two_eofs_are_not_success() {
        let built = build(options());
        let accounts = parse_accounts("one@example.com").expect("address");
        let opens = AtomicUsize::new(0);
        let mut reads = vec![Some("defer google-accounts\n".to_string()), None, None];
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let mut read_line = || {
            Ok(if reads.is_empty() {
                None
            } else {
                reads.remove(0)
            })
        };
        let mut open_pane = || {
            opens.fetch_add(1, Ordering::SeqCst);
            Ok(())
        };
        let mut ports = GuidePorts {
            read_line: &mut read_line,
            open_pane: &mut open_pane,
            stdout: &mut stdout,
            stderr: &mut stderr,
        };
        let code = guide(&built.path, &accounts, &mut ports);
        assert_eq!(code, 1);
        assert_eq!(opens.load(Ordering::SeqCst), 1);
        let err = String::from_utf8(stderr).expect("stderr");
        let out = String::from_utf8(stdout).expect("stdout");
        assert!(err.contains("one@example.com"));
        assert!(!err.contains("essential-mise-checkpoint:"));
        assert!(!out.contains("essential-mise-checkpoint:"));
        assert!(out.contains("Press Enter to check again."));
        assert!(out.matches("Press Enter to check again.").count() >= 2);
    }

    #[test]
    fn guide_opens_once_on_the_transition_into_gaps_and_then_can_pass() {
        let mut schema = options();
        schema.drop_join = true;
        let built = build(schema);
        let accounts = parse_accounts("one@example.com").expect("address");
        let opens = AtomicUsize::new(0);
        let path = built.path.clone();
        let dir = built.dir.path().to_path_buf();
        let mut stage = 0;
        let mut read_line = move || {
            stage += 1;
            if stage == 1 {
                let replacement = gap_db(&dir);
                std::fs::copy(&replacement, &path).expect("replace");
                return Ok(Some("\n".to_string()));
            }
            if stage == 2 {
                return Ok(Some("defer google-accounts\n".to_string()));
            }
            let replacement = ok_db(&dir);
            std::fs::copy(&replacement, &path).expect("replace");
            Ok(Some("\n".to_string()))
        };
        let mut open_pane = || {
            opens.fetch_add(1, Ordering::SeqCst);
            Ok(())
        };
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let mut ports = GuidePorts {
            read_line: &mut read_line,
            open_pane: &mut open_pane,
            stdout: &mut stdout,
            stderr: &mut stderr,
        };
        let code = guide(&built.path, &accounts, &mut ports);
        assert_eq!(code, 0, "{}", String::from_utf8_lossy(&stderr));
        assert_eq!(opens.load(Ordering::SeqCst), 1);
        assert!(
            !String::from_utf8(stdout)
                .expect("out")
                .contains("essential-mise-checkpoint:")
        );
    }

    fn gap_db(dir: &Path) -> PathBuf {
        let mut spec = options();
        spec.accounts = Vec::new();
        let built = build(spec);
        let path = dir.join("gaps.sqlite");
        std::fs::copy(&built.path, &path).expect("copy");
        let _ = built;
        path
    }

    fn ok_db(dir: &Path) -> PathBuf {
        let mut spec = options();
        spec.accounts = vec![account(1, "one@example.com")];
        let built = build(spec);
        let path = dir.join("ok.sqlite");
        std::fs::copy(&built.path, &path).expect("copy");
        path
    }

    #[test]
    fn bad_accounts_exit_1_without_a_report_and_check_json_exits_0() {
        use clap::Parser as _;
        for accounts in [
            "not-an-email",
            "one@example.com,not-an-email",
            "one@example.com,",
        ] {
            let cli = crate::cli::Cli::try_parse_from([
                "emc",
                "google-accounts",
                "check",
                "--accounts",
                accounts,
                "--json",
            ])
            .expect("parse");
            let backend = std::sync::Arc::new(crate::backend::mock::MockBackend::new());
            let mut out = Vec::new();
            let code = crate::cli::run(backend, &cli, &mut out);
            assert_eq!(code, 1, "{accounts}");
            assert!(out.is_empty(), "{accounts}");
        }
        let built = build(options());
        let cli = crate::cli::Cli::try_parse_from([
            "emc",
            "google-accounts",
            "check",
            "--db",
            built.path.to_str().expect("utf8"),
            "--accounts",
            "one@example.com",
            "--json",
        ])
        .expect("parse");
        let backend = std::sync::Arc::new(crate::backend::mock::MockBackend::new());
        let mut out = Vec::new();
        let code = crate::cli::run(backend, &cli, &mut out);
        assert_eq!(code, 0);
        let report: Value = serde_json::from_slice(&out).expect("report");
        assert_eq!(report["status"], "gaps");
        assert_eq!(report["ok"], false);
        assert_eq!(report["title"], TITLE);
    }
}
