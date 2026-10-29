use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::ffi::c_void;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::Deserialize;
use std::os::windows::process::CommandExt;

use crate::diagnose;
use crate::localization::Strings;
use crate::models::{UsageData, UsageSection};
use crate::native_interop;

const USAGE_URL: &str = "https://api.anthropic.com/api/oauth/usage";
const MESSAGES_URL: &str = "https://api.anthropic.com/v1/messages";
const CODEX_USAGE_URL: &str = "https://chatgpt.com/backend-api/wham/usage";
const ANTIGRAVITY_CREDENTIAL_TARGET: &str = "gemini:antigravity";
const ANTIGRAVITY_ENDPOINTS: &[&str] = &[
    "https://daily-cloudcode-pa.googleapis.com",
    "https://daily-cloudcode-pa.sandbox.googleapis.com",
    "https://cloudcode-pa.googleapis.com",
];
const CREATE_NO_WINDOW: u32 = 0x08000000;
const CREATE_NEW_CONSOLE: u32 = 0x00000010;

const MODEL_FALLBACK_CHAIN: &[&str] = &["claude-3-haiku-20240307", "claude-haiku-4-5-20251001"];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PollError {
    AuthRequired,
    NoCredentials,
    TokenExpired,
    NetworkUnavailable,
    RateLimited(Option<u32>),
    ServerError,
    RequestFailed,
    /// The usage endpoint answered for a different account than the one asked
    /// for, so the numbers cannot be attributed to the requested login.
    AccountMismatch,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UsageWindowKind {
    Session,
    Weekly,
}

impl PollError {
    pub fn category(self) -> &'static str {
        match self {
            Self::AuthRequired => "auth_required",
            Self::NoCredentials => "no_credentials",
            Self::TokenExpired => "token_expired",
            Self::NetworkUnavailable => "network_unavailable",
            Self::RateLimited(_) => "rate_limited",
            Self::ServerError => "server_error",
            Self::RequestFailed => "invalid_response",
            Self::AccountMismatch => "account_mismatch",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CredentialWatchMode {
    ActiveSource,
    AllSources,
    Antigravity,
    Codex,
    /// An extra Codex profile: the `auth.json` inside that profile home.
    CodexProfile(PathBuf),
}

pub type CredentialWatchSnapshot = Vec<String>;

/// Alert-dedup key of the default Codex login. Persisted settings already use
/// this key, so it must stay stable across releases.
pub const CODEX_DEFAULT_ACCOUNT_KEY: &str = "codex";

/// One Codex login the monitor can read quota for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CodexAccount {
    /// Stable identity used for alert dedup and settings round-trips.
    pub key: String,
    /// Display label shown in menus and tooltips.
    pub label: String,
    /// `None` for the default `$CODEX_HOME` / `~/.codex` login, `Some` for an
    /// extra profile home whose `auth.json` holds a separate login.
    pub home: Option<PathBuf>,
}

impl CodexAccount {
    /// The login the Codex CLI itself uses, from `$CODEX_HOME` or `~/.codex`.
    pub fn default_account(label: impl Into<String>) -> Self {
        Self {
            key: CODEX_DEFAULT_ACCOUNT_KEY.to_string(),
            label: label.into(),
            home: None,
        }
    }

    /// An extra profile home. The caller is responsible for passing an
    /// absolute, non-empty path (settings normalization enforces this).
    pub fn profile(label: impl Into<String>, home: PathBuf) -> Self {
        let key = format!("codex:{}", home.display());
        Self {
            key,
            label: label.into(),
            home: Some(home),
        }
    }

    /// Whether this login is the default one rather than an extra profile.
    pub fn is_default(&self) -> bool {
        self.home.is_none()
    }
}

/// What a single polling slot talks to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProviderSpec {
    Claude,
    Codex(CodexAccount),
    Antigravity,
}

impl ProviderSpec {
    /// The default Codex login, labelled with the localized provider name.
    pub fn default_codex(label: impl Into<String>) -> Self {
        ProviderSpec::Codex(CodexAccount::default_account(label))
    }

    /// Codex account behind this slot, when the slot is a Codex slot.
    pub fn codex_account(&self) -> Option<&CodexAccount> {
        match self {
            ProviderSpec::Codex(account) => Some(account),
            _ => None,
        }
    }
}

#[derive(Deserialize)]
struct UsageResponse {
    five_hour: Option<UsageBucket>,
    seven_day: Option<UsageBucket>,
}

#[derive(Deserialize)]
struct UsageBucket {
    utilization: f64,
    resets_at: Option<String>,
}

#[derive(Deserialize)]
struct CodexAuthFile {
    tokens: Option<CodexTokenData>,
}

#[derive(Clone, Deserialize)]
struct CodexTokenData {
    access_token: String,
    account_id: Option<String>,
}

#[derive(Deserialize)]
struct CodexUsageResponse {
    rate_limit: Option<Option<Box<CodexRateLimitDetails>>>,
    /// Echo of the account the endpoint actually answered for.
    #[serde(default)]
    account_id: Option<String>,
}

#[derive(Deserialize)]
struct CodexRateLimitDetails {
    primary_window: Option<Option<Box<CodexRateLimitWindow>>>,
    secondary_window: Option<Option<Box<CodexRateLimitWindow>>>,
}

#[derive(Deserialize)]
struct CodexRateLimitWindow {
    used_percent: f64,
    reset_at: i64,
    limit_window_seconds: Option<u64>,
}

const CODEX_SESSION_WINDOW_MAX_SECONDS: u64 = 24 * 60 * 60;

#[derive(Deserialize)]
struct AntigravityAuthFile {
    token: AntigravityTokenData,
}

#[derive(Deserialize)]
struct AntigravityTokenData {
    access_token: String,
}

#[derive(Deserialize)]
struct AntigravityLoadResponse {
    #[serde(rename = "cloudaicompanionProject")]
    project: Option<String>,
}

#[derive(Deserialize)]
struct AntigravityModelsResponse {
    models: HashMap<String, AntigravityModelInfo>,
}

#[derive(Deserialize)]
struct AntigravityModelInfo {
    #[serde(rename = "quotaInfo")]
    quota_info: Option<AntigravityQuotaInfo>,
}

#[derive(Deserialize)]
struct AntigravityQuotaInfo {
    #[serde(rename = "remainingFraction")]
    remaining_fraction: Option<f64>,
    #[serde(rename = "resetTime")]
    reset_time: Option<String>,
}

#[derive(Deserialize)]
struct AntigravityQuotaSummaryResponse {
    groups: Option<Vec<AntigravityQuotaSummaryGroup>>,
}

#[derive(Deserialize)]
struct AntigravityQuotaSummaryGroup {
    #[serde(rename = "displayName")]
    display_name: Option<String>,
    description: Option<String>,
    buckets: Option<Vec<AntigravityQuotaSummaryBucket>>,
}

#[derive(Clone, Deserialize)]
struct AntigravityQuotaSummaryBucket {
    #[serde(rename = "bucketId")]
    bucket_id: Option<String>,
    #[serde(rename = "displayName")]
    display_name: Option<String>,
    window: Option<String>,
    #[serde(rename = "remainingFraction")]
    remaining_fraction: Option<f64>,
    #[serde(rename = "resetTime")]
    reset_time: Option<String>,
}

#[repr(C)]
struct CredentialW {
    flags: u32,
    type_: u32,
    target_name: *mut u16,
    comment: *mut u16,
    last_written: u64,
    credential_blob_size: u32,
    credential_blob: *mut u8,
    persist: u32,
    attribute_count: u32,
    attributes: *mut c_void,
    target_alias: *mut u16,
    user_name: *mut u16,
}

#[link(name = "Advapi32")]
extern "system" {
    fn CredReadW(
        target_name: *const u16,
        type_: u32,
        reserved_flags: u32,
        credential: *mut *mut CredentialW,
    ) -> i32;
    fn CredFree(buffer: *mut c_void);
}

pub fn poll_provider(spec: &ProviderSpec) -> Result<UsageData, PollError> {
    match spec {
        ProviderSpec::Claude => poll_claude_code(),
        ProviderSpec::Codex(account) => poll_codex(account),
        ProviderSpec::Antigravity => poll_antigravity(),
    }
}

/// Whether Claude Code CLI credentials are available from a supported source.
/// Claude Desktop authentication is intentionally not treated as CLI access.
pub fn claude_code_credentials_available() -> bool {
    read_first_credentials().is_some()
}

fn poll_claude_code() -> Result<UsageData, PollError> {
    let creds = match read_first_credentials() {
        Some(c) => c,
        None => {
            diagnose::log("poll failed: no Claude credentials found");
            return Err(PollError::NoCredentials);
        }
    };

    let creds = refresh_or_fallback(creds)?;

    fetch_usage_with_fallback(&creds.access_token)
}

/// Poll one Codex login. Extra profiles are checked strictly, because the
/// endpoint answers a foreign `ChatGPT-Account-Id` with the token's own usage
/// and an empty account id instead of an error.
fn poll_codex(account: &CodexAccount) -> Result<UsageData, PollError> {
    let strict = !account.is_default();
    let creds = match read_codex_credentials(account) {
        Some(creds) => creds,
        None => {
            diagnose::log(format!(
                "Codex usage poll failed: no credentials found for {}",
                account.label
            ));
            return Err(PollError::NoCredentials);
        }
    };

    match fetch_codex_usage(&creds.access_token, creds.account_id.as_deref(), strict) {
        Ok(data) => Ok(data),
        Err(PollError::AuthRequired) => {
            cli_refresh_codex_token(account, &creds.access_token);
            let refreshed = read_codex_credentials(account).ok_or(PollError::TokenExpired)?;
            fetch_codex_usage(
                &refreshed.access_token,
                refreshed.account_id.as_deref(),
                strict,
            )
        }
        Err(error) => Err(error),
    }
}

fn poll_antigravity() -> Result<UsageData, PollError> {
    let creds = match read_antigravity_credentials() {
        Some(creds) => creds,
        None => {
            diagnose::log("Antigravity usage poll failed: no Antigravity credentials found");
            return Err(PollError::NoCredentials);
        }
    };

    fetch_antigravity_usage(&creds.access_token)
}

fn refresh_or_fallback(mut creds: Credentials) -> Result<Credentials, PollError> {
    loop {
        if !is_token_expired(creds.expires_at) {
            return Ok(creds);
        }

        let source = creds.source.clone();
        cli_refresh_token(&source);

        match read_credentials_from_source(&source) {
            Some(refreshed) if !is_token_expired(refreshed.expires_at) => return Ok(refreshed),
            Some(_) => diagnose::log(format!(
                "credentials from {source:?} still expired after refresh attempt"
            )),
            None => diagnose::log(format!(
                "credentials from {source:?} unavailable after refresh attempt"
            )),
        }

        match read_next_credentials_after(&source) {
            Some(next) => creds = next,
            None => return Err(PollError::TokenExpired),
        }
    }
}

/// Invoke the Claude CLI with a minimal prompt to force its internal
/// OAuth token refresh.
fn cli_refresh_token(source: &CredentialSource) {
    match source {
        CredentialSource::Windows(_) => cli_refresh_windows_token(),
        CredentialSource::Wsl { distro } => cli_refresh_wsl_token(distro),
    }
}

fn cli_refresh_windows_token() {
    let claude_path = resolve_windows_claude_path();
    let is_cmd = claude_path.to_lowercase().ends_with(".cmd");
    diagnose::log(format!(
        "attempting Windows Claude token refresh via {claude_path}"
    ));

    let args: &[&str] = &["-p", "."];

    let mut cmd = if is_cmd {
        let mut c = Command::new("cmd.exe");
        c.arg("/c").arg(&claude_path).args(args);
        c
    } else {
        let mut c = Command::new(&claude_path);
        c.args(args);
        c
    };
    cmd.env_remove("CLAUDECODE")
        .env_remove("CLAUDE_CODE_ENTRYPOINT")
        .creation_flags(CREATE_NO_WINDOW)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());

    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(error) => {
            diagnose::log_error("unable to spawn Windows Claude token refresh", error);
            return;
        }
    };

    // Wait up to 30 seconds — don't block the poll thread forever
    let start = std::time::Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) => {
                if start.elapsed() > Duration::from_secs(30) {
                    let _ = child.kill();
                    break;
                }
                std::thread::sleep(Duration::from_millis(500));
            }
            Err(_) => break,
        }
    }
}

