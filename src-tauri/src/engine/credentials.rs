//! Credential resolution for the git transports (docs/02 §4; docs/03 §8).
//!
//! # Transports
//!
//! `git2` is declared in `Cargo.toml` with the `ssh` and `https` features, so
//! libgit2 is built with the ssh (libssh2) and https (openssl) transports
//! alongside the always-present `git://`/`http://` handlers. Every one of them
//! routes an authentication challenge through [`callback`], which resolves a
//! real credential — the ssh-agent and the on-disk keys under `~/.ssh` for ssh,
//! the `gitcredentials(7)` helper protocol for https — and **never** returns a
//! bare [`git2::Cred::default`] (a Negotiate-only credential with no payload,
//! which every authenticated remote rejects).
//!
//! # Why the helper is spoken here, not via git2
//!
//! `git2::Cred::credential_helper` (available under the `https` feature) reads
//! the global configuration only. We speak the `gitcredentials(7)` helper
//! protocol ourselves so the request is built from the *repository's* own
//! configuration — a repo-scoped `credential.helper`, `credential.username` and
//! `credential.useHttpPath` all count — and so we can honour a helper's
//! `quit=1` veto and carry a typed failure back through libgit2 (see below).
//!
//! # The chain
//!
//! For a request that allows an SSH key: ssh-agent, then the on-disk keys under
//! `~/.ssh` in git's own order (`id_ed25519`, `id_ecdsa`, `id_rsa`), honouring
//! the username from the URL or `~/.ssh/config` when one is known. For a request
//! that allows plain user/pass: each configured `credential.helper`, fed the
//! protocol/host/(optional) path request on stdin. libgit2 re-invokes the
//! callback after a rejected credential, so the callback is **stateful** and
//! offers the next candidate on each call.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use git2::CredentialType;

use crate::domain::error::{ErrorCode, ErrorDomain, ErrorPayload};

/// Private key filenames searched under `~/.ssh`, in the order git offers them.
pub const SSH_KEY_FILES: [&str; 3] = ["id_ed25519", "id_ecdsa", "id_rsa"];

/// Build the callback for [`git2::RemoteCallbacks::credentials`]. Each call to
/// this function returns a fresh, stateful chain, so one callback belongs to one
/// fetch/push/connect. The repository supplies the credential configuration
/// (helpers and `credential.useHttpPath`), so a repo-scoped setting counts
/// exactly as git counts it.
pub fn callback(
    repo: &git2::Repository,
) -> impl FnMut(&str, Option<&str>, CredentialType) -> Result<git2::Cred, git2::Error> {
    let config = CredentialConfig::for_repo(repo);
    let mut state = ChainState::default();
    move |url, username, allowed| {
        state
            .next(&config, url, username, allowed)
            .map_err(|payload| git2::Error::from_str(&encode_credential_error(&payload)))
    }
}

/// Resolve the first credential the `allowed` mask accepts. A convenience
/// wrapper over [`ChainState`] for direct callers; the transport routes through
/// [`callback`] so it can offer the following candidate on a retry.
pub fn credential_for(
    url: &str,
    username: Option<&str>,
    allowed: CredentialType,
) -> Result<git2::Cred, ErrorPayload> {
    ChainState::default().next(&CredentialConfig::default(), url, username, allowed)
}

/// The credential settings a chain needs: the `credential.helper` programs and
/// whether `credential.useHttpPath` makes the helper request carry the path.
/// Extracted once per callback, from the repository's own configuration.
#[derive(Clone, Debug, Default)]
struct CredentialConfig {
    helpers: Vec<String>,
    use_http_path: bool,
}

impl CredentialConfig {
    fn for_repo(repo: &git2::Repository) -> Self {
        Self {
            helpers: credential_helpers(repo),
            use_http_path: repo
                .config()
                .ok()
                .and_then(|config| config.get_bool("credential.useHttpPath").ok())
                .unwrap_or(false),
        }
    }
}

