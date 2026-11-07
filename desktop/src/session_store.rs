use anyhow::{Context, Result};
use security_framework::passwords::{
    delete_generic_password, get_generic_password, set_generic_password,
};
const SERVICE: &str = "com.mywhooshbridge.sessions";
const ACCOUNT: &str = "garmin";
pub fn load() -> Result<Option<String>> {
    match get_generic_password(SERVICE, ACCOUNT) {
        Ok(bytes) => Ok(Some(
            String::from_utf8(bytes).context("Session Garmin illisible dans le trousseau")?,
        )),
        Err(e) if e.code() == -25300 => Ok(None),
        Err(e) => {
            Err(e).context("Accès au trousseau macOS impossible ; déverrouillez-le puis réessayez")
        }
    }
}
pub fn save(tokens: &str) -> Result<()> {
    set_generic_password(SERVICE, ACCOUNT, tokens.as_bytes())
        .context("Enregistrement de la session Garmin dans le trousseau impossible")
}
pub fn clear() -> Result<()> {
    match delete_generic_password(SERVICE, ACCOUNT) {
        Ok(()) => Ok(()),
        Err(e) if e.code() == -25300 => Ok(()),
        Err(e) => Err(e).context("Suppression de la session Garmin impossible"),
    }
}

const WHOOSH_ACCOUNT: &str = "mywhoosh.credentials";
pub fn load_whoosh() -> Result<Option<(String, String)>> {
    match get_generic_password(SERVICE, WHOOSH_ACCOUNT) {
        Ok(bytes) => Ok(Some(
            serde_json::from_slice(&bytes)
                .context("Identifiants MyWhoosh illisibles dans le trousseau")?,
        )),
        Err(e) if e.code() == -25300 => Ok(None),
        Err(e) => Err(e).context("Accès aux identifiants MyWhoosh du trousseau impossible"),
    }
}
pub fn save_whoosh(email: &str, password: &str) -> Result<()> {
    set_generic_password(
        SERVICE,
        WHOOSH_ACCOUNT,
        &serde_json::to_vec(&(email, password))?,
    )
    .context("Enregistrement MyWhoosh dans le trousseau impossible")
}
pub fn clear_whoosh() -> Result<()> {
    match delete_generic_password(SERVICE, WHOOSH_ACCOUNT) {
        Ok(()) => Ok(()),
        Err(e) if e.code() == -25300 => Ok(()),
        Err(e) => Err(e).context("Suppression des identifiants MyWhoosh impossible"),
    }
}

const DEVICES_ACCOUNT: &str = "garmin.device_preferences";
#[derive(Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct DevicePreferences {
    pub selected: u16,
    #[serde(default)]
    pub language: Option<bool>,
    pub units: std::collections::BTreeMap<u16, u32>,
}
impl Default for DevicePreferences {
    fn default() -> Self {
        Self {
            selected: 4440,
            language: None,
            units: Default::default(),
        }
    }
}
pub fn load_devices() -> Result<DevicePreferences> {
    match get_generic_password(SERVICE, DEVICES_ACCOUNT) {
        Ok(bytes) => {
            let mut value: DevicePreferences = serde_json::from_slice(&bytes)
                .context("Réglages Garmin illisibles dans le trousseau")?;
            value
                .units
                .retain(|product, unit| crate::devices::firmware(*product).is_some() && *unit != 0);
            if crate::devices::firmware(value.selected).is_none() {
                value.selected = 4440;
            }
            Ok(value)
        }
        Err(e) if e.code() == -25300 => Ok(DevicePreferences::default()),
        Err(e) => Err(e).context("Lecture des Unit ID dans le trousseau impossible"),
    }
}
pub fn save_devices(value: &DevicePreferences) -> Result<()> {
    set_generic_password(SERVICE, DEVICES_ACCOUNT, &serde_json::to_vec(value)?)
        .context("Enregistrement du Unit ID dans le trousseau impossible")
}
