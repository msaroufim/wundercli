use std::collections::HashMap;
use std::env;
use std::fs;
use std::io;
use std::process::Command;

use serde::{Deserialize, Serialize};

use todo::{
    ACTIVE_FILE, COMPLETED_FILE, Todo, data_file, latest_local_update, read_all_todos,
    write_all_todos,
};

pub const GIST_FILE_NAME: &str = "wundercli.json";
pub const GIST_ID_ENV_VAR: &str = "TODO_GIST_ID";
pub const GIST_TOKEN_ENV_VAR: &str = "TODO_GIST_TOKEN";
pub const GITHUB_TOKEN_ENV_VAR: &str = "GITHUB_TOKEN";

const GIST_API_BASE_URL: &str = "https://api.github.com/gists";
const APP_USER_AGENT: &str = "wundercli-sync";
const META_FILE: &str = "sync-meta.json";
const CONFIG_FILE: &str = "sync-config.json";
const GIST_DESCRIPTION: &str = "wundercli sync";

#[derive(Debug)]
pub struct SyncConfig {
    gist_id: String,
    token: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct StoredConfig {
    gist_id: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
struct SyncState {
    version: u32,
    updated_at: u64,
    active: Vec<Todo>,
    completed: Vec<Todo>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct SyncMeta {
    last_remote_updated_at: Option<u64>,
    last_snapshot: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GistResponse {
    files: HashMap<String, GistFileResponse>,
}

#[derive(Debug, Deserialize)]
struct GistFileResponse {
    content: Option<String>,
}

#[derive(Debug, Serialize)]
struct GistUpdateRequest {
    files: HashMap<String, GistFileUpdate>,
}

#[derive(Debug, Serialize)]
struct GistFileUpdate {
    content: String,
}

pub enum SyncOutcome {
    Pulled,
    Pushed,
    Unchanged,
}

pub fn run_sync_command(args: &[String], verbose: bool) -> Result<(), String> {
    let sync = sync_config()?;

    match args.first().map(String::as_str) {
        None | Some("once") => {
            sync_once(&sync, verbose)?;
            Ok(())
        }
        Some(other) => Err(format!("Unknown sync subcommand: {other}")),
    }
}

pub fn run_setup_command() -> Result<(), String> {
    ensure_gh_available()?;
    let token = resolve_token()?;
    let local = read_local_sync_state()?;
    let seeded_state = SyncState {
        version: 1,
        updated_at: current_unix_timestamp()?,
        active: local.active.clone(),
        completed: local.completed.clone(),
    };
    let gist_id = match resolve_gist_id()? {
        Some(existing) => {
            update_gist_state(
                &SyncConfig {
                    gist_id: existing.clone(),
                    token: token.clone(),
                },
                &seeded_state,
            )?;
            existing
        }
        None => create_gist_from_state(&seeded_state)?,
    };

    write_stored_config(&StoredConfig {
        gist_id: gist_id.clone(),
    })?;
    write_sync_meta(&SyncMeta {
        last_remote_updated_at: Some(seeded_state.updated_at),
        last_snapshot: Some(snapshot_string(&seeded_state)?),
    })?;

    println!("Sync is configured.");
    println!("Gist ID: {}", gist_id);
    println!("Local todos were uploaded to the gist.");
    println!("Run `todo sync` whenever you want to push or pull changes.");
    if env::var(GIST_ID_ENV_VAR).is_err() {
        println!(
            "Optional override: export {}={} to pin a different gist in this shell.",
            GIST_ID_ENV_VAR, gist_id
        );
    }

    Ok(())
}

pub fn print_sync_help() {
    println!("todo sync");
    println!("todo sync once");
    println!("todo --verbose sync");
    println!("Sync source: {} and {}", ACTIVE_FILE, COMPLETED_FILE);
    println!(
        "Gist config: ${} and ${} (or ${})",
        GIST_ID_ENV_VAR, GIST_TOKEN_ENV_VAR, GITHUB_TOKEN_ENV_VAR
    );
}

pub fn sync_configured() -> bool {
    sync_config().is_ok()
}

fn sync_once(sync: &SyncConfig, verbose: bool) -> Result<SyncOutcome, String> {
    let remote = fetch_gist_state(sync)?;
    let local = read_local_sync_state()?;
    let meta = read_sync_meta()?;
    let local_snapshot = snapshot_string(&local)?;
    let remote_snapshot = snapshot_string(&remote)?;

    if local_snapshot == remote_snapshot {
        write_sync_meta(&SyncMeta {
            last_remote_updated_at: Some(remote.updated_at),
            last_snapshot: Some(remote_snapshot),
        })?;
        if verbose {
            println!("Already in sync.");
        }
        return Ok(SyncOutcome::Unchanged);
    }

    if meta.last_snapshot.as_deref() == Some(local_snapshot.as_str())
        && meta.last_remote_updated_at != Some(remote.updated_at)
    {
        write_all_todos(&remote.active, &remote.completed)?;
        write_sync_meta(&SyncMeta {
            last_remote_updated_at: Some(remote.updated_at),
            last_snapshot: Some(remote_snapshot),
        })?;
        if verbose {
            println!("Pulled remote changes into local files.");
        }
        return Ok(SyncOutcome::Pulled);
    }

    if meta.last_snapshot.as_deref() == Some(remote_snapshot.as_str()) {
        let pushed = push_local_state(sync, &local)?;
        if verbose {
            println!("Pushed local changes to the gist.");
        }
        return Ok(pushed);
    }

    if remote.updated_at > latest_local_update()? {
        write_all_todos(&remote.active, &remote.completed)?;
        write_sync_meta(&SyncMeta {
            last_remote_updated_at: Some(remote.updated_at),
            last_snapshot: Some(remote_snapshot),
        })?;
        if verbose {
            println!("Pulled remote changes after conflict check.");
        }
        return Ok(SyncOutcome::Pulled);
    }

    let pushed = push_local_state(sync, &local)?;
    if verbose {
        println!("Pushed local changes after conflict check.");
    }
    Ok(pushed)
}

fn push_local_state(sync: &SyncConfig, local: &SyncState) -> Result<SyncOutcome, String> {
    let state = SyncState {
        version: 1,
        updated_at: current_unix_timestamp()?,
        active: local.active.clone(),
        completed: local.completed.clone(),
    };
    let snapshot = snapshot_string(&state)?;

    update_gist_state(sync, &state)?;
    write_sync_meta(&SyncMeta {
        last_remote_updated_at: Some(state.updated_at),
        last_snapshot: Some(snapshot),
    })?;

    Ok(SyncOutcome::Pushed)
}

fn sync_config() -> Result<SyncConfig, String> {
    let gist_id = resolve_gist_id()?.ok_or_else(|| {
        format!(
            "No sync gist configured. Run `todo setup` or set {}.",
            GIST_ID_ENV_VAR
        )
    })?;
    let token = resolve_token()?;

    Ok(SyncConfig { gist_id, token })
}

fn read_local_sync_state() -> Result<SyncState, String> {
    let (active, completed) = read_all_todos()?;

    Ok(SyncState {
        version: 1,
        updated_at: latest_local_update()?,
        active,
        completed,
    })
}

fn read_sync_meta() -> Result<SyncMeta, String> {
    let path = data_file(META_FILE)?;
    match fs::read_to_string(path) {
        Ok(contents) => serde_json::from_str(&contents).map_err(|err| err.to_string()),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(SyncMeta::default()),
        Err(err) => Err(err.to_string()),
    }
}

fn write_sync_meta(meta: &SyncMeta) -> Result<(), String> {
    let path = data_file(META_FILE)?;
    let json = serde_json::to_string_pretty(meta).map_err(|err| err.to_string())?;
    fs::write(path, json).map_err(|err| err.to_string())
}

fn snapshot_string(state: &SyncState) -> Result<String, String> {
    serde_json::to_string(&(&state.active, &state.completed)).map_err(|err| err.to_string())
}

fn fetch_gist_state(sync: &SyncConfig) -> Result<SyncState, String> {
    let url = format!("{}/{}", GIST_API_BASE_URL, sync.gist_id);
    let mut response = api_get(sync, &url).call().map_err(http_error_to_string)?;
    let gist: GistResponse = response
        .body_mut()
        .read_json()
        .map_err(http_error_to_string)?;
    let file = gist
        .files
        .get(GIST_FILE_NAME)
        .ok_or_else(|| format!("Gist {} does not contain {}", sync.gist_id, GIST_FILE_NAME))?;
    let content = file.content.as_deref().ok_or_else(|| {
        format!(
            "Gist {} returned no content for {}",
            sync.gist_id, GIST_FILE_NAME
        )
    })?;

    serde_json::from_str(content).map_err(|err| {
        format!(
            "Invalid JSON in gist {} file {}: {}",
            sync.gist_id, GIST_FILE_NAME, err
        )
    })
}

fn update_gist_state(sync: &SyncConfig, state: &SyncState) -> Result<(), String> {
    let url = format!("{}/{}", GIST_API_BASE_URL, sync.gist_id);
    let mut files = HashMap::new();
    files.insert(
        GIST_FILE_NAME.to_string(),
        GistFileUpdate {
            content: serde_json::to_string_pretty(state).map_err(|err| err.to_string())?,
        },
    );
    let payload = GistUpdateRequest { files };

    api_patch(sync, &url)
        .send_json(&payload)
        .map_err(http_error_to_string)?;

    Ok(())
}

fn api_get(sync: &SyncConfig, url: &str) -> ureq::RequestBuilder<ureq::typestate::WithoutBody> {
    ureq::Agent::new_with_defaults()
        .get(url)
        .header("accept", "application/vnd.github+json")
        .header("authorization", &format!("Bearer {}", sync.token))
        .header("user-agent", APP_USER_AGENT)
        .header("x-github-api-version", "2022-11-28")
}

fn api_patch(sync: &SyncConfig, url: &str) -> ureq::RequestBuilder<ureq::typestate::WithBody> {
    ureq::Agent::new_with_defaults()
        .patch(url)
        .header("accept", "application/vnd.github+json")
        .header("authorization", &format!("Bearer {}", sync.token))
        .header("user-agent", APP_USER_AGENT)
        .header("x-github-api-version", "2022-11-28")
}

fn current_unix_timestamp() -> Result<u64, String> {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .map_err(|err| err.to_string())
}

fn http_error_to_string(err: ureq::Error) -> String {
    err.to_string()
}

fn resolve_gist_id() -> Result<Option<String>, String> {
    if let Ok(value) = env::var(GIST_ID_ENV_VAR) {
        let gist_id = value.trim().to_string();
        if gist_id.is_empty() {
            return Err(format!("{} is set but empty", GIST_ID_ENV_VAR));
        }
        return Ok(Some(gist_id));
    }

    let path = data_file(CONFIG_FILE)?;
    match fs::read_to_string(path) {
        Ok(contents) => {
            let config: StoredConfig =
                serde_json::from_str(&contents).map_err(|err| err.to_string())?;
            if config.gist_id.trim().is_empty() {
                return Err("Stored gist id is empty".to_string());
            }
            Ok(Some(config.gist_id))
        }
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(err.to_string()),
    }
}

fn write_stored_config(config: &StoredConfig) -> Result<(), String> {
    let path = data_file(CONFIG_FILE)?;
    let json = serde_json::to_string_pretty(config).map_err(|err| err.to_string())?;
    fs::write(path, json).map_err(|err| err.to_string())
}

fn resolve_token() -> Result<String, String> {
    if let Ok(value) = env::var(GIST_TOKEN_ENV_VAR).or_else(|_| env::var(GITHUB_TOKEN_ENV_VAR)) {
        let token = value.trim().to_string();
        if token.is_empty() {
            return Err(format!("{} is empty", GIST_TOKEN_ENV_VAR));
        }
        return Ok(token);
    }

    ensure_gh_available()?;
    let output = Command::new("gh")
        .args(["auth", "token"])
        .output()
        .map_err(|err| format!("Failed to run gh auth token: {err}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "gh auth token failed. Run `gh auth login` first. {}",
            stderr.trim()
        ));
    }

    let token = String::from_utf8(output.stdout)
        .map_err(|err| err.to_string())?
        .trim()
        .to_string();
    if token.is_empty() {
        return Err("gh auth token returned an empty token".to_string());
    }

    Ok(token)
}

fn ensure_gh_available() -> Result<(), String> {
    let output = Command::new("gh")
        .arg("--version")
        .output()
        .map_err(|err| format!("GitHub CLI is required for setup: {err}"))?;
    if output.status.success() {
        Ok(())
    } else {
        Err("GitHub CLI is required for setup".to_string())
    }
}

fn create_gist_from_state(state: &SyncState) -> Result<String, String> {
    let json = serde_json::to_string_pretty(state).map_err(|err| err.to_string())?;
    let output = Command::new("gh")
        .args([
            "gist",
            "create",
            "-",
            "--filename",
            GIST_FILE_NAME,
            "--desc",
            GIST_DESCRIPTION,
        ])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .and_then(|mut child| {
            use std::io::Write;

            child
                .stdin
                .as_mut()
                .ok_or_else(|| io::Error::other("missing stdin"))?
                .write_all(json.as_bytes())?;
            child.wait_with_output()
        })
        .map_err(|err| format!("Failed to run gh gist create: {err}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("gh gist create failed: {}", stderr.trim()));
    }

    let url = String::from_utf8(output.stdout)
        .map_err(|err| err.to_string())?
        .trim()
        .to_string();
    let gist_id = url
        .rsplit('/')
        .next()
        .filter(|value| !value.is_empty())
        .ok_or_else(|| format!("Could not parse gist id from gh output: {url}"))?;

    Ok(gist_id.to_string())
}

#[cfg(test)]
mod tests {
    use super::{SyncMeta, SyncState, Todo, snapshot_string};

    #[test]
    fn snapshot_ignores_updated_at() {
        let left = SyncState {
            version: 1,
            updated_at: 10,
            active: vec![Todo {
                id: 1,
                text: "buy milk".to_string(),
            }],
            completed: Vec::new(),
        };
        let right = SyncState {
            version: 1,
            updated_at: 999,
            active: vec![Todo {
                id: 1,
                text: "buy milk".to_string(),
            }],
            completed: Vec::new(),
        };

        assert_eq!(
            snapshot_string(&left).unwrap(),
            snapshot_string(&right).unwrap()
        );
    }

    #[test]
    fn sync_meta_defaults_empty() {
        let meta = SyncMeta::default();
        assert!(meta.last_remote_updated_at.is_none());
        assert!(meta.last_snapshot.is_none());
    }
}