/// The stateful credential chain. libgit2 asks again after each rejected
/// credential, so the step counters remember how far we have offered.
#[derive(Default)]
struct ChainState {
    ssh_step: usize,
    http_step: usize,
}

impl ChainState {
    fn next(
        &mut self,
        config: &CredentialConfig,
        url: &str,
        username: Option<&str>,
        allowed: CredentialType,
    ) -> Result<git2::Cred, ErrorPayload> {
        if allowed.contains(CredentialType::SSH_KEY) {
            return self.next_ssh(url, username);
        }
        if allowed.contains(CredentialType::USER_PASS_PLAINTEXT) {
            return self.next_http(config, url, username);
        }
        if allowed.contains(CredentialType::USERNAME) {
            // libgit2 wants a bare username (it builds the ssh user from it).
            if let Some(user) = username.map(str::to_string).or_else(|| url_username(url)) {
                return Ok(git2::Cred::username(&user)?);
            }
            return Err(ErrorPayload::new(
                ErrorCode::InvalidRequest,
                format!("No username is available for '{url}'"),
            )
            .with_domain(ErrorDomain::Service)
            .with_param("class", "username_required"));
        }
        Err(ErrorPayload::new(
            ErrorCode::InvalidRequest,
            format!(
                "The server asked for a credential type Meridian cannot provide \
                 (allowed = {allowed:?}); configure a git credential helper"
            ),
        )
        .with_domain(ErrorDomain::Service)
        .with_param("class", "unsupported_credential_type"))
    }

    fn next_ssh(&mut self, url: &str, username: Option<&str>) -> Result<git2::Cred, ErrorPayload> {
        // Step 0 is the ssh-agent; the on-disk keys follow in git's order.
        if self.ssh_step == 0 {
            self.ssh_step = 1;
            let user = ssh_user(url, username);
            if let Ok(cred) = git2::Cred::ssh_key_from_agent(&user) {
                return Ok(cred);
            }
        }

        let home = home_dir();
        let keys = ssh_key_paths(&home);
        if let Some(private) = keys.get(self.ssh_step - 1) {
            self.ssh_step += 1;
            let user = ssh_user(url, username);
            let public = public_key_for(private);
            return Ok(git2::Cred::ssh_key(
                &user,
                public.as_deref(),
                private,
                None,
            )?);
        }

        // Nothing left to offer. Say which of the two situations this is: no
        // key at all, or keys that were offered and refused (passphrase-locked
        // with no agent, or rejected by the server).
        let message = if keys.is_empty() {
            format!(
                "No usable SSH credential for '{url}': the ssh-agent could not be reached \
                 and no key was found in {}/.ssh ({}). Add an SSH key, or use an HTTPS \
                 remote with a git credential helper.",
                home.display(),
                SSH_KEY_FILES.join(", ")
            )
        } else {
            format!(
                "No usable SSH credential for '{url}': the ssh-agent could not be reached \
                 and none of the {} key(s) in {}/.ssh were accepted (passphrase-locked or \
                 rejected by the server). Add the key to the agent, or use an HTTPS remote \
                 with a git credential helper.",
                keys.len(),
                home.display()
            )
        };
        Err(ErrorPayload::new(ErrorCode::InvalidRequest, message)
            .with_domain(ErrorDomain::Service)
            .with_param("class", "ssh_unavailable"))
    }