fn cli_refresh_wsl_token(distro: &str) {
    diagnose::log(format!(
        "attempting WSL Claude token refresh in distro {distro}"
    ));
    let mut cmd = Command::new("wsl.exe");
    cmd.arg("-d")
        .arg(distro)
        .arg("--")
        .arg("bash")
        .arg("-lic")
        .arg("if command -v claude >/dev/null 2>&1; then claude -p .; elif [ -x \"$HOME/.local/bin/claude\" ]; then \"$HOME/.local/bin/claude\" -p .; else exit 127; fi")
        .env_remove("CLAUDECODE")
        .env_remove("CLAUDE_CODE_ENTRYPOINT")
        .creation_flags(CREATE_NO_WINDOW)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());

    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(error) => {
            diagnose::log_error("unable to spawn WSL Claude token refresh", error);
            return;
        }
    };

    wait_for_refresh(&mut child);
}

/// Serializes profile deletion with CLI refreshes, which can write credentials.
pub static CODEX_PROFILE_WRITES: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn codex_refresh_is_current(account: &CodexAccount, expected_token: &str) -> bool {
    read_codex_credentials(account)
        .is_some_and(|credentials| credentials.access_token == expected_token)
}

fn cli_refresh_codex_token(account: &CodexAccount, expected_token: &str) {
    let _profile_write = CODEX_PROFILE_WRITES
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    // A poll can finish after its profile was removed. Never let its refresh
    // recreate a deleted home (or fall back to the main account).
    if !codex_refresh_is_current(account, expected_token) {
        return;
    }
    let codex_path = match resolve_windows_codex_path() {
        Ok(path) => path,
        Err(error) => {
            diagnose::log_error("unable to find Codex CLI for token refresh", error);
            return;
        }
    };
    let extension = codex_path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default();
    let is_cmd = extension.eq_ignore_ascii_case("cmd") || extension.eq_ignore_ascii_case("bat");
    let is_ps1 = extension.eq_ignore_ascii_case("ps1");
    diagnose::log(format!(
        "attempting Windows Codex token refresh for {} via {}",
        account.label,
        codex_path.display()
    ));

    let args: &[&str] = &["exec", "."];

    let mut cmd = if is_cmd {
        let mut c = Command::new("cmd.exe");
        configure_cmd_script(&mut c, &codex_path, args, "/c");
        c
    } else if is_ps1 {
        let mut c = Command::new("powershell.exe");
        c.arg("-NoProfile")
            .arg("-ExecutionPolicy")
            .arg("Bypass")
            .arg("-File")
            .arg(&codex_path)
            .args(args);
        c
    } else {
        let mut c = Command::new(&codex_path);
        c.args(args);
        c
    };
    cmd.creation_flags(CREATE_NO_WINDOW)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    if let Some(home) = &account.home {
        // The CLI resolves its login from CODEX_HOME, so refreshing an extra
        // profile has to point the child at that profile's home directory.
        cmd.env("CODEX_HOME", home);
    }
    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(error) => {
            diagnose::log_error("unable to spawn Windows Codex token refresh", error);
            return;
        }
    };

    wait_for_refresh(&mut child);
}

/// Start `codex login` for a profile directory, so an extra account can be
/// authorized without the user opening a terminal.
///
/// The CLI owns the OAuth flow, including which account the browser authorizes,
/// and it needs an interactive terminal, so it is started in a console window of
/// its own. The monitor only points `CODEX_HOME` at the profile and later reads
/// the `auth.json` the CLI writes; it never handles the sign-in itself.
pub fn spawn_codex_login(home: &Path) -> std::io::Result<()> {
    // The CLI refuses to load a configuration from a missing directory, so the
    // profile folder has to exist before it starts.
    std::fs::create_dir_all(home)?;
    let codex_path = resolve_windows_codex_path()?;
    let extension = codex_path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default();
    let is_script = extension.eq_ignore_ascii_case("cmd") || extension.eq_ignore_ascii_case("bat");
    let is_ps1 = extension.eq_ignore_ascii_case("ps1");
    diagnose::log(format!(
        "starting Codex login for {} via {}",
        home.display(),
        codex_path.display()
    ));

    let mut cmd = if is_script {
        // `/k` keeps the CLI's output on screen instead of letting the window
        // flash past when the sign-in fails.
        let mut c = Command::new("cmd.exe");
        configure_cmd_script(&mut c, &codex_path, &["login"], "/k");
        c
    } else if is_ps1 {
        let mut c = Command::new("powershell.exe");
        c.arg("-NoProfile")
            .arg("-NoExit")
            .arg("-ExecutionPolicy")
            .arg("Bypass")
            .arg("-File")
            .arg(&codex_path)
            .arg("login");
        c
    } else {
        let mut c = Command::new(&codex_path);
        c.arg("login");
        c
    };
    cmd.env("CODEX_HOME", home);
    // A console window of its own: the CLI needs a terminal for the sign-in, and
    // redirecting its streams would take that away.
    cmd.creation_flags(CREATE_NEW_CONSOLE);
    cmd.spawn().map(|_child| ())
}

/// Whether a profile directory already holds a usable login. Used to notice a
/// finished `codex login`, and to ignore a half-written credentials file.
pub fn codex_profile_ready(home: &Path) -> bool {
    let account = CodexAccount::profile(String::new(), home.to_path_buf());
    codex_auth_path_for(&account).is_some_and(|path| path.is_file())
        && read_codex_credentials(&account).is_some()
}

/// Spawn a command and wait up to `timeout` for it to finish.
/// Returns None if the process fails to start or exceeds the deadline.
fn run_with_timeout(cmd: &mut Command, timeout: Duration) -> Option<std::process::Output> {
    let mut child = cmd.spawn().ok()?;
    let start = std::time::Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_)) => return child.wait_with_output().ok(),
            Ok(None) => {
                if start.elapsed() > timeout {
                    let _ = child.kill();
                    let _ = child.wait();
                    return None;
                }
                std::thread::sleep(Duration::from_millis(100));
            }
            Err(_) => return None,
        }
    }
}

fn wait_for_refresh(child: &mut std::process::Child) {
    // Wait up to 30 seconds; don't block the poll thread forever.
    let start = std::time::Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) => {
                if start.elapsed() > Duration::from_secs(30) {
                    let _ = child.kill();
                    break;
                }
                std::thread::sleep(Duration::from_millis(500));
            }
            Err(_) => break,
        }
    }
}

/// Resolve the full path to the `claude` CLI executable.
fn resolve_windows_claude_path() -> String {
    for name in &["claude.cmd", "claude"] {
        if Command::new(name)
            .arg("--version")
            .creation_flags(CREATE_NO_WINDOW)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .is_ok()
        {
            return name.to_string();
        }
    }

    for name in &["claude.cmd", "claude"] {
        if let Ok(output) = Command::new("where.exe")
            .arg(name)
            .creation_flags(CREATE_NO_WINDOW)
            .output()
        {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                if let Some(first_line) = stdout.lines().next() {
                    let path = first_line.trim().to_string();
                    if !path.is_empty() {
                        return path;
                    }
                }
            }
        }
    }

    "claude.cmd".to_string()
}

fn resolve_windows_codex_path() -> std::io::Result<PathBuf> {
    let directories = std::env::var_os("PATH")
        .map(|path| std::env::split_paths(&path).collect::<Vec<_>>())
        .unwrap_or_default();
    codex_cli_in_directories(&directories)
}

