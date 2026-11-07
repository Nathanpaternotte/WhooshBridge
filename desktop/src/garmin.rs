use anyhow::{bail, Context, Result};
use serde_json::{json, Value};
use std::{
    io::{BufRead, BufReader, Write},
    path::Path,
    process::{Child, ChildStdin, Command, Stdio},
    sync::mpsc::{self, Receiver},
    time::Duration,
};

pub enum UploadOutcome {
    Imported,
    Duplicate,
    Received,
}

pub struct Garmin {
    pub persistence_failed: bool,
    child: Child,
    input: ChildStdin,
    output: Receiver<String>,
}
impl Garmin {
    pub fn start() -> Result<Self> {
        let executable = std::env::current_exe()?;
        let bundled = executable
            .parent()
            .context("Dossier application absent")?
            .join("../Resources/garmin-bridge/garmin-bridge");
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
        let mut command = if bundled.is_file() {
            Command::new(bundled)
        } else {
            let python = std::env::var_os("WHOOSH_PYTHON")
                .map(std::path::PathBuf::from)
                .unwrap_or_else(|| root.join("venv/bin/python"));
            let mut c = Command::new(python);
            c.arg("-u")
                .arg(root.join("desktop/bridge/garmin_bridge.py"));
            c
        };
        let mut child = command.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn().context("Composant Garmin introuvable. Installez les dépendances Python ou utilisez l’application empaquetée.")?;
        let input = child.stdin.take().unwrap();
        let reader = BufReader::new(child.stdout.take().unwrap());
        let (sender, output) = mpsc::channel();
        std::thread::spawn(move || {
            for line in reader.lines() {
                match line {
                    Ok(line) => {
                        if sender.send(line).is_err() {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
        });
        Ok(Self {
            persistence_failed: false,
            child,
            input,
            output,
        })
    }
    fn send(&mut self, value: Value) -> Result<()> {
        serde_json::to_writer(&mut self.input, &value)?;
        self.input.write_all(b"\n")?;
        self.input.flush()?;
        Ok(())
    }
    fn receive(&mut self) -> Result<Value> {
        let line = match self.output.recv_timeout(Duration::from_secs(120)) {
            Ok(line) => line,
            Err(_) => {
                let _ = self.child.kill();
                bail!("Garmin ne répond plus. Vérifiez Garmin Connect avant de réessayer, puis réessayez la session enregistrée.");
            }
        };
        let response: Value = serde_json::from_str(&line).context("Réponse Garmin invalide")?;
        // Record only protocol categories and validated HTTP status codes.
        if let Some(code) = response["code"].as_str() {
            let safe_http = ["upload_http_", "login_http_"].iter().any(|prefix| {
                code.strip_prefix(prefix)
                    .and_then(|s| s.parse::<u16>().ok())
                    .is_some_and(|status| (100..=599).contains(&status))
            });
            if safe_http
                || matches!(
                    code,
                    "upload_error"
                        | "login_error"
                        | "session_missing"
                        | "upload_received"
                        | "upload_unconfirmed_result"
                        | "upload_unconfirmed_body"
                )
            {
                crate::diagnostics::record("garmin", code);
            }
        }
        if let Some(kind @ ("uploaded" | "duplicate" | "received" | "ready" | "error")) =
            response["type"].as_str()
        {
            crate::diagnostics::record("garmin.response", kind);
        }
        if response["session_warning"] == true {
            self.persistence_failed = true;
        }
        if let Some(tokens) = response["tokens"].as_str() {
            if crate::session_store::save(tokens).is_err() {
                // Do not turn a confirmed upload into an apparent failure.
                crate::diagnostics::record("garmin.session", "keychain_save_failed");
                self.persistence_failed = true;
            }
        }
        if response["type"] == "auth_required" {
            return Err(crate::worker::AuthRequired("garmin").into());
        }
        if response["type"] == "error" {
            bail!(
                "{}",
                response["message"].as_str().unwrap_or("Erreur Garmin")
            );
        }
        Ok(response)
    }
    pub fn resume(&mut self) -> Result<bool> {
        let Some(tokens) = crate::session_store::load()? else {
            return Ok(false);
        };
        self.send(json!({"op":"resume", "tokens":tokens}))?;
        let response = self.receive()?;
        anyhow::ensure!(
            response["type"] == "ready",
            "Restauration Garmin non confirmée"
        );
        Ok(true)
    }
    pub fn login(
        &mut self,
        email: &str,
        password: &str,
        mut mfa: impl FnMut() -> Result<String>,
    ) -> Result<()> {
        self.send(json!({"op":"login","email":email,"password":password}))?;
        loop {
            let response = self.receive()?;
            match response["type"].as_str() {
                Some("mfa") => self.send(json!({"op":"mfa","code":mfa()?}))?,
                Some("ready") => return Ok(()),
                _ => bail!("Réponse d’authentification Garmin inattendue"),
            }
        }
    }
    pub fn upload(&mut self, path: &Path) -> Result<UploadOutcome> {
        self.send(json!({"op":"upload","path":path}))?;
        match self.receive()?["type"].as_str() {
            Some("uploaded") => Ok(UploadOutcome::Imported),
            Some("duplicate") => Ok(UploadOutcome::Duplicate),
            Some("received") => Ok(UploadOutcome::Received),
            _ => bail!("Import Garmin non confirmé"),
        }
    }
}
impl Drop for Garmin {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
