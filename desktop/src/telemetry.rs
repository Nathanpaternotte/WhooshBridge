//! Optional installation counts. Never pass account/activity data to this module.
use serde::{Deserialize, Serialize};
use std::{
    io::{Read, Write},
    path::Path,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};

// Public ingestion token, intentionally shipped in the app; never a personal API key.
const PROJECT_TOKEN: &str = "phc_Iqw5LUW5Gu5MRxWjgHVQ9f5wJIlzEyfebkrFxMm1PUt";
const ENDPOINT: &str = "https://eu.i.posthog.com/i/v0/e/";

#[derive(Serialize, Deserialize)]
struct State {
    installation_id: String,
    last_attempt_day: Option<String>,
}

fn load(path: &Path) -> anyhow::Result<State> {
    match std::fs::read(path) {
        Ok(bytes) => Ok(serde_json::from_slice(&bytes)?),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            let mut bytes = [0u8; 16];
            std::fs::File::open("/dev/urandom")?.read_exact(&mut bytes)?;
            Ok(State {
                installation_id: bytes.iter().map(|b| format!("{b:02x}")).collect(),
                last_attempt_day: None,
            })
        }
        Err(e) => Err(e.into()),
    }
}

fn save(path: &Path, state: &State) -> anyhow::Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("Missing directory"))?;
    std::fs::create_dir_all(parent)?;
    let mut file = tempfile::NamedTempFile::new_in(parent)?;
    file.write_all(&serde_json::to_vec(state)?)?;
    file.as_file().sync_all()?;
    file.persist(path)?;
    Ok(())
}

fn payload(id: &str) -> serde_json::Value {
    serde_json::json!({
        "api_key": PROJECT_TOKEN,
        "event": "app_active",
        "distinct_id": id,
        "properties": {
            "app_version": env!("CARGO_PKG_VERSION"),
            "$process_person_profile": false,
            "$geoip_disable": true,
            "$ip": "0.0.0.0"
        }
    })
}

// Persist before sending: an offline or ambiguous request is not retried that day.
fn reserve_day(path: &Path, day: &str) -> anyhow::Result<Option<String>> {
    let mut state = load(path)?;
    if state.last_attempt_day.as_deref() == Some(day) {
        return Ok(None);
    }
    state.last_attempt_day = Some(day.to_owned());
    save(path, &state)?;
    Ok(Some(state.installation_id))
}

pub struct Telemetry {
    enabled: Arc<AtomicBool>,
    wake: std::sync::mpsc::Sender<()>,
}
impl Telemetry {
    pub fn start() -> Self {
        let enabled = Arc::new(AtomicBool::new(false));
        let active = enabled.clone();
        let (wake, receiver) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let Some(home) = std::env::var_os("HOME") else {
                return;
            };
            let path = std::path::PathBuf::from(home)
                .join("Library/Application Support/Whoosh Bridge/telemetry.json");
            let Ok(client) = reqwest::blocking::Client::builder()
                .timeout(Duration::from_secs(5))
                .redirect(reqwest::redirect::Policy::none())
                .build()
            else {
                return;
            };
            loop {
                match receiver.recv_timeout(Duration::from_secs(60)) {
                    Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
                    _ => {
                        if !active.load(Ordering::SeqCst) {
                            continue;
                        }
                        let day = chrono::Utc::now().format("%Y-%m-%d").to_string();
                        if let Ok(Some(id)) = reserve_day(&path, &day) {
                            if active.load(Ordering::SeqCst) {
                                // Best effort only. No logs, response bodies, retries or UI errors.
                                let _ = client.post(ENDPOINT).json(&payload(&id)).send();
                            }
                        }
                    }
                }
            }
        });
        Self { enabled, wake }
    }
    pub fn set_enabled(&self, enabled: bool) {
        self.enabled.store(enabled, Ordering::SeqCst);
        let _ = self.wake.send(());
    }
}
impl Drop for Telemetry {
    fn drop(&mut self) {
        self.enabled.store(false, Ordering::SeqCst);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn counts_once_per_utc_day_and_keeps_identity() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("telemetry.json");
        let id = reserve_day(&path, "2026-01-01").unwrap().unwrap();
        assert_eq!(id.len(), 32);
        assert!(reserve_day(&path, "2026-01-01").unwrap().is_none());
        assert_eq!(reserve_day(&path, "2026-01-02").unwrap().unwrap(), id);
        assert_eq!(
            std::fs::metadata(path).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
    #[test]
    fn unreadable_state_is_preserved() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("telemetry.json");
        std::fs::write(&path, "broken").unwrap();
        assert!(reserve_day(&path, "2026-01-01").is_err());
        assert_eq!(std::fs::read_to_string(path).unwrap(), "broken");
    }
    #[test]
    fn event_has_only_allowed_data() {
        let event = payload("synthetic-installation");
        assert_eq!(event["event"], "app_active");
        assert_eq!(event["properties"].as_object().unwrap().len(), 4);
        assert_eq!(event["properties"]["$process_person_profile"], false);
        assert_eq!(event["properties"]["$geoip_disable"], true);
        assert_eq!(event["properties"]["$ip"], "0.0.0.0");
    }
}
