use crate::diagnostics;
use anyhow::{Context, Result};
use reqwest::blocking::Client;
use serde_json::{json, Value};
use std::sync::atomic::AtomicBool;
use std::{io::Read, time::Duration};

pub struct MyWhoosh {
    client: Client,
    web: crate::web_session::WebSession,
}
#[derive(Clone)]
pub struct Activity {
    pub id: String,
    pub name: String,
    pub date: String,
    pub file_id: Value,
}
impl Activity {
    fn parse(value: &Value) -> Option<Self> {
        let file_id = value.get("activityFileId")?.clone();
        if file_id.is_null() {
            return None;
        }
        let id = value
            .get("id")
            .or_else(|| value.get("_id"))
            .unwrap_or(&file_id);
        let raw_date = ["date", "timestamp", "startTime", "createdAt"]
            .iter()
            .find_map(|k| value.get(k));
        let date = raw_date
            .map(|v| {
                if let Some(n) = v
                    .as_i64()
                    .or_else(|| v.as_str().and_then(|s| s.parse().ok()))
                {
                    let seconds = if n > 10_000_000_000 { n / 1000 } else { n };
                    if let Some(d) = chrono::DateTime::from_timestamp(seconds, 0) {
                        return d
                            .with_timezone(&chrono::Local)
                            .format("%d/%m/%Y · %H:%M")
                            .to_string();
                    }
                }
                v.as_str().unwrap_or("Date inconnue").to_owned()
            })
            .unwrap_or_else(|| "Date inconnue".into());
        Some(Self {
            id: id
                .as_str()
                .map(str::to_owned)
                .unwrap_or_else(|| id.to_string()),
            name: value
                .get("name")
                .or_else(|| value.get("title"))
                .and_then(Value::as_str)
                .unwrap_or("Sortie MyWhoosh")
                .into(),
            date,
            file_id,
        })
    }
}
impl MyWhoosh {
    pub fn login(cancel: &AtomicBool, email: &str, password: &str) -> Result<Self> {
        Self::connect(cancel, "--login", Some((email, password)))
    }
    pub fn restore(cancel: &AtomicBool) -> Result<Self> {
        Self::connect(cancel, "--restore", None)
    }
    fn connect(cancel: &AtomicBool, mode: &str, credentials: Option<(&str, &str)>) -> Result<Self> {
        diagnostics::record("mywhoosh.web.login", "started");
        let mut web = crate::web_session::WebSession::start(mode)?;
        if let Some((email, password)) = credentials {
            web.send(json!({"email":email,"password":password}))?;
        }
        let response = web.receive(
            Duration::from_secs(if mode == "--restore" { 90 } else { 600 }),
            Some(cancel),
        )?;
        anyhow::ensure!(
            response["type"] == "ready",
            "Session MyWhoosh non confirmée"
        );
        diagnostics::record("mywhoosh.web.login", "authenticated");
        let client = Client::builder()
            .timeout(Duration::from_secs(60))
            .connect_timeout(Duration::from_secs(15))
            .build()?;
        Ok(Self { client, web })
    }
    pub fn activities(&mut self, page: usize) -> Result<Vec<Activity>> {
        diagnostics::record("mywhoosh.web.activities", "started");
        let value = self.web.request(json!({"op":"activities","page":page}))?;
        anyhow::ensure!(
            value["type"] == "activities",
            "Réponse de sorties inattendue"
        );
        let entries = value["data"]["results"]
            .as_array()
            .context("Format de sorties MyWhoosh inattendu")?;
        diagnostics::record("mywhoosh.web.activities", "loaded");
        Ok(entries.iter().filter_map(Activity::parse).collect())
    }
    pub fn download(&mut self, activity: &Activity) -> Result<Vec<u8>> {
        let value = self
            .web
            .request(json!({"op":"download","fileId":activity.file_id}))?;
        anyhow::ensure!(
            value["type"] == "download",
            "Réponse de téléchargement inattendue"
        );
        let url = value["url"]
            .as_str()
            .context("Lien de téléchargement absent")?;
        anyhow::ensure!(
            url.starts_with("https://"),
            "Lien de téléchargement non sécurisé"
        );
        // Do not attach the MyWhoosh bearer token to the signed download URL.
        let response = self
            .client
            .get(url)
            .send()
            .context("Téléchargement impossible")?
            .error_for_status()
            .context("Téléchargement refusé")?;
        let mut bytes = Vec::new();
        response
            .take(64 * 1024 * 1024 + 1)
            .read_to_end(&mut bytes)
            .context("Lecture du FIT impossible")?;
        anyhow::ensure!(
            bytes.len() <= 64 * 1024 * 1024,
            "Fichier FIT trop volumineux"
        );
        Ok(bytes)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn understands_repo_formats() {
        let a = Activity::parse(&json!({"_id":"ride", "title":"Test", "timestamp":1768039347000i64,"activityFileId":"f"})).unwrap();
        assert_eq!(a.id, "ride");
        assert_eq!(a.name, "Test");
        assert!(a.date.contains("2026"));
        assert!(Activity::parse(&json!({"id":"missing-file"})).is_none());
    }
}
