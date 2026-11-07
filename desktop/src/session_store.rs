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
fn load_legacy_devices() -> Result<DevicePreferences> {
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
// Preferences contain no passwords or session tokens. Keep them outside Keychain
// so changing language or a recorder ID never requires credential authorization.
fn preferences_path() -> Result<std::path::PathBuf> {
    Ok(std::path::PathBuf::from(
        std::env::var_os("HOME").context("Dossier utilisateur introuvable")?,
    )
    .join("Library/Application Support/Whoosh Bridge/preferences.json"))
}

fn read_preferences(path: &std::path::Path) -> Result<Option<DevicePreferences>> {
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => anyhow::bail!("Lecture des réglages locaux impossible"),
    };
    let mut value: DevicePreferences = serde_json::from_slice(&bytes)
        .context("Réglages locaux illisibles ; le fichier existant a été conservé")?;
    value
        .units
        .retain(|product, unit| crate::devices::firmware(*product).is_some() && *unit != 0);
    if crate::devices::firmware(value.selected).is_none() {
        value.selected = DevicePreferences::default().selected;
    }
    Ok(Some(value))
}

fn write_preferences(path: &std::path::Path, value: &DevicePreferences) -> Result<()> {
    use std::io::Write;
    let parent = path.parent().context("Dossier des réglages absent")?;
    std::fs::create_dir_all(parent).context("Création du dossier des réglages impossible")?;
    // NamedTempFile is private (0600 on macOS); persist replaces atomically.
    let mut file = tempfile::NamedTempFile::new_in(parent)
        .context("Enregistrement des réglages locaux impossible")?;
    file.write_all(&serde_json::to_vec(value)?)?;
    file.as_file().sync_all()?;
    file.persist(path)
        .map_err(|_| anyhow::anyhow!("Enregistrement des réglages locaux impossible"))?;
    Ok(())
}

pub fn load_devices() -> Result<DevicePreferences> {
    let path = preferences_path()?;
    if let Some(value) = read_preferences(&path)? {
        return Ok(value);
    }
    // Migrate once. If Keychain access is denied, do not save defaults over it.
    // Retain the legacy item for compatibility with older app versions.
    let value = load_legacy_devices()?;
    write_preferences(&path, &value)?;
    Ok(value)
}

pub fn save_devices(value: &DevicePreferences) -> Result<()> {
    write_preferences(&preferences_path()?, value)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn local_preferences_roundtrip_and_validate_without_keychain() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("preferences.json");
        assert!(read_preferences(&path).unwrap().is_none());
        let mut value = DevicePreferences {
            selected: 4962,
            language: Some(true),
            ..Default::default()
        };
        value.units.insert(4962, 123456789);
        write_preferences(&path, &value).unwrap();
        assert!(read_preferences(&path).unwrap().unwrap() == value);
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        value.selected = 65535;
        value.units.insert(65535, 42);
        value.units.insert(4440, 0);
        write_preferences(&path, &value).unwrap();
        let restored = read_preferences(&path).unwrap().unwrap();
        assert_eq!(restored.selected, 4440);
        assert_eq!(restored.units.len(), 1);
        std::fs::write(&path, b"invalid").unwrap();
        assert!(read_preferences(&path).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), b"invalid");
    }
}