/// Put the path in the child environment: CMD expands this once, so literal `%`
/// characters in the expanded value (including `%NAME%`) are not re-expanded.
fn cmd_script_command(args: &[&str], mode: &str) -> String {
    let mut command = "\"%CODEX_CLI_SCRIPT_PATH%\"".to_string();
    for arg in args {
        command.push(' ');
        command.push_str(arg);
    }
    // raw_arg is intentional: Rust's normal Windows quoting escapes embedded
    // quotes for argv parsing, which CMD does not use when parsing /c or /k.
    format!(" /d /v:off /s {mode} \"{command}\"")
}

fn configure_cmd_script(command: &mut Command, path: &Path, args: &[&str], mode: &str) {
    command
        .raw_arg(cmd_script_command(args, mode))
        .env("CODEX_CLI_SCRIPT_PATH", path);
}

fn codex_cli_in_directories(directories: &[PathBuf]) -> std::io::Result<PathBuf> {
    // Use native Unicode paths rather than decoding `where.exe` output, whose
    // console code page need not be UTF-8 (notably for npm paths under usernames).
    find_codex_cli(|name| {
        directories
            .iter()
            .map(|directory| directory.join(name))
            .find(|path| path.is_file())
    })
}

fn find_codex_cli(mut find: impl FnMut(&str) -> Option<PathBuf>) -> std::io::Result<PathBuf> {
    for name in [
        "codex.cmd",
        "codex.ps1",
        "codex.exe",
        "codex.bat",
        "codex.com",
        "codex",
    ] {
        if let Some(path) = find(name) {
            return Ok(path);
        }
    }

    Err(std::io::Error::new(
        std::io::ErrorKind::NotFound,
        "Codex CLI was not found",
    ))
}

fn build_agent() -> Result<ureq::Agent, PollError> {
    let tls = native_tls::TlsConnector::new().map_err(|_| PollError::RequestFailed)?;
    Ok(ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(30))
        .tls_connector(std::sync::Arc::new(tls))
        .build())
}

fn classify_http_status(status: u16) -> PollError {
    match status {
        401 | 403 => PollError::AuthRequired,
        429 => PollError::RateLimited(None),
        500..=599 => PollError::ServerError,
        _ => PollError::RequestFailed,
    }
}

fn classify_ureq_error(error: &ureq::Error) -> PollError {
    match error {
        ureq::Error::Status(429, response) => PollError::RateLimited(
            response
                .header("Retry-After")
                .and_then(|value| crate::retry_after::delay_ms(value, SystemTime::now())),
        ),
        ureq::Error::Status(status, _) => classify_http_status(*status),
        ureq::Error::Transport(_) => PollError::NetworkUnavailable,
    }
}

pub fn credential_watch_snapshot(mode: CredentialWatchMode) -> CredentialWatchSnapshot {
    match mode {
        CredentialWatchMode::Codex => {
            return codex_auth_path()
                .map(|p| vec![windows_credential_watch_signature(&p)])
                .unwrap_or_default();
        }
        CredentialWatchMode::CodexProfile(home) => {
            return vec![windows_credential_watch_signature(&home.join("auth.json"))];
        }
        CredentialWatchMode::Antigravity => {
            return vec![antigravity_credential_watch_signature()];
        }
        CredentialWatchMode::ActiveSource | CredentialWatchMode::AllSources => {}
    }

    let sources = match mode {
        CredentialWatchMode::ActiveSource => read_first_credentials()
            .map(|creds| vec![creds.source])
            .unwrap_or_else(all_known_credential_sources),
        CredentialWatchMode::AllSources => all_known_credential_sources(),
        CredentialWatchMode::Antigravity
        | CredentialWatchMode::Codex
        | CredentialWatchMode::CodexProfile(_) => unreachable!(),
    };

    let mut snapshot: CredentialWatchSnapshot = sources
        .into_iter()
        .filter_map(|source| credential_watch_signature(&source))
        .collect();
    snapshot.sort();
    snapshot.dedup();
    snapshot
}

fn all_known_credential_sources() -> Vec<CredentialSource> {
    let mut sources = Vec::new();
    if let Some(source) = windows_credential_source() {
        sources.push(source);
    }
    for distro in list_wsl_distros() {
        sources.push(CredentialSource::Wsl { distro });
    }
    sources
}

fn windows_credential_source() -> Option<CredentialSource> {
    Some(CredentialSource::Windows(windows_credentials_path_from(
        std::env::var_os("CLAUDE_CONFIG_DIR").map(PathBuf::from),
        dirs::home_dir(),
    )?))
}

fn windows_credentials_path_from(
    config_dir: Option<PathBuf>,
    home: Option<PathBuf>,
) -> Option<PathBuf> {
    let config_dir = config_dir.filter(|path| !path.as_os_str().is_empty());
    let directory = config_dir.or_else(|| home.map(|path| path.join(".claude")))?;
    Some(directory.join(".credentials.json"))
}

fn credential_watch_signature(source: &CredentialSource) -> Option<String> {
    match source {
        CredentialSource::Windows(path) => Some(windows_credential_watch_signature(path)),
        CredentialSource::Wsl { distro } => wsl_credential_watch_signature(distro),
    }
}

fn windows_credential_watch_signature(path: &PathBuf) -> String {
    let key = format!("win:{}", path.display());
    match std::fs::metadata(path) {
        Ok(metadata) => {
            let modified = metadata
                .modified()
                .ok()
                .and_then(|value| value.duration_since(UNIX_EPOCH).ok())
                .map(|value| value.as_nanos())
                .unwrap_or(0);
            format!("{key}|present|{}|{modified}", metadata.len())
        }
        Err(_) => format!("{key}|missing"),
    }
}

fn wsl_credential_watch_signature(distro: &str) -> Option<String> {
    let output = run_with_timeout(
        Command::new("wsl.exe")
            .arg("-d")
            .arg(distro)
            .arg("--")
            .arg("sh")
            .arg("-lc")
            .arg(
                "if [ -f ~/.claude/.credentials.json ]; then \
                 stat -c 'present|%s|%Y' ~/.claude/.credentials.json; \
                 else echo missing; fi",
            )
            .creation_flags(CREATE_NO_WINDOW)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null()),
        Duration::from_secs(5),
    )?;

    let state = if output.status.success() {
        decode_wsl_text(&output.stdout).trim().to_string()
    } else {
        format!("status-{}", output.status)
    };

    Some(format!("wsl:{distro}|{state}"))
}

fn fetch_usage_with_fallback(token: &str) -> Result<UsageData, PollError> {
    // Try the dedicated usage endpoint first
    if let Some(data) = try_usage_endpoint(token)? {
        // If reset timers are missing, fill them in from the Messages API
        if data.session.resets_at.is_none() || data.weekly.resets_at.is_none() {
            if let Ok(fallback) = fetch_usage_via_messages(token) {
                let mut merged = data;
                if merged.session.resets_at.is_none() {
                    merged.session.resets_at = fallback.session.resets_at;
                }
                if merged.weekly.resets_at.is_none() {
                    merged.weekly.resets_at = fallback.weekly.resets_at;
                }
                return Ok(merged);
            }
        }
        return Ok(data);
    }

    // Fall back to Messages API with rate limit headers
    let result = fetch_usage_via_messages(token);
    if result.is_err() {
        diagnose::log("usage endpoint and Messages API fallback both failed");
    }
    result
}

fn try_usage_endpoint(token: &str) -> Result<Option<UsageData>, PollError> {
    let agent = build_agent()?;

    let resp = match agent
        .get(USAGE_URL)
        .set("Authorization", &format!("Bearer {token}"))
        .set("anthropic-beta", "oauth-2025-04-20")
        .call()
    {
        Ok(resp) => resp,
        Err(ureq::Error::Status(code, _)) if code == 401 || code == 403 => {
            diagnose::log(format!(
                "usage endpoint returned auth error status {code}; re-login required"
            ));
            return Err(PollError::AuthRequired);
        }
        Err(error @ ureq::Error::Status(429, _)) => return Err(classify_ureq_error(&error)),
        Err(_) => return Ok(None),
    };

    let response: UsageResponse = match resp.into_json() {
        Ok(response) => response,
        Err(_) => return Ok(None),
    };
    let mut data = UsageData::default();

    if let Some(bucket) = &response.five_hour {
        data.session.percentage = bucket.utilization;
        data.session.resets_at = parse_iso8601(bucket.resets_at.as_deref());
    }

    if let Some(bucket) = &response.seven_day {
        data.weekly.percentage = bucket.utilization;
        data.weekly.resets_at = parse_iso8601(bucket.resets_at.as_deref());
    }

    Ok(Some(data))
}

fn fetch_usage_via_messages(token: &str) -> Result<UsageData, PollError> {
    let agent = build_agent()?;
    let mut last_error = PollError::RequestFailed;

    for model in MODEL_FALLBACK_CHAIN {
        let body = serde_json::json!({
            "model": model,
            "max_tokens": 1,
            "messages": [{"role": "user", "content": "."}]
        });

        let response = match agent
            .post(MESSAGES_URL)
            .set("Authorization", &format!("Bearer {token}"))
            .set("anthropic-version", "2023-06-01")
            .set("anthropic-beta", "oauth-2025-04-20")
            .send_json(&body)
        {
            Ok(resp) => resp,
            Err(ureq::Error::Status(code, _)) if code == 401 || code == 403 => {
                diagnose::log(format!(
                    "messages endpoint returned auth error status {code}; re-login required"
                ));
                return Err(PollError::AuthRequired);
            }
            Err(ureq::Error::Status(code, resp)) => {
                last_error =
                    if code == 429 {
                        PollError::RateLimited(resp.header("Retry-After").and_then(|value| {
                            crate::retry_after::delay_ms(value, SystemTime::now())
                        }))
                    } else {
                        classify_http_status(code)
                    };
                if matches!(last_error, PollError::RateLimited(_)) {
                    return Err(last_error);
                }
                resp
            }
            Err(error) => {
                last_error = classify_ureq_error(&error);
                continue;
            }
        };

        let h5 = response.header("anthropic-ratelimit-unified-5h-utilization");
        let h7 = response.header("anthropic-ratelimit-unified-7d-utilization");
        let hs = response.header("anthropic-ratelimit-unified-status");

        if h5.is_some() || h7.is_some() || hs.is_some() {
            return Ok(parse_rate_limit_headers(&response));
        }
    }

    Err(last_error)
}