    fn next_http(
        &mut self,
        config: &CredentialConfig,
        url: &str,
        username: Option<&str>,
    ) -> Result<git2::Cred, ErrorPayload> {
        if let Some(helper) = config.helpers.get(self.http_step) {
            self.http_step += 1;
            let request = credential_request(url, username, config.use_http_path);
            if let Ok(reply) = run_credential_helper(helper, &request) {
                if reply.quit {
                    // gitcredentials(7): `quit=1` from a helper's `get` is a veto
                    // — stop here rather than consulting the remaining helpers,
                    // which could hand back a stale credential the user just
                    // rejected.
                    return Err(ErrorPayload::new(
                        ErrorCode::InvalidRequest,
                        format!(
                            "The credential helper for '{url}' refused to answer: the \
                             stored credential was rejected or has been removed."
                        ),
                    )
                    .with_domain(ErrorDomain::Service)
                    .with_param("class", "credential_rejected"));
                }
                if let (Some(user), Some(pass)) = (reply.username, reply.password) {
                    return Ok(git2::Cred::userpass_plaintext(&user, &pass)?);
                }
            }
            // This helper had nothing; try the next.
            return self.next_http(config, url, username);
        }

        Err(credential_unavailable(url))
    }
}

/// The error for "every helper was consulted and none produced a credential".
fn credential_unavailable(url: &str) -> ErrorPayload {
    ErrorPayload::new(
        ErrorCode::InvalidRequest,
        format!(
            "No credential is available for '{url}'. Configure a git credential \
             helper (for example `git config --global credential.helper osxkeychain`)."
        ),
    )
    .with_domain(ErrorDomain::Service)
    .with_param("class", "credential_unavailable")
}

// ---------------------------------------------------------------------------
// SSH
// ---------------------------------------------------------------------------

/// The username to present over ssh: the URL's username, the callback's, the
/// `~/.ssh/config` `User` for the host, or git's own default.
fn ssh_user(url: &str, username: Option<&str>) -> String {
    username
        .map(str::to_string)
        .or_else(|| url_username(url))
        .or_else(|| {
            let host = parse_url(url).map(|parts| parts.host)?;
            ssh_config_user(&home_dir(), &host)
        })
        .unwrap_or_else(|| "git".to_string())
}

/// The private keys present under `<home>/.ssh`, in git's search order. Only
/// files that exist are returned, so the caller can offer them in turn.
pub fn ssh_key_paths(home: &Path) -> Vec<PathBuf> {
    let ssh_dir = home.join(".ssh");
    SSH_KEY_FILES
        .iter()
        .map(|name| ssh_dir.join(name))
        .filter(|path| path.is_file())
        .collect()
}

/// The first private key git would try, or `None` when `~/.ssh` has none.
pub fn ssh_private_key(home: &Path) -> Option<PathBuf> {
    ssh_key_paths(home).into_iter().next()
}

/// The matching `~/.ssh/config` `User`, when a `Host` block for `host` names one.
pub fn ssh_config_user(home: &Path, host: &str) -> Option<String> {
    let text = std::fs::read_to_string(home.join(".ssh").join("config")).ok()?;
    let host = host.to_ascii_lowercase();
    let mut block_matches = false;
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = split_directive(line) else {
            continue;
        };
        match key.as_str() {
            "host" => {
                block_matches = value
                    .split_whitespace()
                    .any(|pattern| glob_matches(&pattern.to_ascii_lowercase(), &host));
            }
            "user" if block_matches => {
                return value.split_whitespace().next().map(str::to_string);
            }
            _ => {}
        }
    }
    None
}

/// A `.pub` sibling of a private key, when it exists.
fn public_key_for(private: &Path) -> Option<PathBuf> {
    let public = PathBuf::from(format!("{}.pub", private.display()));
    public.is_file().then_some(public)
}

fn home_dir() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}

// ---------------------------------------------------------------------------
// git credential helper protocol (gitcredentials(7))
// ---------------------------------------------------------------------------

/// One helper's parsed reply to a `get` request.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct HelperReply {
    pub username: Option<String>,
    pub password: Option<String>,
    /// The helper answered `quit=1`: stop asking, it has nothing.
    pub quit: bool,
}

