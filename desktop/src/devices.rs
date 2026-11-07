// IDs verified against Garmin's official FIT profile, 2026-11-07:
// https://github.com/garmin/fit-python-sdk/blob/main/garmin_fit_sdk/profile.py
pub const DEVICES: &[(&str, u16, &str)] = &[
    ("Edge 840", 4062, "Compteur vélo"),
    ("Edge 540", 4061, "Compteur vélo"),
    ("Edge 1040", 3843, "Compteur vélo"),
    ("Edge 1050", 4440, "Compteur vélo"),
    ("Edge 1060", 4962, "Compteur vélo"),
    ("Edge 550", 4633, "Compteur vélo"),
    ("Edge 850", 4634, "Compteur vélo"),
    ("Edge 530", 3121, "Compteur vélo"),
    ("Edge 830", 3122, "Compteur vélo"),
    ("Edge 1030", 2713, "Compteur vélo"),
    ("Edge 1030 Plus", 3570, "Compteur vélo"),
    ("Edge 130 Plus", 3558, "Compteur vélo"),
    ("Edge Explore 2", 4169, "Compteur vélo"),
    ("Forerunner 255", 3992, "Montre multisport"),
    ("Forerunner 265", 4257, "Montre multisport"),
    ("Forerunner 955", 4024, "Montre multisport"),
    ("Forerunner 965", 4315, "Montre multisport"),
    ("Forerunner 970", 4565, "Montre multisport"),
    ("fēnix 7", 3906, "Montre multisport"),
    ("fēnix 7 Pro Solar", 4375, "Montre multisport"),
    ("fēnix 8", 4536, "Montre multisport"),
    ("epix Gen 2", 3943, "Montre multisport"),
    ("Enduro 2", 4341, "Montre multisport"),
];

// Published compatibility firmware per Edge; not necessarily latest or installed.
pub const EDGE_FIRMWARE: &[(u16, u16)] = &[
    (4062, 3133),
    (4061, 3133),
    (3843, 3133),
    (4440, 3133),
    (4962, 819),
    (4633, 3220),
    (4634, 3220),
    (3121, 975),
    (3122, 975),
    (2713, 1375),
    (3570, 675),
    (3558, 300),
    (4169, 3023),
];
pub fn firmware(product: u16) -> Option<u16> {
    EDGE_FIRMWARE.iter().find(|d| d.0 == product).map(|d| d.1)
}
#[cfg(test)]
mod tests {
    #[test]
    fn catalogue_matches_audited_official_profile() {
        let audit: serde_json::Value =
            serde_json::from_str(include_str!("../tests/device_catalog_audit.json")).unwrap();
        let rows = audit["devices"].as_array().unwrap();
        assert_eq!(rows.len(), super::DEVICES.len());
        let firmware = audit["edge_firmware"].as_array().unwrap();
        assert_eq!(firmware.len(), super::EDGE_FIRMWARE.len());
        for &(product, version) in super::EDGE_FIRMWARE {
            assert!(version > 0 && version < u16::MAX);
            assert!(firmware
                .iter()
                .any(|r| r["product"] == product && r["software_raw"] == version));
            assert!(super::DEVICES
                .iter()
                .any(|d| d.1 == product && d.0.starts_with("Edge")));
        }
        for ((name, product, _), row) in super::DEVICES.iter().zip(rows) {
            assert_eq!(row["name"].as_str().unwrap(), *name);
            assert_eq!(row["product"].as_u64().unwrap(), u64::from(*product));
        }
    }
}