fn parse_rate_limit_headers(response: &ureq::Response) -> UsageData {
    let mut data = UsageData::default();

    data.session.percentage =
        get_header_f64(response, "anthropic-ratelimit-unified-5h-utilization") * 100.0;
    data.session.resets_at = unix_to_system_time(get_header_i64(
        response,
        "anthropic-ratelimit-unified-5h-reset",
    ));

    data.weekly.percentage =
        get_header_f64(response, "anthropic-ratelimit-unified-7d-utilization") * 100.0;
    data.weekly.resets_at = unix_to_system_time(get_header_i64(
        response,
        "anthropic-ratelimit-unified-7d-reset",
    ));

    let overall_reset = get_header_i64(response, "anthropic-ratelimit-unified-reset");

    if data.session.percentage == 0.0 && data.weekly.percentage == 0.0 {
        let status = response.header("anthropic-ratelimit-unified-status");
        if status == Some("rejected") {
            let claim = response.header("anthropic-ratelimit-unified-representative-claim");
            match claim {
                Some("five_hour") => data.session.percentage = 100.0,
                Some("seven_day") => data.weekly.percentage = 100.0,
                _ => {}
            }
        }

        if data.session.resets_at.is_none() && overall_reset.is_some() {
            data.session.resets_at = unix_to_system_time(overall_reset);
        }
    }

    data
}

/// `strict` requests an answer that names the requested account. The endpoint
/// replies with the bearer token's own usage (and an empty account id) when the
/// requested account is not the one the token belongs to, so a login the user
/// configured separately must confirm the echo before its numbers are shown.
fn fetch_codex_usage(
    token: &str,
    account_id: Option<&str>,
    strict: bool,
) -> Result<UsageData, PollError> {
    let agent = build_agent()?;
    let mut request = agent
        .get(CODEX_USAGE_URL)
        .set("Authorization", &format!("Bearer {token}"))
        .set("User-Agent", "codex-cli");

    if let Some(account_id) = account_id.filter(|value| !value.is_empty()) {
        request = request.set("ChatGPT-Account-Id", account_id);
    }

    let resp = match request.call() {
        Ok(resp) => resp,
        Err(error) => {
            let classified = classify_ureq_error(&error);
            diagnose::log_error("Codex usage endpoint request failed", error);
            return Err(classified);
        }
    };

    let response: CodexUsageResponse = match resp.into_json() {
        Ok(response) => response,
        Err(error) => {
            diagnose::log_error("unable to parse Codex usage response", error);
            return Err(PollError::RequestFailed);
        }
    };

    if codex_account_mismatch(account_id, response.account_id.as_deref(), strict) {
        diagnose::log("Codex usage endpoint answered for a different account");
        return Err(PollError::AccountMismatch);
    }

    codex_usage_from_response(response).ok_or(PollError::RequestFailed)
}

/// Whether a response belongs to a different login than the one requested.
///
/// The endpoint always answers HTTP 200, so the echo in the body is the only
/// signal: a different account id is always a mismatch. An empty or missing
/// echo is only treated as a mismatch when the caller asked strictly (an extra
/// profile), because the default login has always been displayed from whatever
/// the token reports.
fn codex_account_mismatch(requested: Option<&str>, returned: Option<&str>, strict: bool) -> bool {
    let Some(requested) = requested.filter(|value| !value.is_empty()) else {
        return false;
    };
    match returned.filter(|value| !value.is_empty()) {
        Some(returned) => returned != requested,
        None => strict,
    }
}

fn codex_usage_from_response(response: CodexUsageResponse) -> Option<UsageData> {
    let details = *response.rate_limit.flatten()?;
    let mut data = UsageData::default();
    let mut has_session = false;
    let mut has_weekly = false;

    let primary = details.primary_window.flatten();
    let secondary = details.secondary_window.flatten();

    for window in [primary.as_deref(), secondary.as_deref()]
        .into_iter()
        .flatten()
    {
        match codex_window_kind(window) {
            Some(UsageWindowKind::Session) if !has_session => {
                data.session = codex_section_from_window(window);
                has_session = true;
            }
            Some(UsageWindowKind::Weekly) if !has_weekly => {
                data.weekly = codex_section_from_window(window);
                has_weekly = true;
            }
            _ => {}
        }
    }

    // Older responses did not include a window duration. Preserve their
    // positional meaning while avoiding overwriting a duration-classified
    // window from the newer response shape.
    if let Some(window) = primary
        .as_deref()
        .filter(|window| window.limit_window_seconds.is_none() && !has_session)
    {
        data.session = codex_section_from_window(window);
    }
    if let Some(window) = secondary
        .as_deref()
        .filter(|window| window.limit_window_seconds.is_none() && !has_weekly)
    {
        data.weekly = codex_section_from_window(window);
    }

    Some(data)
}

fn codex_window_kind(window: &CodexRateLimitWindow) -> Option<UsageWindowKind> {
    window.limit_window_seconds.map(|seconds| {
        if seconds <= CODEX_SESSION_WINDOW_MAX_SECONDS {
            UsageWindowKind::Session
        } else {
            UsageWindowKind::Weekly
        }
    })
}

fn codex_section_from_window(window: &CodexRateLimitWindow) -> UsageSection {
    UsageSection {
        percentage: window.used_percent,
        resets_at: unix_to_system_time(Some(window.reset_at)),
    }
}

fn antigravity_credential_watch_signature() -> String {
    let Some(content) = read_windows_generic_credential(ANTIGRAVITY_CREDENTIAL_TARGET) else {
        return format!("{ANTIGRAVITY_CREDENTIAL_TARGET}|missing");
    };

    let mut hasher = DefaultHasher::new();
    content.hash(&mut hasher);
    format!(
        "{ANTIGRAVITY_CREDENTIAL_TARGET}|present|{}|{}",
        content.len(),
        hasher.finish()
    )
}

fn fetch_antigravity_usage(token: &str) -> Result<UsageData, PollError> {
    let mut auth_error = false;
    let mut last_error = PollError::RequestFailed;

    for base_url in ANTIGRAVITY_ENDPOINTS {
        match fetch_antigravity_usage_from_endpoint(base_url, token) {
            Ok(data) => return Ok(data),
            Err(PollError::AuthRequired) => auth_error = true,
            Err(error) => last_error = error,
        }
    }

    if auth_error {
        Err(PollError::AuthRequired)
    } else {
        Err(last_error)
    }
}

fn fetch_antigravity_usage_from_endpoint(
    base_url: &str,
    token: &str,
) -> Result<UsageData, PollError> {
    let project = fetch_antigravity_project(base_url, token)?;
    if let Some(project) = project.as_deref() {
        match fetch_antigravity_quota_summary(base_url, token, project) {
            Ok(data) => return Ok(data),
            Err(PollError::AuthRequired) => return Err(PollError::AuthRequired),
            Err(error) => diagnose::log(format!(
                "Antigravity retrieveUserQuotaSummary failed, falling back to model quota: {error:?}"
            )),
        }
    }

    let session = fetch_antigravity_model_quota(base_url, token, project.as_deref())?;
    let weekly = UsageSection::default();

    Ok(UsageData { session, weekly })
}

fn fetch_antigravity_project(base_url: &str, token: &str) -> Result<Option<String>, PollError> {
    let agent = build_agent()?;
    let body = serde_json::json!({
        "metadata": {
            "ideType": "ANTIGRAVITY"
        }
    });

    let resp = match agent
        .post(&format!("{base_url}/v1internal:loadCodeAssist"))
        .set("Authorization", &format!("Bearer {token}"))
        .set("Content-Type", "application/json")
        .set("User-Agent", "antigravity")
        .send_json(&body)
    {
        Ok(resp) => resp,
        Err(error) => {
            let classified = classify_ureq_error(&error);
            diagnose::log_error("Antigravity loadCodeAssist request failed", error);
            return Err(classified);
        }
    };

    let response: AntigravityLoadResponse = match resp.into_json() {
        Ok(response) => response,
        Err(error) => {
            diagnose::log_error("unable to parse Antigravity loadCodeAssist response", error);
            return Err(PollError::RequestFailed);
        }
    };

    Ok(response.project.filter(|project| !project.is_empty()))
}

fn fetch_antigravity_model_quota(
    base_url: &str,
    token: &str,
    project: Option<&str>,
) -> Result<UsageSection, PollError> {
    let agent = build_agent()?;
    let body = match project {
        Some(project) => serde_json::json!({ "project": project }),
        None => serde_json::json!({}),
    };

    let resp = match agent
        .post(&format!("{base_url}/v1internal:fetchAvailableModels"))
        .set("Authorization", &format!("Bearer {token}"))
        .set("Content-Type", "application/json")
        .set("User-Agent", "antigravity")
        .send_json(&body)
    {
        Ok(resp) => resp,
        Err(error) => {
            let classified = classify_ureq_error(&error);
            diagnose::log_error("Antigravity fetchAvailableModels request failed", error);
            return Err(classified);
        }
    };

    let response: AntigravityModelsResponse = match resp.into_json() {
        Ok(response) => response,
        Err(error) => {
            diagnose::log_error(
                "unable to parse Antigravity fetchAvailableModels response",
                error,
            );
            return Err(PollError::RequestFailed);
        }
    };

    best_antigravity_section(response.models.into_iter().filter_map(|(model, info)| {
        let quota = info.quota_info?;
        if !is_antigravity_display_model(&model) {
            return None;
        }
        antigravity_section_from_quota(quota)
    }))
    .ok_or(PollError::RequestFailed)
}

