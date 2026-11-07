use anyhow::{bail, Context, Result};
use serde_json::Value;
use std::{
    io::{BufRead, BufReader, Write},
    process::{Child, ChildStdin, Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, RecvTimeoutError},
    },
    time::{Duration, Instant},
};

pub struct WebSession {
    child: Child,
    input: ChildStdin,
    output: Receiver<String>,
}
impl WebSession {
    pub fn start(mode: &str) -> Result<Self> {
        let executable = std::env::current_exe()?.with_file_name("whoosh-web-login");
        let mut child = Command::new(executable)
            .arg(mode)
            .env(
                "WHOOSH_LANGUAGE",
                if crate::i18n::english() { "en" } else { "fr" },
            )
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .context(
                "Fenêtre MyWhoosh introuvable. Reconstruisez les deux binaires de l’application.",
            )?;
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
            child,
            input,
            output,
        })
    }
    pub fn receive(&mut self, timeout: Duration, cancel: Option<&AtomicBool>) -> Result<Value> {
        let start = Instant::now();
        loop {
            if cancel.is_some_and(|c| c.load(Ordering::Relaxed)) {
                let _ = self.child.kill();
                bail!("Connexion MyWhoosh annulée");
            }
            if start.elapsed() >= timeout {
                let _ = self.child.kill();
                bail!("MyWhoosh n’a pas répondu à temps. Réessayez la connexion enregistrée.");
            }
            match self.output.recv_timeout(Duration::from_millis(200)) {
                Ok(line) => {
                    let value: Value =
                        serde_json::from_str(&line).context("Réponse web MyWhoosh invalide")?;
                    if value["type"] == "auth_required" {
                        return Err(crate::worker::AuthRequired("mywhoosh").into());
                    }
                    if value["type"] == "error" {
                        bail!("Le site MyWhoosh n’a pas pu charger les données. Réessayez ; votre session est conservée.");
                    }
                    return Ok(value);
                }
                Err(RecvTimeoutError::Timeout) => (),
                Err(RecvTimeoutError::Disconnected) => {
                    bail!("La fenêtre de connexion MyWhoosh a été fermée ou n’a pas pu démarrer.")
                }
            }
        }
    }
    pub fn clear() -> Result<()> {
        let mut session = Self::start("--clear")?;
        let value = session.receive(Duration::from_secs(30), None)?;
        anyhow::ensure!(
            value["type"] == "cleared",
            "Effacement MyWhoosh non confirmé"
        );
        Ok(())
    }
    pub fn send(&mut self, value: Value) -> Result<()> {
        serde_json::to_writer(&mut self.input, &value)?;
        self.input.write_all(b"\n")?;
        self.input.flush()?;
        Ok(())
    }
    pub fn request(&mut self, value: Value) -> Result<Value> {
        self.send(value)?;
        self.receive(Duration::from_secs(90), None)
    }
}
impl Drop for WebSession {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