/// Parse the `key=value` lines a credential helper writes to stdout. Parsing
/// stops at the blank line that terminates the record; `quit=1` is honoured as
/// a rejection (no credentials, don't ask more helpers).
pub fn parse_helper_output(stdout: &str) -> HelperReply {
    let mut reply = HelperReply::default();
    for line in stdout.lines() {
        if line.trim().is_empty() {
            break;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        match key.trim() {
            "username" => reply.username = Some(value.trim().to_string()),
            "password" => reply.password = Some(value.trim().to_string()),
            "quit" => reply.quit = matches!(value.trim(), "1" | "true"),
            _ => {}
        }
    }
    reply
}

/// Read the configured `credential.helper` values (git's own list; an empty
/// value resets the list). Read from the repository's configuration, which
/// includes the local `.git/config` as well as the global/system files, so a
/// repo-scoped `credential.helper` is honoured exactly as git honours it.
fn credential_helpers(repo: &git2::Repository) -> Vec<String> {
    let Ok(config) = repo.config() else {
        return Vec::new();
    };
    let mut helpers = Vec::new();
    let Ok(mut entries) = config.entries(Some("credential.helper")) else {
        return helpers;
    };
    while let Some(entry) = entries.next() {
        let Ok(entry) = entry else { continue };
        if entry.name().ok() != Some("credential.helper") {
            continue;
        }
        let Ok(value) = entry.value() else { continue };
        let value = value.trim();
        if value.is_empty() {
            helpers.clear();
        } else {
            helpers.push(value.to_string());
        }
    }
    helpers
}

/// Split a `credential.helper` value into the program and its arguments, the way
/// git does: `!cmd` is a shell command; a path is run directly; anything else is
/// `git credential-<name>` with the remaining words as arguments.
fn helper_command(helper: &str) -> (String, Vec<String>) {
    let helper = helper.trim();
    if let Some(command) = helper.strip_prefix('!') {
        return (
            "sh".to_string(),
            vec!["-c".to_string(), command.to_string(), helper.to_string()],
        );
    }
    let mut words = helper.split_whitespace();
    let first = words.next().unwrap_or_default().to_string();
    let rest: Vec<String> = words.map(str::to_string).collect();
    if first.contains('/') {
        (first, rest)
    } else {
        let mut args = vec![format!("credential-{first}")];
        args.extend(rest);
        ("git".to_string(), args)
    }
}

/// Invoke `helper` with a `get` request for `url`, returning its parsed reply.
fn run_credential_helper(helper: &str, request: &str) -> std::io::Result<HelperReply> {
    let (program, mut args) = helper_command(helper);
    args.push("get".to_string());

    let mut child = Command::new(&program)
        .args(&args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(request.as_bytes())?;
    }
    let output = child.wait_with_output()?;
    Ok(parse_helper_output(&String::from_utf8_lossy(
        &output.stdout,
    )))
}

/// The `get` request body for a URL (protocol, host, optional username/path,
/// then the blank line that ends the record).
///
/// `path` is only included when the caller asks (`credential.useHttpPath`), the
/// way git does it: the CLI stores host-only credentials by default, so sending
/// `path` unconditionally makes the helper miss credentials `git` itself finds.
fn credential_request(url: &str, username: Option<&str>, use_http_path: bool) -> String {
    let mut request = String::new();
    if let Some(parts) = parse_url(url) {
        request.push_str(&format!("protocol={}\n", parts.protocol));
        request.push_str(&format!("host={}\n", parts.host));
        if let Some(user) = username.or(parts.username.as_deref()) {
            request.push_str(&format!("username={user}\n"));
        }
        if use_http_path {
            if let Some(path) = &parts.path {
                request.push_str(&format!("path={path}\n"));
            }
        }
    } else {
        request.push_str(&format!("url={url}\n"));
    }
    request.push('\n');
    request
}

// ---------------------------------------------------------------------------
// Carrying a structured failure through libgit2
// ---------------------------------------------------------------------------

/// A credential failure can only cross libgit2 as a [`git2::Error`], which
/// carries a message and nothing else — the structured `class` we attach is lost
/// by the time the fetch/push caller sees the error. So the message is prefixed
/// with the class and [`classify`](super::error::classify) unpacks it, keeping
/// the class the UI branches on.
const CLASS_MARKER: &str = "\u{1f}meridian-credential\u{1f}";

/// Wrap a credential [`ErrorPayload`] so its `class` survives the round trip
/// through libgit2 as a plain error message.
pub fn encode_credential_error(payload: &ErrorPayload) -> String {
    let class = payload
        .params
        .get("class")
        .map(String::as_str)
        .unwrap_or("");
    format!("{CLASS_MARKER}{class}\u{1f}{}", payload.message)
}

/// Recover the [`ErrorPayload`] a credential failure was encoded from, when a
/// message carries [`CLASS_MARKER`]. `None` for any other message.
pub fn decode_credential_error(message: &str) -> Option<ErrorPayload> {
    let rest = message.strip_prefix(CLASS_MARKER)?;
    let (class, text) = rest.split_once('\u{1f}')?;
    let payload = ErrorPayload::new(ErrorCode::InvalidRequest, text.to_string())
        .with_domain(ErrorDomain::Service);
    Some(if class.is_empty() {
        payload
    } else {
        payload.with_param("class", class)
    })
}

// ---------------------------------------------------------------------------
// URL parsing
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Eq)]
struct UrlParts {
    protocol: String,
    host: String,
    path: Option<String>,
    username: Option<String>,
}