fn fetch_antigravity_quota_summary(
    base_url: &str,
    token: &str,
    project: &str,
) -> Result<UsageData, PollError> {
    let agent = build_agent()?;
    let body = serde_json::json!({ "project": project });

    let resp = match agent
        .post(&format!("{base_url}/v1internal:retrieveUserQuotaSummary"))
        .set("Authorization", &format!("Bearer {token}"))
        .set("Content-Type", "application/json")
        .set("User-Agent", "antigravity")
        .send_json(&body)
    {
        Ok(resp) => resp,
        Err(error) => {
            let classified = classify_ureq_error(&error);
            diagnose::log_error("Antigravity retrieveUserQuotaSummary request failed", error);
            return Err(classified);
        }
    };

    let response: AntigravityQuotaSummaryResponse = match resp.into_json() {
        Ok(response) => response,
        Err(error) => {
            diagnose::log_error(
                "unable to parse Antigravity retrieveUserQuotaSummary response",
                error,
            );
            return Err(PollError::RequestFailed);
        }
    };

    antigravity_usage_from_summary(response).ok_or(PollError::RequestFailed)
}

fn antigravity_section_from_quota(quota: AntigravityQuotaInfo) -> Option<UsageSection> {
    let remaining = quota.remaining_fraction?.clamp(0.0, 1.0);
    Some(UsageSection {
        percentage: (1.0 - remaining) * 100.0,
        resets_at: parse_iso8601(quota.reset_time.as_deref()),
    })
}

fn antigravity_section_from_summary_bucket(
    bucket: &AntigravityQuotaSummaryBucket,
) -> Option<UsageSection> {
    let remaining = bucket.remaining_fraction?.clamp(0.0, 1.0);
    Some(UsageSection {
        percentage: (1.0 - remaining) * 100.0,
        resets_at: parse_iso8601(bucket.reset_time.as_deref()),
    })
}

fn antigravity_usage_from_summary(response: AntigravityQuotaSummaryResponse) -> Option<UsageData> {
    let mut fallback = None;

    for group in response.groups.unwrap_or_default() {
        let is_gemini = is_antigravity_gemini_summary_group(&group);
        let usage = antigravity_usage_from_summary_group(group);

        if is_gemini && usage.is_some() {
            return usage;
        }

        if fallback.is_none() {
            fallback = usage;
        }
    }

    fallback
}

fn antigravity_usage_from_summary_group(group: AntigravityQuotaSummaryGroup) -> Option<UsageData> {
    let mut data = UsageData::default();
    let mut has_quota = false;

    for bucket in group.buckets.unwrap_or_default() {
        let Some(section) = antigravity_section_from_summary_bucket(&bucket) else {
            continue;
        };

        match bucket.window.as_deref() {
            Some(window) if window.eq_ignore_ascii_case("5h") => {
                data.session = section;
                has_quota = true;
            }
            Some(window) if window.eq_ignore_ascii_case("weekly") => {
                data.weekly = section;
                has_quota = true;
            }
            _ => {}
        }
    }

    has_quota.then_some(data)
}

fn is_antigravity_gemini_summary_group(group: &AntigravityQuotaSummaryGroup) -> bool {
    group
        .display_name
        .as_deref()
        .is_some_and(|name| name.to_ascii_lowercase().contains("gemini"))
        || group
            .description
            .as_deref()
            .is_some_and(|description| description.to_ascii_lowercase().contains("gemini"))
        || group.buckets.as_ref().is_some_and(|buckets| {
            buckets.iter().any(|bucket| {
                bucket
                    .bucket_id
                    .as_deref()
                    .is_some_and(|id| id.to_ascii_lowercase().starts_with("gemini-"))
                    || bucket
                        .display_name
                        .as_deref()
                        .is_some_and(|name| name.to_ascii_lowercase().contains("gemini"))
            })
        })
}

fn best_antigravity_section<I>(sections: I) -> Option<UsageSection>
where
    I: IntoIterator<Item = UsageSection>,
{
    sections.into_iter().max_by(|a, b| {
        a.percentage
            .partial_cmp(&b.percentage)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.resets_at.cmp(&b.resets_at))
    })
}

fn is_antigravity_display_model(model: &str) -> bool {
    model.starts_with("gemini")
        || model.starts_with("claude")
        || model.starts_with("gpt")
        || model.starts_with("image")
        || model.starts_with("imagen")
}

fn get_header_f64(response: &ureq::Response, name: &str) -> f64 {
    response
        .header(name)
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(0.0)
}

fn get_header_i64(response: &ureq::Response, name: &str) -> Option<i64> {
    response.header(name).and_then(|s| s.parse::<i64>().ok())
}

fn unix_to_system_time(unix_secs: Option<i64>) -> Option<SystemTime> {
    let secs = unix_secs?;
    if secs < 0 {
        return None;
    }
    Some(UNIX_EPOCH + Duration::from_secs(secs as u64))
}

struct Credentials {
    access_token: String,
    expires_at: Option<i64>,
    source: CredentialSource,
}

#[derive(Clone, Debug)]
enum CredentialSource {
    Windows(PathBuf),
    Wsl { distro: String },
}

fn read_first_credentials() -> Option<Credentials> {
    if let Some(creds) = read_windows_credentials() {
        return Some(creds);
    }

    for distro in list_wsl_distros() {
        if let Some(creds) = read_wsl_credentials(&distro) {
            return Some(creds);
        }
    }

    None
}

fn read_windows_credentials() -> Option<Credentials> {
    let CredentialSource::Windows(cred_path) = windows_credential_source()? else {
        return None;
    };
    let content = match std::fs::read_to_string(&cred_path) {
        Ok(content) => content,
        Err(error) => {
            if diagnose::is_enabled() {
                diagnose::log_error(
                    &format!(
                        "unable to read Windows credentials at {}",
                        cred_path.display()
                    ),
                    error,
                );
            }
            return None;
        }
    };
    parse_credentials(&content, CredentialSource::Windows(cred_path))
}

fn read_credentials_from_source(source: &CredentialSource) -> Option<Credentials> {
    match source {
        CredentialSource::Windows(path) => {
            let content = std::fs::read_to_string(path).ok()?;
            parse_credentials(&content, source.clone())
        }
        CredentialSource::Wsl { distro } => read_wsl_credentials(distro),
    }
}

/// The default Codex home directory: `$CODEX_HOME`, or `~/.codex`.
pub fn codex_home() -> Option<PathBuf> {
    if let Some(codex_home) = std::env::var_os("CODEX_HOME").map(PathBuf::from) {
        return Some(codex_home);
    }

    Some(dirs::home_dir()?.join(".codex"))
}

fn codex_auth_path() -> Option<PathBuf> {
    Some(codex_home()?.join("auth.json"))
}

/// Credentials file for a Codex login: the default location, or the profile's
/// own `auth.json`.
pub fn codex_auth_path_for(account: &CodexAccount) -> Option<PathBuf> {
    match &account.home {
        Some(home) => Some(home.join("auth.json")),
        None => codex_auth_path(),
    }
}

fn read_codex_credentials(account: &CodexAccount) -> Option<CodexTokenData> {
    let auth_path = codex_auth_path_for(account)?;
    let content = match std::fs::read_to_string(&auth_path) {
        Ok(content) => content,
        Err(error) => {
            diagnose::log_error(
                &format!(
                    "unable to read Codex credentials at {}",
                    auth_path.display()
                ),
                error,
            );
            return None;
        }
    };

    let auth: CodexAuthFile = serde_json::from_str(&content).ok()?;
    auth.tokens.filter(|tokens| !tokens.access_token.is_empty())
}

/// Quota identity from local credentials, never the display email. Keep the
/// subject when available so different members of one workspace remain distinct.
pub fn codex_account_identity(account: &CodexAccount) -> Option<(String, Option<String>)> {
    let content = std::fs::read_to_string(codex_auth_path_for(account)?).ok()?;
    let file: serde_json::Value = serde_json::from_str(&content).ok()?;
    let tokens = file.get("tokens")?;
    let account_id = tokens.get("account_id")?.as_str()?.trim();
    if account_id.is_empty() {
        return None;
    }
    let subject = tokens
        .get("id_token")
        .and_then(|token| token.as_str())
        .and_then(|token| id_token_claim(token, "sub"))
        .filter(|subject| !subject.is_empty());
    Some((account_id.to_string(), subject))
}

/// Display label for a login, read from the id token already stored in
/// `auth.json`. The CLI cannot be asked without a terminal, so this is the only
/// non-interactive source of an account name. Returns None when the file or the
/// claim is unusable, letting callers fall back to other names.
pub fn codex_account_label(account: &CodexAccount) -> Option<String> {
    let content = std::fs::read_to_string(codex_auth_path_for(account)?).ok()?;
    let file: serde_json::Value = serde_json::from_str(&content).ok()?;
    let id_token = file.get("tokens")?.get("id_token")?.as_str()?;
    let email = id_token_claim(id_token, "email")?;
    let email = email.trim();
    if email.is_empty() {
        None
    } else {
        Some(email.to_string())
    }
}

/// Read one string claim from a JWT payload. The signature is not verified:
/// this only ever labels a login the user already configured locally.
fn id_token_claim(token: &str, claim: &str) -> Option<String> {
    let payload = token.split('.').nth(1)?;
    let decoded = base64url_decode(payload)?;
    let json: serde_json::Value = serde_json::from_slice(&decoded).ok()?;
    json.get(claim)?.as_str().map(|value| value.to_string())
}

