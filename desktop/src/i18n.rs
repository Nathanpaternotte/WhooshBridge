//! Translate app-owned text at rendering time; ride names and credentials stay untouched.
use std::{
    collections::BTreeMap,
    sync::{
        atomic::{AtomicBool, Ordering},
        OnceLock,
    },
};
static ENGLISH: AtomicBool = AtomicBool::new(false);
static CATALOG: OnceLock<BTreeMap<String, String>> = OnceLock::new();
pub fn english() -> bool {
    ENGLISH.load(Ordering::Relaxed)
}
pub fn set_english(value: bool) {
    ENGLISH.store(value, Ordering::Relaxed);
}
pub fn initialize() {
    let languages = std::process::Command::new("/usr/bin/defaults")
        .args(["read", "-g", "AppleLanguages"])
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
        .unwrap_or_default();
    let first = languages
        .split(|c: char| !c.is_ascii_alphabetic() && c != '-')
        .find(|s| !s.is_empty())
        .unwrap_or("en");
    set_english(!first.starts_with("fr"));
    if cfg!(debug_assertions) {
        if let Ok(lang) = std::env::var("WHOOSH_UI_LANGUAGE") {
            set_english(lang == "en");
        }
    }
}
pub fn t(text: impl AsRef<str>) -> String {
    if english() {
        translate(text.as_ref())
    } else {
        text.as_ref().to_owned()
    }
}
fn catalog() -> &'static BTreeMap<String, String> {
    CATALOG.get_or_init(|| {
        serde_json::from_str(include_str!("../locales/en.json"))
            .expect("checked translation catalog")
    })
}
fn parts(template: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut rest = template;
    while let Some(start) = rest.find('{') {
        let Some(end) = rest[start..].find('}') else {
            break;
        };
        parts.push(&rest[..start]);
        rest = &rest[start + end + 1..];
    }
    parts.push(rest);
    parts
}
fn captures<'a>(template: &str, text: &'a str) -> Option<Vec<&'a str>> {
    let pieces = parts(template);
    if pieces.len() < 2 {
        return None;
    }
    let mut rest = text.strip_prefix(pieces[0])?;
    let mut values = Vec::new();
    for (i, literal) in pieces.iter().enumerate().skip(1) {
        let last = i == pieces.len() - 1;
        let end = if last {
            rest.strip_suffix(literal)?.len()
        } else {
            rest.find(literal)?
        };
        if !rest[end..].starts_with(literal) {
            return None;
        }
        values.push(&rest[..end]);
        rest = &rest[end + literal.len()..];
    }
    rest.is_empty().then_some(values)
}
fn translate(text: &str) -> String {
    if let Some(value) = catalog().get(text) {
        return value.clone();
    }
    if let Some(error) = text.strip_prefix("Échec : ") {
        return format!("Failed: {}", translate(error));
    }
    for (source, target) in catalog() {
        if let Some(values) = captures(source, text) {
            let pieces = parts(target);
            if pieces.len() != values.len() + 1 {
                continue;
            }
            let mut result = pieces[0].to_string();
            for (value, suffix) in values.iter().zip(pieces.iter().skip(1)) {
                result.push_str(value);
                result.push_str(suffix);
            }
            return result;
        }
    }
    text.into()
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn templates_preserve_user_text_and_translate_errors() {
        assert_eq!(
            translate("Téléchargement : Été, 2 / 3 activités"),
            "Downloading: Été, 2 / 3 activités"
        );
        assert_eq!(translate("2 / 3 sorties traitées"), "2 / 3 rides processed");
        assert_eq!(translate("Version FIT : 31.33"), "FIT version: 31.33");
        assert_eq!(
            translate("Échec : Garmin a rejeté le fichier FIT."),
            "Failed: Garmin rejected the FIT file."
        );
        for text in ["●  Garmin", "↻", "Été", "🛜", "éééééééééé"] {
            assert_eq!(translate(text), text);
        }
        for (source, target) in catalog() {
            assert_eq!(parts(source).len(), parts(target).len(), "{source}");
        }
    }
}