/// Parse the remote URL forms git accepts: scheme URLs (`https://`, `ssh://`,
/// `git://`) and the scp-like `[user@]host:path` shorthand (which is ssh).
fn parse_url(url: &str) -> Option<UrlParts> {
    if let Some((scheme, rest)) = url.split_once("://") {
        let (authority, path) = match rest.split_once('/') {
            Some((authority, path)) => (authority, Some(path.to_string())),
            None => (rest, None),
        };
        let (username, hostport) = match authority.rsplit_once('@') {
            Some((user, hostport)) => (Some(user.to_string()), hostport),
            None => (None, authority),
        };
        let host = hostport.split(':').next().unwrap_or(hostport).to_string();
        return Some(UrlParts {
            protocol: scheme.to_string(),
            host,
            path,
            username,
        });
    }

    // scp-like shorthand: a colon that is not part of a path separator.
    if let Some((userhost, path)) = url.split_once(':') {
        if !userhost.contains('/') && !userhost.is_empty() {
            let (username, host) = match userhost.rsplit_once('@') {
                Some((user, host)) => (Some(user.to_string()), host.to_string()),
                None => (None, userhost.to_string()),
            };
            return Some(UrlParts {
                protocol: "ssh".to_string(),
                host,
                path: Some(path.to_string()),
                username,
            });
        }
    }
    None
}

/// The username embedded in a URL, when there is one.
fn url_username(url: &str) -> Option<String> {
    parse_url(url).and_then(|parts| parts.username)
}

/// Split one `~/.ssh/config` line into a lowercased directive and its value,
/// accepting both `Key value` and `Key=value`.
fn split_directive(line: &str) -> Option<(String, String)> {
    let (key, value) = match line.split_once('=') {
        Some((key, value)) => (key.trim(), value.trim()),
        None => {
            let mut parts = line.splitn(2, char::is_whitespace);
            let key = parts.next()?.trim();
            let value = parts.next().unwrap_or("").trim();
            (key, value)
        }
    };
    Some((key.to_ascii_lowercase(), value.to_string()))
}