/// Minimal unpadded base64url decoder, enough for JWT payload segments.
fn base64url_decode(input: &str) -> Option<Vec<u8>> {
    fn digit(byte: u8) -> Option<u32> {
        match byte {
            b'A'..=b'Z' => Some((byte - b'A') as u32),
            b'a'..=b'z' => Some((byte - b'a') as u32 + 26),
            b'0'..=b'9' => Some((byte - b'0') as u32 + 52),
            b'-' | b'+' => Some(62),
            b'_' | b'/' => Some(63),
            _ => None,
        }
    }

    let mut out = Vec::with_capacity(input.len() * 3 / 4);
    let mut buffer = 0u32;
    let mut bits = 0u32;
    for byte in input.bytes() {
        if byte == b'=' {
            break;
        }
        buffer = (buffer << 6) | digit(byte)?;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buffer >> bits) as u8);
        }
    }
    Some(out)
}

fn read_antigravity_credentials() -> Option<AntigravityTokenData> {
    let content = read_windows_generic_credential(ANTIGRAVITY_CREDENTIAL_TARGET)?;
    let auth: AntigravityAuthFile = serde_json::from_str(&content).ok()?;
    if auth.token.access_token.is_empty() {
        None
    } else {
        Some(auth.token)
    }
}

fn read_windows_generic_credential(target: &str) -> Option<String> {
    const CRED_TYPE_GENERIC: u32 = 1;

    let mut target_wide: Vec<u16> = target.encode_utf16().chain(std::iter::once(0)).collect();
    let mut credential: *mut CredentialW = std::ptr::null_mut();

    let ok = unsafe {
        CredReadW(
            target_wide.as_mut_ptr(),
            CRED_TYPE_GENERIC,
            0,
            &mut credential,
        )
    };

    if ok == 0 || credential.is_null() {
        diagnose::log(format!(
            "unable to read Windows generic credential target {target}"
        ));
        return None;
    }

    let result = unsafe {
        let cred = &*credential;
        if cred.credential_blob_size == 0 || cred.credential_blob.is_null() {
            CredFree(credential as *mut c_void);
            return None;
        }
        let bytes =
            std::slice::from_raw_parts(cred.credential_blob, cred.credential_blob_size as usize);
        let text = String::from_utf8(bytes.to_vec()).ok();
        CredFree(credential as *mut c_void);
        text
    };

    result
}

fn read_wsl_credentials(distro: &str) -> Option<Credentials> {
    let output = run_with_timeout(
        Command::new("wsl.exe")
            .arg("-d")
            .arg(distro)
            .arg("--")
            .arg("sh")
            .arg("-lc")
            .arg("cat ~/.claude/.credentials.json")
            .creation_flags(CREATE_NO_WINDOW)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null()),
        Duration::from_secs(5),
    )?;

    if !output.status.success() {
        diagnose::log(format!(
            "WSL credentials probe failed for distro {distro} with status {}",
            output.status
        ));
        return None;
    }

    let content = String::from_utf8(output.stdout).ok()?;
    parse_credentials(
        &content,
        CredentialSource::Wsl {
            distro: distro.to_string(),
        },
    )
}

fn parse_credentials(content: &str, source: CredentialSource) -> Option<Credentials> {
    let json: serde_json::Value = serde_json::from_str(content).ok()?;

    let oauth = json.get("claudeAiOauth")?;
    let access_token = oauth
        .get("accessToken")
        .and_then(|v| v.as_str())?
        .to_string();
    let expires_at = oauth.get("expiresAt").and_then(|v| v.as_i64());

    Some(Credentials {
        access_token,
        expires_at,
        source,
    })
}

fn read_next_credentials_after(source: &CredentialSource) -> Option<Credentials> {
    match source {
        CredentialSource::Windows(_) => {
            for distro in list_wsl_distros() {
                if let Some(creds) = read_wsl_credentials(&distro) {
                    return Some(creds);
                }
            }
        }
        CredentialSource::Wsl { distro } => {
            let mut past_current = false;
            for candidate_distro in list_wsl_distros() {
                if !past_current {
                    past_current = candidate_distro == *distro;
                    continue;
                }
                if let Some(creds) = read_wsl_credentials(&candidate_distro) {
                    return Some(creds);
                }
            }
        }
    }

    None
}

fn list_wsl_distros() -> Vec<String> {
    let output = match run_with_timeout(
        Command::new("wsl.exe")
            .args(["-l", "-q"])
            .creation_flags(CREATE_NO_WINDOW)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null()),
        Duration::from_secs(5),
    ) {
        Some(output) if output.status.success() => output,
        _ => {
            diagnose::log("unable to enumerate WSL distros");
            return Vec::new();
        }
    };

    let stdout = decode_wsl_text(&output.stdout);
    stdout
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

fn decode_wsl_text(bytes: &[u8]) -> String {
    if bytes.is_empty() {
        return String::new();
    }

    if let Some(decoded) = decode_utf16le(bytes) {
        return decoded;
    }

    String::from_utf8_lossy(bytes).into_owned()
}

fn decode_utf16le(bytes: &[u8]) -> Option<String> {
    if bytes.len() < 2 || !bytes.len().is_multiple_of(2) {
        return None;
    }

    let body = if bytes.starts_with(&[0xFF, 0xFE]) {
        &bytes[2..]
    } else if looks_like_utf16le(bytes) {
        bytes
    } else {
        return None;
    };

    let units: Vec<u16> = body
        .as_chunks::<2>()
        .0
        .iter()
        .map(|chunk| u16::from_le_bytes(*chunk))
        .collect();

    Some(String::from_utf16_lossy(&units))
}

fn looks_like_utf16le(bytes: &[u8]) -> bool {
    let sample_len = bytes.len().min(128);
    let units = sample_len / 2;
    if units == 0 {
        return false;
    }

    let nul_high_bytes = bytes[..sample_len]
        .as_chunks::<2>()
        .0
        .iter()
        .filter(|chunk| chunk[1] == 0)
        .count();

    nul_high_bytes * 2 >= units
}

fn is_token_expired(expires_at: Option<i64>) -> bool {
    let Some(exp) = expires_at else { return false };
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;
    now >= exp
}

/// Parse an ISO 8601 timestamp string into a SystemTime.
fn parse_iso8601(s: Option<&str>) -> Option<SystemTime> {
    let s = s?;
    let suffix = s.get(19..)?;
    let datetime_end = suffix
        .find(['+', '-'].as_ref())
        .map(|index| index + 19)
        .or_else(|| suffix.find('Z').map(|index| index + 19))
        .unwrap_or(s.len());
    let datetime_part = &s[..datetime_end];
    let offset_part = &s[datetime_end..];
    let local_seconds = parse_datetime_to_unix(datetime_part, "").ok()?;
    let offset_seconds = if offset_part.is_empty() || offset_part == "Z" {
        0_i64
    } else {
        let sign = match offset_part.as_bytes().first()? {
            b'+' => 1_i64,
            b'-' => -1_i64,
            _ => return None,
        };
        let (hours, minutes) = offset_part[1..].split_once(':')?;
        let hours: u64 = hours.parse().ok()?;
        let minutes: u64 = minutes.parse().ok()?;
        if hours > 23 || minutes > 59 {
            return None;
        }
        sign * i64::try_from(hours * 3600 + minutes * 60).ok()?
    };
    let utc_seconds = if offset_seconds >= 0 {
        local_seconds.checked_sub(offset_seconds as u64)?
    } else {
        local_seconds.checked_add(offset_seconds.unsigned_abs())?
    };
    UNIX_EPOCH.checked_add(Duration::from_secs(utc_seconds))
}

/// Minimal datetime parser — avoids pulling in chrono/time crates.
fn parse_datetime_to_unix(s: &str, _fmt: &str) -> Result<u64, ()> {
    // Extract date and time parts from "YYYY-MM-DDTHH:MM:SS[.frac]"
    let (date_str, time_str) = s.split_once('T').ok_or(())?;
    let date_parts: Vec<&str> = date_str.split('-').collect();
    if date_parts.len() != 3 {
        return Err(());
    }

    let year: u64 = date_parts[0].parse().map_err(|_| ())?;
    let month: u64 = date_parts[1].parse().map_err(|_| ())?;
    let day: u64 = date_parts[2].parse().map_err(|_| ())?;
    if !(1970..=9999).contains(&year) || !(1..=12).contains(&month) {
        return Err(());
    }

    // Strip fractional seconds
    let time_base = time_str.split('.').next().unwrap_or(time_str);
    let time_parts: Vec<&str> = time_base.split(':').collect();
    if time_parts.len() != 3 {
        return Err(());
    }

    let hour: u64 = time_parts[0].parse().map_err(|_| ())?;
    let min: u64 = time_parts[1].parse().map_err(|_| ())?;
    let sec: u64 = time_parts[2].parse().map_err(|_| ())?;
    let days_in_month = match month {
        2 if is_leap(year) => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    };
    if day == 0 || day > days_in_month || hour > 23 || min > 59 || sec > 59 {
        return Err(());
    }

    // Days from year (using a simplified calculation for dates after 1970)
    let mut days: u64 = 0;
    for y in 1970..year {
        days += if is_leap(y) { 366 } else { 365 };
    }

    let month_days = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    for m in 1..month {
        days += month_days[(m - 1) as usize];
        if m == 2 && is_leap(year) {
            days += 1;
        }
    }
    days += day - 1;

    Ok(days * 86400 + hour * 3600 + min * 60 + sec)
}

fn is_leap(y: u64) -> bool {
    (y.is_multiple_of(4) && !y.is_multiple_of(100)) || y.is_multiple_of(400)
}

/// Codex leaves absent windows as default sections. Its reported windows
/// include a reset timestamp, including windows with zero usage.
pub fn format_codex_line(
    section: &UsageSection,
    strings: Strings,
    show_remaining_in_chinese: bool,
    window: UsageWindowKind,
) -> String {
    if section.percentage == 0.0 && section.resets_at.is_none() {
        return "--".to_string();
    }
    format_line(section, strings, show_remaining_in_chinese, window)
}

/// Format a usage section for the compact taskbar display.
pub fn format_line(
    section: &UsageSection,
    strings: Strings,
    show_remaining_in_chinese: bool,
    window: UsageWindowKind,
) -> String {
    if show_remaining_in_chinese {
        return format_simplified_chinese_line(section, window);
    }

    let pct = format!("{:.0}%", section.percentage);
    let cd = format_countdown(section.resets_at, strings);
    if cd.is_empty() {
        pct
    } else {
        format!("{pct} \u{00b7} {cd}")
    }
}

fn format_simplified_chinese_line(section: &UsageSection, window: UsageWindowKind) -> String {
    let remaining = remaining_percentage(section.percentage);
    let reset = section
        .resets_at
        .and_then(native_interop::system_time_to_local);
    format_simplified_chinese_values(remaining, reset, window)
}

fn format_simplified_chinese_values(
    remaining: f64,
    reset: Option<windows::Win32::Foundation::SYSTEMTIME>,
    window: UsageWindowKind,
) -> String {
    let Some(reset) = reset else {
        return format!("{remaining:.0}%");
    };
    match window {
        UsageWindowKind::Session => {
            format!(
                "{remaining:.0}%  {:02}:{:02}重置",
                reset.wHour, reset.wMinute
            )
        }
        UsageWindowKind::Weekly => {
            format!("{remaining:.0}%  {:02}/{:02}重置", reset.wMonth, reset.wDay)
        }
    }
}

pub fn remaining_percentage(used_percentage: f64) -> f64 {
    (100.0 - used_percentage).clamp(0.0, 100.0)
}

fn format_countdown(resets_at: Option<SystemTime>, strings: Strings) -> String {
    let reset = match resets_at {
        Some(t) => t,
        None => return String::new(),
    };

    let remaining = match reset.duration_since(SystemTime::now()) {
        Ok(d) => d,
        Err(_) => return strings.now.to_string(),
    };

    format_countdown_from_secs(remaining.as_secs(), strings)
}

/// Calculate how long until the display text would change
pub fn time_until_display_change(resets_at: Option<SystemTime>) -> Option<Duration> {
    let reset = resets_at?;
    let remaining = reset.duration_since(SystemTime::now()).ok()?;
    Some(time_until_display_change_from_secs(remaining.as_secs()))
}

fn format_countdown_from_secs(total_secs: u64, strings: Strings) -> String {
    let total_mins = total_secs / 60;
    let total_hours = total_secs / 3600;
    let total_days = total_secs / 86400;

    if total_days >= 1 {
        format!("{total_days}{}", strings.day_suffix)
    } else if total_hours >= 1 {
        format!("{total_hours}{}", strings.hour_suffix)
    } else if total_mins >= 1 {
        format!("{total_mins}{}", strings.minute_suffix)
    } else {
        format!("{total_secs}{}", strings.second_suffix)
    }
}