/// A small `fnmatch` subset for `Host` patterns: `*` matches any run, `?` one
/// character; matching is anchored.
fn glob_matches(pattern: &str, text: &str) -> bool {
    fn matches(pattern: &[char], text: &[char]) -> bool {
        match pattern.split_first() {
            None => text.is_empty(),
            Some(('*', rest)) => (0..=text.len()).any(|skip| matches(rest, &text[skip..])),
            Some(('?', rest)) => match text.split_first() {
                Some((_, tail)) => matches(rest, tail),
                None => false,
            },
            Some((&expected, rest)) => match text.split_first() {
                Some((&found, tail)) => expected == found && matches(rest, tail),
                None => false,
            },
        }
    }
    let pattern: Vec<char> = pattern.chars().collect();
    let text: Vec<char> = text.chars().collect();
    matches(&pattern, &text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    fn temp_home(tag: &str) -> PathBuf {
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let serial = COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!(
            "meridian-credentials-{}-{tag}-{serial}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    // -----------------------------------------------------------------
    // credential-helper output
    // -----------------------------------------------------------------

    #[test]
    fn helper_output_parses_a_realistic_reply() {
        // Exactly the shape `git credential-osxkeychain`/`-store` write.
        let stdout = "protocol=https\nhost=github.com\nusername=octocat\npassword=hunter2\n\n";
        let reply = parse_helper_output(stdout);
        assert_eq!(reply.username.as_deref(), Some("octocat"));
        assert_eq!(reply.password.as_deref(), Some("hunter2"));
        assert!(!reply.quit);
    }

    #[test]
    fn helper_output_honours_a_quit_rejection() {
        let reply = parse_helper_output("quit=1\n\n");
        assert!(reply.quit, "quit=1 must be read as a rejection");
        assert_eq!(reply.username, None);
        assert_eq!(reply.password, None);
    }

    #[test]
    fn helper_output_stops_at_the_blank_line_terminator() {
        let stdout = "username=alice\npassword=secret\n\nusername=ignored\npassword=ignored\n";
        let reply = parse_helper_output(stdout);
        assert_eq!(reply.username.as_deref(), Some("alice"));
        assert_eq!(reply.password.as_deref(), Some("secret"));
    }

    #[test]
    fn helper_output_ignores_lines_that_are_not_key_value_pairs() {
        let reply = parse_helper_output("warning: something\nusername=bob\npassword=pw\n");
        assert_eq!(reply.username.as_deref(), Some("bob"));
        assert_eq!(reply.password.as_deref(), Some("pw"));
    }

    #[test]
    fn helper_command_matches_gits_own_rules() {
        assert_eq!(
            helper_command("store"),
            ("git".to_string(), vec!["credential-store".to_string()])
        );
        assert_eq!(
            helper_command("cache --timeout=300"),
            (
                "git".to_string(),
                vec!["credential-cache".to_string(), "--timeout=300".to_string()]
            )
        );
        assert_eq!(
            helper_command("/usr/local/bin/helper"),
            ("/usr/local/bin/helper".to_string(), Vec::new())
        );
        let (program, args) = helper_command("!f() { cat; }; f");
        assert_eq!(program, "sh");
        assert_eq!(args[0], "-c");
        assert_eq!(args[1], "f() { cat; }; f");
    }

    // -----------------------------------------------------------------
    // ssh key search order
    // -----------------------------------------------------------------

    #[test]
    fn ssh_keys_are_searched_in_gits_order() {
        let home = temp_home("order");
        let ssh = home.join(".ssh");
        std::fs::create_dir_all(&ssh).unwrap();
        for name in ["id_rsa", "id_ecdsa", "id_ed25519"] {
            std::fs::write(ssh.join(name), "PRIVATE KEY").unwrap();
        }

        let names: Vec<String> = ssh_key_paths(&home)
            .iter()
            .map(|path| path.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, vec!["id_ed25519", "id_ecdsa", "id_rsa"]);
        assert_eq!(ssh_private_key(&home).unwrap(), ssh.join("id_ed25519"));

        // Removing the preferred key promotes the next one.
        std::fs::remove_file(ssh.join("id_ed25519")).unwrap();
        assert_eq!(ssh_private_key(&home).unwrap(), ssh.join("id_ecdsa"));

        std::fs::remove_file(ssh.join("id_ecdsa")).unwrap();
        assert_eq!(ssh_private_key(&home).unwrap(), ssh.join("id_rsa"));

        std::fs::remove_file(ssh.join("id_rsa")).unwrap();
        assert!(ssh_private_key(&home).is_none());

        let _ = std::fs::remove_dir_all(&home);
    }

    #[test]
    fn ssh_config_user_is_read_for_the_matching_host() {
        let home = temp_home("sshcfg");
        let ssh = home.join(".ssh");
        std::fs::create_dir_all(&ssh).unwrap();
        std::fs::write(
            ssh.join("config"),
            "# a comment\nHost github.com\n    User octocat\n\nHost *.example.com\n    User deploy\n",
        )
        .unwrap();

        assert_eq!(
            ssh_config_user(&home, "github.com").as_deref(),
            Some("octocat")
        );
        assert_eq!(
            ssh_config_user(&home, "api.example.com").as_deref(),
            Some("deploy")
        );
        assert_eq!(ssh_config_user(&home, "gitlab.com"), None);

        let _ = std::fs::remove_dir_all(&home);
    }

    // -----------------------------------------------------------------
    // URL parsing
    // -----------------------------------------------------------------

    #[test]
    fn urls_parse_in_both_forms() {
        let https = parse_url("https://octocat@github.com/owner/repo.git").unwrap();
        assert_eq!(https.protocol, "https");
        assert_eq!(https.host, "github.com");
        assert_eq!(https.username.as_deref(), Some("octocat"));
        assert_eq!(https.path.as_deref(), Some("owner/repo.git"));

        let ssh = parse_url("ssh://git@github.com:22/owner/repo.git").unwrap();
        assert_eq!(ssh.protocol, "ssh");
        assert_eq!(ssh.host, "github.com");
        assert_eq!(ssh.username.as_deref(), Some("git"));

        let scp = parse_url("git@github.com:owner/repo.git").unwrap();
        assert_eq!(scp.protocol, "ssh");
        assert_eq!(scp.host, "github.com");
        assert_eq!(scp.username.as_deref(), Some("git"));
        assert_eq!(scp.path.as_deref(), Some("owner/repo.git"));

        assert!(parse_url("/local/path/repo").is_none());
    }

    #[test]
    fn a_credential_request_carries_the_helper_fields() {
        let request =
            credential_request("https://github.com/owner/repo.git", Some("octocat"), false);
        assert!(request.contains("protocol=https\n"));
        assert!(request.contains("host=github.com\n"));
        assert!(request.contains("username=octocat\n"));
        assert!(
            request.ends_with("\n\n"),
            "the record ends with a blank line"
        );
    }

    #[test]
    fn a_credential_request_omits_path_unless_use_http_path() {
        // git sends `path` only when `credential.useHttpPath` is set (default
        // false): the CLI stores host-only credentials, so an unconditional
        // `path` makes the helper miss a credential `git` itself finds.
        let host_only = credential_request("https://github.com/owner/repo.git", None, false);
        assert!(
            !host_only.contains("path="),
            "path must be omitted by default"
        );
        assert!(host_only.contains("host=github.com\n"));

        let with_path = credential_request("https://github.com/owner/repo.git", None, true);
        assert!(with_path.contains("path=owner/repo.git\n"));
    }

    // -----------------------------------------------------------------
    // credential-helper subprocess (gitcredentials(7))
    // -----------------------------------------------------------------

    /// Write an executable helper script that ignores its args and stdin and
    /// prints `body` to stdout.
    fn helper_script(dir: &Path, name: &str, body: &str) -> PathBuf {
        let path = dir.join(name);
        std::fs::write(&path, format!("#!/bin/sh\ncat >/dev/null\n{body}")).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(&path).unwrap().permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(&path, perms).unwrap();
        }
        path
    }

    fn temp_repo(tag: &str) -> (git2::Repository, PathBuf) {
        let dir = temp_home(tag);
        let repo = git2::Repository::init(&dir).expect("init a scratch repository");
        (repo, dir)
    }

    #[test]
    fn a_configured_helper_supplies_userpass_credentials() {
        let dir = temp_home("helper-run");
        let helper = helper_script(
            &dir,
            "helper.sh",
            "printf 'username=alice\\npassword=secret\\n\\n'\n",
        );
        let config = CredentialConfig {
            helpers: vec![helper.to_string_lossy().into_owned()],
            use_http_path: false,
        };

        let cred = ChainState::default()
            .next(
                &config,
                "https://example.invalid/owner/repo.git",
                None,
                CredentialType::USER_PASS_PLAINTEXT,
            )
            .expect("the helper supplies a credential");
        assert!(cred.has_username());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_helper_quit_veto_stops_the_chain() {
        // Helper A refuses with `quit=1`; helper B *would* answer. git treats
        // `quit=1` as a veto, so B must never be consulted.
        let dir = temp_home("helper-quit");
        let quitter = helper_script(&dir, "quit.sh", "printf 'quit=1\\n\\n'\n");
        let fallback = helper_script(
            &dir,
            "fallback.sh",
            "printf 'username=fallback\\npassword=pw\\n\\n'\n",
        );
        let config = CredentialConfig {
            helpers: vec![
                quitter.to_string_lossy().into_owned(),
                fallback.to_string_lossy().into_owned(),
            ],
            use_http_path: false,
        };

        let error = match ChainState::default().next(
            &config,
            "https://example.invalid/owner/repo.git",
            None,
            CredentialType::USER_PASS_PLAINTEXT,
        ) {
            Ok(_) => panic!("a quit veto must not yield the next helper's credential"),
            Err(error) => error,
        };
        assert_eq!(
            error.params.get("class").map(String::as_str),
            Some("credential_rejected"),
            "the failure must be reported as the veto, not as a missing credential"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn credential_helpers_reads_repository_local_config() {
        // The helper list must include a repo-scoped `credential.helper`, which
        // `Config::open_default()` (global/xdg/system only) could never see.
        let (repo, dir) = temp_repo("helper-cfg");
        let mut config = repo.config().unwrap();
        config
            .set_str(
                "credential.helper",
                "store --file=/tmp/meridian-helper-test",
            )
            .unwrap();

        let helpers = credential_helpers(&repo);
        assert!(
            helpers.iter().any(|h| h.starts_with("store --file=")),
            "the repo-local credential.helper must be read, got {helpers:?}"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_credential_class_survives_the_libgit2_round_trip() {
        // libgit2 carries only the message; the structured class is encoded in
        // it and must decode back so the UI can branch on the failure.
        let payload = credential_unavailable("https://example.invalid/o/r.git");
        let encoded = encode_credential_error(&payload);
        let decoded = decode_credential_error(&encoded).expect("the marker decodes");
        assert_eq!(
            decoded.params.get("class").map(String::as_str),
            Some("credential_unavailable")
        );
        assert!(decoded.message.contains("https://example.invalid/o/r.git"));
        assert!(decode_credential_error("an ordinary libgit2 message").is_none());
    }

    // -----------------------------------------------------------------
    // the chain never yields a bare Negotiate default
    // -----------------------------------------------------------------

    #[test]
    fn a_default_only_request_is_refused() {
        // `CredentialType::DEFAULT` is the Negotiate-only bit that
        // `git2::Cred::default()` satisfies. We must refuse rather than return
        // an empty credential that every authenticated remote rejects.
        let payload = match credential_for(
            "https://github.com/owner/repo.git",
            None,
            CredentialType::DEFAULT,
        ) {
            Err(payload) => payload,
            Ok(_) => panic!("a bare default request must not produce a credential"),
        };
        assert_eq!(
            payload.params.get("class").map(String::as_str),
            Some("unsupported_credential_type")
        );
    }

    #[test]
    fn a_username_only_request_uses_the_url_username() {
        let cred = credential_for(
            "ssh://git@github.com/owner/repo.git",
            None,
            CredentialType::USERNAME,
        )
        .expect("the url carries a username");
        assert!(cred.has_username());
    }
}