fn time_until_display_change_from_secs(total_secs: u64) -> Duration {
    let total_mins = total_secs / 60;
    let total_hours = total_secs / 3600;
    let total_days = total_secs / 86400;

    let current_bucket_start = if total_days >= 1 {
        total_days * 86400
    } else if total_hours >= 1 {
        total_hours * 3600
    } else if total_mins >= 1 {
        total_mins * 60
    } else {
        total_secs
    };

    Duration::from_secs(total_secs.saturating_sub(current_bucket_start) + 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wsl_text_decoding_preserves_utf16_and_utf8_output() {
        let text = "Ubuntu\r\n";
        let encoded: Vec<u8> = text.encode_utf16().flat_map(u16::to_le_bytes).collect();
        assert_eq!(decode_wsl_text(&encoded), text);

        let unicode = "Ubuntu 日本語 😀\r\n";
        let with_bom: Vec<u8> = std::iter::once(0xFEFF)
            .chain(unicode.encode_utf16())
            .flat_map(u16::to_le_bytes)
            .collect();
        assert_eq!(decode_wsl_text(&with_bom), unicode);
        assert_eq!(decode_wsl_text(unicode.as_bytes()), unicode);
        assert_eq!(decode_wsl_text(&[]), "");
    }

    #[test]
    fn utf16_detection_handles_short_odd_and_invalid_input() {
        for bytes in [&[][..], &[0], &[b'A', 0, b'B']] {
            assert_eq!(decode_utf16le(bytes), None);
        }
        assert!(!looks_like_utf16le(&[]));
        // Detection ignores an incomplete trailing unit, as before.
        assert!(looks_like_utf16le(&[b'A', 0, b'B']));
        assert_eq!(decode_utf16le(&[0xFF, 0xFE]), Some(String::new()));
        assert_eq!(
            decode_utf16le(&[0xFF, 0xFE, 0x00, 0xD8]),
            Some("\u{FFFD}".into())
        );
    }

    #[test]
    fn leap_years_follow_gregorian_century_rules() {
        for (year, expected) in [(1900, false), (2000, true), (2024, true), (2025, false)] {
            assert_eq!(is_leap(year), expected, "year {year}");
        }
    }

    #[test]
    fn iso8601_parser_applies_positive_and_negative_offsets() {
        let utc = parse_iso8601(Some("2026-03-05T13:00:00Z")).unwrap();
        assert_eq!(parse_iso8601(Some("2026-03-05T08:00:00-05:00")), Some(utc));
        assert_eq!(parse_iso8601(Some("2026-03-05T14:00:00+01:00")), Some(utc));
    }

    #[test]
    fn iso8601_parser_rejects_invalid_components() {
        for timestamp in [
            "1969-12-31T23:59:59Z",
            "10000-01-01T00:00:00Z",
            "2026-00-01T00:00:00Z",
            "2026-13-01T00:00:00Z",
            "2026-02-30T00:00:00Z",
            "2026-01-00T00:00:00Z",
            "2026-01-01T24:00:00Z",
            "2026-01-01T00:60:00Z",
            "2026-01-01T00:00:60Z",
        ] {
            assert_eq!(parse_iso8601(Some(timestamp)), None, "{timestamp}");
        }
    }

    #[test]
    fn missing_codex_cli_is_a_resolution_error_not_a_fake_script_path() {
        let error = find_codex_cli(|_| None).unwrap_err();
        assert_eq!(error.kind(), std::io::ErrorKind::NotFound);
        let mut searched = Vec::new();
        let path = find_codex_cli(|name| {
            searched.push(name.to_string());
            (name == "codex.exe").then(|| PathBuf::from(r"C:\Program Files\Codex\codex.exe"))
        })
        .unwrap();
        assert_eq!(path, PathBuf::from(r"C:\Program Files\Codex\codex.exe"));
        assert_eq!(searched, ["codex.cmd", "codex.ps1", "codex.exe"]);
    }

    #[test]
    fn stale_polls_cannot_refresh_deleted_or_replaced_profile_credentials() {
        let home = std::env::var_os("LOCALAPPDATA")
            .map(|base| PathBuf::from(base).join("Temp").join("opencode"))
            .unwrap_or_else(std::env::temp_dir)
            .join(format!(
                "codex-refresh-{}-{}",
                std::process::id(),
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
        let account = CodexAccount::profile("test", home.clone());
        assert!(!codex_refresh_is_current(&account, "old-token"));
        assert!(!home.exists());
        std::fs::create_dir_all(&home).unwrap();
        std::fs::write(
            home.join("auth.json"),
            r#"{"tokens":{"access_token":"new-token","account_id":"new-account"}}"#,
        )
        .unwrap();
        assert!(!codex_refresh_is_current(&account, "old-token"));
        assert!(codex_refresh_is_current(&account, "new-token"));
        std::fs::remove_dir_all(&home).unwrap();
        assert!(!codex_refresh_is_current(&account, "new-token"));
        assert!(!home.exists());
    }

    #[test]
    fn codex_cli_discovery_preserves_unicode_installation_paths() {
        let root = std::env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(std::env::temp_dir)
            .join("Temp")
            .join("opencode")
            .join(format!("codex-cli-{}", std::process::id()));
        let directory = root.join("安装😀 O’Brien");
        std::fs::create_dir_all(&directory).unwrap();
        let executable = directory.join("codex.cmd");
        std::fs::write(&executable, "test-only discovery fixture").unwrap();
        assert_eq!(codex_cli_in_directories(&[directory]).unwrap(), executable);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn cmd_script_invocation_handles_literal_percent_in_install_path() {
        let root = std::env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(std::env::temp_dir)
            .join("Temp/opencode/cmd-script-%NAME%-test")
            .join(format!("{} & tools", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let script = root.join("codex.cmd");
        let output = root.join("result.txt");
        std::fs::write(
            &script,
            "@echo off\r\necho %* > \"%CODEX_TEST_OUTPUT%\"\r\n",
        )
        .unwrap();
        let mut command = Command::new("cmd.exe");
        configure_cmd_script(&mut command, &script, &["hello"], "/c");
        let status = command.env("CODEX_TEST_OUTPUT", &output).status().unwrap();
        assert!(status.success());
        assert_eq!(std::fs::read_to_string(output).unwrap().trim(), "hello");
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn credential_signature_detects_same_length_rewrites_within_one_second() {
        let path = std::env::temp_dir().join(format!(
            "quota-credential-signature-{}.json",
            std::process::id()
        ));
        let mut file = std::fs::File::create(&path).unwrap();
        use std::io::Write;
        file.write_all(b"fixture").unwrap();
        let time = UNIX_EPOCH + Duration::from_secs(1700000000);
        file.set_modified(time + Duration::from_millis(100))
            .unwrap();
        let before = windows_credential_watch_signature(&path);
        file.set_modified(time + Duration::from_millis(200))
            .unwrap();
        let after = windows_credential_watch_signature(&path);
        assert_ne!(before, after);
        drop(file);
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn claude_credentials_path_honors_custom_config_directory() {
        assert_eq!(
            windows_credentials_path_from(
                Some(PathBuf::from(r"D:\claude-config")),
                Some(PathBuf::from(r"C:\Users\Ray")),
            ),
            Some(PathBuf::from(r"D:\claude-config\.credentials.json"))
        );
        assert_eq!(
            windows_credentials_path_from(None, Some(PathBuf::from(r"C:\Users\Ray"))),
            Some(PathBuf::from(r"C:\Users\Ray\.claude\.credentials.json"))
        );
    }

    #[test]
    fn codex_profile_login_resolves_its_own_auth_file_and_alert_key() {
        let default = CodexAccount::default_account("Codex");
        assert!(default.is_default());
        assert_eq!(default.key, CODEX_DEFAULT_ACCOUNT_KEY);
        // The default login keeps its legacy key so existing persisted alert
        // dedup entries stay valid after upgrading.
        assert!(default.home.is_none());

        let profile = CodexAccount::profile("work", PathBuf::from(r"C:\Users\Ray\.codex-work"));
        assert!(!profile.is_default());
        assert_ne!(profile.key, CODEX_DEFAULT_ACCOUNT_KEY);
        assert_eq!(
            codex_auth_path_for(&profile),
            Some(PathBuf::from(r"C:\Users\Ray\.codex-work\auth.json"))
        );
    }

    #[test]
    fn codex_account_mismatch_needs_a_matching_echo_for_profiles() {
        // No account id was requested, so nothing can be wrong.
        assert!(!codex_account_mismatch(None, Some("other"), false));
        assert!(!codex_account_mismatch(Some(""), Some("other"), true));
        // A named, different account is always a mismatch.
        assert!(codex_account_mismatch(Some("a"), Some("b"), false));
        assert!(!codex_account_mismatch(Some("a"), Some("a"), false));
        // An empty echo is the endpoint's way of saying "not your account".
        // Only an extra profile is treated strictly; the default login has to
        // keep displaying whatever its own token reports.
        assert!(!codex_account_mismatch(Some("a"), Some(""), false));
        assert!(!codex_account_mismatch(Some("a"), None, false));
        assert!(codex_account_mismatch(Some("a"), Some(""), true));
        assert!(codex_account_mismatch(Some("a"), None, true));
    }

    #[test]
    fn codex_account_label_reads_the_stored_id_token_email() {
        let dir = std::env::temp_dir().join(format!(
            "quota-codex-label-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let account = CodexAccount::profile("", dir.clone());

        // No file yet: the caller has to fall back to another name.
        assert_eq!(codex_account_label(&account), None);

        let id_token = concat!(
            "eyJhbGciOiJub25lIn0.",
            "eyJlbWFpbCI6IndvcmtAZXhhbXBsZS5jb20iLCJodHRwczovL2FwaS5vcGVuYWkuY29tL2F1dGgiOnsiY2hhdGdwdF9wbGFuX3R5cGUiOiJwbHVzIn19",
            ".signature"
        );
        std::fs::write(
            dir.join("auth.json"),
            format!(r#"{{"tokens":{{"access_token":"token","id_token":"{id_token}"}}}}"#),
        )
        .unwrap();
        assert_eq!(
            codex_account_label(&account),
            Some("work@example.com".to_string())
        );

        // A token that is not a JWT must not panic or invent a name.
        std::fs::write(
            dir.join("auth.json"),
            r#"{"tokens":{"id_token":"not-a-jwt"}}"#,
        )
        .unwrap();
        assert_eq!(codex_account_label(&account), None);

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn remaining_percentage_is_clamped() {
        assert_eq!(remaining_percentage(30.0), 70.0);
        assert_eq!(remaining_percentage(-5.0), 100.0);
        assert_eq!(remaining_percentage(120.0), 0.0);
    }

    #[test]
    fn codex_weekly_only_window_is_not_misreported_as_session_usage() {
        let response: CodexUsageResponse = serde_json::from_str(
            r#"{
                "rate_limit": {
                    "primary_window": {
                        "used_percent": 21,
                        "limit_window_seconds": 604800,
                        "reset_at": 1784500338
                    },
                    "secondary_window": null
                }
            }"#,
        )
        .expect("weekly-only response should deserialize");

        let usage = codex_usage_from_response(response).expect("rate limit should be available");

        assert_eq!(usage.session.percentage, 0.0);
        assert!(usage.session.resets_at.is_none());
        assert_eq!(usage.weekly.percentage, 21.0);
        assert!(usage.weekly.resets_at.is_some());
        for language in [
            crate::localization::LanguageId::SimplifiedChinese,
            crate::localization::LanguageId::English,
        ] {
            assert_eq!(
                format_codex_line(
                    &usage.session,
                    language.strings(),
                    language == crate::localization::LanguageId::SimplifiedChinese,
                    UsageWindowKind::Session,
                ),
                "--"
            );
        }
    }

    #[test]
    fn codex_legacy_windows_keep_positional_mapping_without_durations() {
        let response: CodexUsageResponse = serde_json::from_str(
            r#"{
                "rate_limit": {
                    "primary_window": {
                        "used_percent": 18,
                        "reset_at": 1784500338
                    },
                    "secondary_window": {
                        "used_percent": 33,
                        "reset_at": 1785000000
                    }
                }
            }"#,
        )
        .expect("legacy response should deserialize");

        let usage = codex_usage_from_response(response).expect("rate limit should be available");

        assert_eq!(usage.session.percentage, 18.0);
        assert_eq!(usage.weekly.percentage, 33.0);
    }

    #[test]
    fn codex_session_only_zero_usage_keeps_real_quota_visible() {
        let response: CodexUsageResponse = serde_json::from_str(
            r#"{
                "rate_limit": {
                    "primary_window": {
                        "used_percent": 0,
                        "limit_window_seconds": 18000,
                        "reset_at": 1784500338
                    },
                    "secondary_window": null
                }
            }"#,
        )
        .unwrap();
        let usage = codex_usage_from_response(response).unwrap();
        let strings = crate::localization::LanguageId::SimplifiedChinese.strings();
        assert!(
            format_codex_line(&usage.session, strings, true, UsageWindowKind::Session)
                .starts_with("100%")
        );
        assert_eq!(
            format_codex_line(&usage.weekly, strings, true, UsageWindowKind::Weekly),
            "--"
        );
    }

    #[test]
    fn rate_limit_response_preserves_valid_retry_hints() {
        for (header, expected) in [("120", Some(120_000)), ("invalid", None)] {
            let response: ureq::Response =
                format!("HTTP/1.1 429 Too Many Requests\r\nRetry-After: {header}\r\n\r\n")
                    .parse()
                    .unwrap();
            assert_eq!(
                classify_ureq_error(&ureq::Error::Status(429, response)),
                PollError::RateLimited(expected)
            );
        }
    }

    #[test]
    fn classifies_http_failures_for_user_visible_recovery() {
        assert_eq!(classify_http_status(401), PollError::AuthRequired);
        assert_eq!(classify_http_status(403), PollError::AuthRequired);
        assert_eq!(classify_http_status(429), PollError::RateLimited(None));
        assert_eq!(classify_http_status(500), PollError::ServerError);
        assert_eq!(classify_http_status(503), PollError::ServerError);
        assert_eq!(classify_http_status(404), PollError::RequestFailed);
        assert_eq!(
            PollError::NetworkUnavailable.category(),
            "network_unavailable"
        );
    }

    #[test]
    fn simplified_chinese_line_labels_remaining_usage() {
        let strings = crate::localization::LanguageId::SimplifiedChinese.strings();
        assert_eq!(strings.session_window, "5h");
        assert_eq!(strings.weekly_window, "7d");
        let section = UsageSection {
            percentage: 30.0,
            resets_at: None,
        };
        assert_eq!(
            format_line(&section, strings, true, UsageWindowKind::Session),
            "70%"
        );
        let session_reset = windows::Win32::Foundation::SYSTEMTIME {
            wHour: 18,
            wMinute: 30,
            ..Default::default()
        };
        assert_eq!(
            format_simplified_chinese_values(82.0, Some(session_reset), UsageWindowKind::Session,),
            "82%  18:30重置"
        );
        let weekly_reset = windows::Win32::Foundation::SYSTEMTIME {
            wMonth: 7,
            wDay: 17,
            ..Default::default()
        };
        assert_eq!(
            format_simplified_chinese_values(97.0, Some(weekly_reset), UsageWindowKind::Weekly,),
            "97%  07/17重置"
        );
    }

    #[test]
    fn antigravity_summary_prefers_gemini_group() {
        let response: AntigravityQuotaSummaryResponse = serde_json::from_str(
            r#"{
                "groups": [
                    {
                        "displayName": "Claude and GPT models",
                        "buckets": [
                            {
                                "bucketId": "3p-weekly",
                                "window": "weekly",
                                "resetTime": "2026-06-20T18:32:02Z",
                                "remainingFraction": 1
                            },
                            {
                                "bucketId": "3p-5h",
                                "window": "5h",
                                "resetTime": "2026-06-13T23:32:02Z",
                                "remainingFraction": 1
                            }
                        ]
                    },
                    {
                        "displayName": "Gemini Models",
                        "description": "Models within this group: Gemini Flash, Gemini Pro",
                        "buckets": [
                            {
                                "bucketId": "gemini-weekly",
                                "displayName": "Weekly Limit",
                                "window": "weekly",
                                "resetTime": "2026-06-20T17:08:54Z",
                                "remainingFraction": 0.99304295
                            },
                            {
                                "bucketId": "gemini-5h",
                                "displayName": "Five Hour Limit",
                                "window": "5h",
                                "resetTime": "2026-06-13T22:08:54Z",
                                "remainingFraction": 0.9582575
                            }
                        ]
                    }
                ]
            }"#,
        )
        .expect("summary response should deserialize");

        let usage =
            antigravity_usage_from_summary(response).expect("Gemini quota should be selected");

        assert!((usage.weekly.percentage - 0.695705).abs() < 0.000001);
        assert!((usage.session.percentage - 4.17425).abs() < 0.000001);
        assert!(usage.weekly.resets_at.is_some());
        assert!(usage.session.resets_at.is_some());
    }
}
