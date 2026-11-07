//! Conservative, byte-preserving FIT metadata editor. Unknown/developer fields
//! remain byte-for-byte intact. Repair MyWhoosh summaries and recorder identity.
use anyhow::{bail, ensure, Context, Result};
#[path = "fit_identity.rs"]
mod identity;
#[path = "fit_summary.rs"]
mod summary;

#[derive(Clone)]
struct Definition {
    global: u16,
    big: bool,
    fields: Vec<(u8, usize)>,
    developer_size: usize,
    raw: Vec<u8>,
}

pub fn crc(bytes: &[u8]) -> u16 {
    let mut crc = 0u16;
    for byte in bytes {
        crc ^= *byte as u16;
        for _ in 0..8 {
            crc = if crc & 1 != 0 {
                (crc >> 1) ^ 0xa001
            } else {
                crc >> 1
            };
        }
    }
    crc
}

fn word(bytes: &[u8], big: bool) -> u16 {
    if big {
        u16::from_be_bytes([bytes[0], bytes[1]])
    } else {
        u16::from_le_bytes([bytes[0], bytes[1]])
    }
}

pub fn modify(input: &[u8], product: u16, unit_id: Option<u32>) -> Result<Vec<u8>> {
    identity::normalize(&modify_base(input, product, unit_id)?, product, unit_id)
}

fn modify_base(input: &[u8], product: u16, unit_id: Option<u32>) -> Result<Vec<u8>> {
    ensure!(
        unit_id != Some(0),
        "L’identifiant d’unité Garmin ne peut pas être zéro"
    );
    ensure!(
        input.len() >= 14 && &input[8..12] == b".FIT",
        "Fichier FIT invalide"
    );
    let header = input[0] as usize;
    ensure!(
        header == 12 || header == 14,
        "En-tête FIT non pris en charge"
    );
    let length = u32::from_le_bytes(input[4..8].try_into()?) as usize;
    let end = header.checked_add(length).context("Taille FIT invalide")?;
    ensure!(
        end.checked_add(2) == Some(input.len()),
        "FIT tronqué ou concaténé"
    );
    if header == 14 && input[12..14] != [0, 0] {
        ensure!(crc(&input[..14]) == 0, "CRC d’en-tête FIT invalide");
    }
    ensure!(crc(input) == 0, "CRC du fichier FIT invalide");
    let mut output = input.to_vec();
    let mut defs: [Option<Definition>; 16] = std::array::from_fn(|_| None);
    let mut pos = header;
    let mut changed_file_id = false;
    let mut mywhoosh = false;
    let mut samples = Vec::new();
    let mut sessions = Vec::new();
    let mut last_timestamp: Option<u32> = None;
    while pos < end {
        let record_start = pos;
        let record = input[pos];
        pos += 1;
        let compressed = record & 0x80 != 0;
        let local = if compressed {
            ((record >> 5) & 3) as usize
        } else {
            (record & 15) as usize
        };
        if !compressed && record & 0x40 != 0 {
            ensure!(pos + 5 <= end, "Définition FIT tronquée");
            ensure!(input[pos + 1] <= 1, "Architecture FIT invalide");
            let big = input[pos + 1] == 1;
            let global = word(&input[pos + 2..pos + 4], big);
            let count = input[pos + 4] as usize;
            pos += 5;
            ensure!(pos + 3 * count <= end, "Champs FIT tronqués");
            let mut fields = Vec::new();
            for _ in 0..count {
                fields.push((input[pos], input[pos + 1] as usize));
                pos += 3;
            }
            let mut developer_size = 0;
            if record & 0x20 != 0 {
                ensure!(pos < end, "Champs développeur absents");
                let count = input[pos] as usize;
                pos += 1;
                ensure!(pos + 3 * count <= end, "Champs développeur tronqués");
                for _ in 0..count {
                    developer_size += input[pos + 1] as usize;
                    pos += 3;
                }
            }
            defs[local] = Some(Definition {
                global,
                big,
                fields,
                developer_size,
                raw: input[record_start..pos].to_vec(),
            });
            continue;
        }
        let def = defs[local]
            .as_ref()
            .context("Message FIT sans définition")?;
        let start = pos;
        let mut fields = Vec::new();
        for &(number, size) in &def.fields {
            if compressed && number == 253 {
                continue;
            }
            ensure!(pos + size <= end, "Message FIT tronqué");
            fields.push((number, pos, size));
            pos += size;
        }
        pos += def.developer_size;
        ensure!(pos <= end && pos > start, "Taille du message FIT invalide");
        if let Some(timestamp) = summary::number(input, &fields, 253, def.big) {
            last_timestamp = Some(timestamp);
        } else if compressed {
            if let Some(previous) = last_timestamp {
                let offset = u32::from(record & 31);
                let mut timestamp = (previous & !31) | offset;
                if offset < (previous & 31) {
                    timestamp = timestamp
                        .checked_add(32)
                        .context("Timestamp FIT hors limites")?;
                }
                last_timestamp = Some(timestamp);
            }
        }
        if def.global == 20 {
            let timestamp = if compressed {
                last_timestamp
            } else {
                summary::number(input, &fields, 253, def.big)
            };
            samples.push(summary::Sample {
                timestamp,
                values: [
                    summary::number(input, &fields, 3, def.big).filter(|v| *v > 0),
                    summary::number(input, &fields, 4, def.big),
                    summary::number(input, &fields, 7, def.big),
                ],
            });
            if mywhoosh {
                for &(number, p, size) in &fields {
                    if number == 13 && size == 1 {
                        output[p] = 0x7f;
                    }
                }
            }
        }
        if def.global == 18 {
            sessions.push(summary::Session {
                start: record_start,
                end: pos,
                definition: def.clone(),
                fields: fields.clone(),
                compressed,
            });
        }
        if def.global == 0 {
            mywhoosh |= summary::number(input, &fields, 1, def.big) == Some(331);
        }
        let recorder = def.global == 23
            && fields
                .iter()
                .any(|&(n, p, s)| n == 0 && s == 1 && input[p] == 0);
        if def.global != 0 && !recorder {
            continue;
        }
        // file_id.type must designate an activity, not a workout/course/settings file.
        if def.global == 0 {
            ensure!(
                fields
                    .iter()
                    .any(|&(n, p, s)| n == 0 && s == 1 && input[p] == 4),
                "Le FIT ne contient pas une activité"
            );
        }
        let product_field = if def.global == 0 { 2 } else { 4 };
        let mut manufacturer_found = false;
        let mut product_found = false;
        for &(number, p, size) in &fields {
            // MyWhoosh's fixed synthetic serial must not identify a previously
            // imported Garmin model. Clear stale names; the canonical creator
            // message below contains the complete selected product name.
            if number == 3 {
                ensure!(size == 4, "Numéro d’unité FIT incompatible");
                let serial = unit_id.unwrap_or(0);
                output[p..p + 4].copy_from_slice(&if def.big {
                    serial.to_be_bytes()
                } else {
                    serial.to_le_bytes()
                });
                continue;
            }
            if (def.global == 0 && number == 8) || (recorder && matches!(number, 19 | 27)) {
                output[p..p + size].fill(0);
                continue;
            }
            let value = if number == 1 || (recorder && number == 2) {
                // device_info field 1 is device_type, NOT manufacturer.
                if recorder && number == 1 {
                    continue;
                }
                manufacturer_found = true;
                1u16
            } else if number == product_field {
                product_found = true;
                product
            } else {
                continue;
            };
            ensure!(size == 2, "Champ d’appareil FIT incompatible");
            output[p..p + 2].copy_from_slice(&if def.big {
                value.to_be_bytes()
            } else {
                value.to_le_bytes()
            });
        }
        if def.global == 0 {
            ensure!(
                manufacturer_found && product_found,
                "Identifiants d’appareil absents du FIT"
            );
            changed_file_id = true;
        }
    }
    if !changed_file_id {
        bail!("Message file_id absent du FIT");
    }
    // A MyWhoosh export can have no device_info at all. Always finish with a
    // complete creator identity; keep all external sensor messages untouched.
    let name = crate::devices::DEVICES
        .iter()
        .find(|d| d.1 == product)
        .context("Appareil Garmin inconnu")?
        .0
        .as_bytes();
    ensure!(name.len() < 255, "Nom d’appareil trop long");
    output.truncate(end);
    if mywhoosh {
        summary::repair(&mut output, input, &sessions, &samples)?;
    }
    output.extend([
        0x40,
        0,
        0,
        23,
        0,
        5,
        0,
        1,
        2,
        2,
        2,
        0x84,
        4,
        2,
        0x84,
        3,
        4,
        0x8c,
        27,
        (name.len() + 1) as u8,
        7,
    ]);
    output.extend([0, 0, 1, 0]); // local message 0, device_index 0, Garmin 1
    output.extend(product.to_le_bytes());
    output.extend(unit_id.unwrap_or(0).to_le_bytes()); // zero only when no real Unit ID was supplied
    output.extend(name);
    output.push(0);
    let new_length = (output.len() - header) as u32;
    output[4..8].copy_from_slice(&new_length.to_le_bytes());
    if header == 14 {
        let checksum = crc(&output[..12]);
        output[12..14].copy_from_slice(&checksum.to_le_bytes());
    }
    let checksum = crc(&output);
    output.extend(checksum.to_le_bytes());
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture(big: bool) -> Vec<u8> {
        let mut data = vec![0x40, 0, big as u8, 0, 0, 3, 0, 1, 0, 1, 2, 0x84, 2, 2, 0x84];
        data.extend([0, 4, 0, 0, 0, 0]);
        // device_info definition, recorder and external sensor share a definition.
        data.extend([0x61, 0, big as u8]);
        data.extend(if big { [0, 23] } else { [23, 0] });
        data.extend([3, 0, 1, 2, 2, 2, 0x84, 4, 2, 0x84, 1, 7, 3, 0]);
        data.extend([1, 0, 0, 0, 0, 0, 9, 8, 7]);
        data.extend([1, 1, 3, 4, 5, 6, 6, 5, 4]);
        // Timestamp compression: timestamp is omitted from compressed data.
        data.extend([0x42, 0, 0, 20, 0, 2, 253, 4, 0x86, 3, 1, 2]);
        data.extend([0xc0, 123]);
        let mut file = vec![14, 0x20, 0, 0];
        file.extend((data.len() as u32).to_le_bytes());
        file.extend(b".FIT");
        file.extend(crc(&file).to_le_bytes());
        file.extend(data);
        file.extend(crc(&file).to_le_bytes());
        file
    }
    #[test]
    fn preserves_sensors_unknown_fields_and_timestamps() {
        for big in [false, true] {
            let input = fixture(big);
            let output = modify_base(&input, 4062, None).unwrap();
            assert_eq!(crc(&output), 0);
            assert!(output
                .windows(b"Edge 840\0".len())
                .any(|w| w == b"Edge 840\0"));
            let mut allowed = vec![4, 5, 6, 7, 12, 13, 31, 32, 33, 34, 56, 57, 58, 59];
            allowed.extend([input.len() - 2, input.len() - 1]);
            for i in 0..input.len() {
                if !allowed.contains(&i) {
                    assert_eq!(input[i], output[i], "offset {i}");
                }
            }
            assert_eq!(word(&output[33..35], big), 4062);
            assert_eq!(word(&output[58..60], big), 4062);
        }
    }
    #[test]
    fn adds_creator_to_exports_without_device_info_for_every_model() {
        // file_id: activity, MyWhoosh, old product 3570, synthetic serial.
        let mut data = vec![
            0x40, 0, 0, 0, 0, 4, 0, 1, 0, 1, 2, 0x84, 2, 2, 0x84, 3, 4, 0x8c,
        ];
        data.extend([0, 4, 0x4b, 1, 0xf2, 0x0d, 1, 2, 3, 4]);
        let mut input = vec![12, 0x20, 0, 0];
        input.extend((data.len() as u32).to_le_bytes());
        input.extend(b".FIT");
        input.extend(data);
        input.extend(crc(&input).to_le_bytes());
        for &(name, product, _) in crate::devices::DEVICES {
            let output = modify_base(&input, product, None).unwrap();
            assert_eq!(crc(&output), 0);
            assert_eq!(word(&output[32..34], false), 1);
            assert_eq!(word(&output[34..36], false), product);
            assert_eq!(&output[36..40], &[0; 4]);
            let creator = &output[40..output.len() - 2];
            assert_eq!(&creator[..6], &[0x40, 0, 0, 23, 0, 5]);
            assert_eq!(&creator[21..25], &[0, 0, 1, 0]);
            assert_eq!(word(&creator[25..27], false), product);
            assert_eq!(&creator[27..31], &[0; 4]);
            assert_eq!(&creator[31..creator.len() - 1], name.as_bytes());
            let with_unit = modify_base(&input, product, Some(123456789)).unwrap();
            assert_eq!(&with_unit[36..40], &123456789u32.to_le_bytes());
            assert_eq!(&with_unit[67..71], &123456789u32.to_le_bytes());
            assert_eq!(crc(&with_unit), 0);
            assert!(modify_base(&input, product, Some(0)).is_err());
        }
    }
    #[test]
    fn repairs_only_mywhoosh_records_and_keeps_zero_effort_samples() {
        for manufacturer in [331u16, 32] {
            let mut data = vec![0x40, 0, 0, 0, 0, 3, 0, 1, 0, 1, 2, 0x84, 2, 2, 0x84];
            data.extend([0, 4]);
            data.extend(manufacturer.to_le_bytes());
            data.extend(3570u16.to_le_bytes());
            data.extend([0x41, 0, 0, 20, 0, 4, 13, 1, 1, 3, 1, 2, 4, 1, 2, 7, 2, 0x84]);
            let record_offset = 12 + data.len();
            data.extend([1, 25, 100, 0, 0, 0]);
            data.extend([1, 25, 200, 90, 200, 0]);
            data.extend([1, 25, 0, 255, 255, 255]);
            data.extend([0x42, 0, 0, 18, 0, 3, 16, 1, 2, 18, 1, 2, 20, 2, 0x84]);
            let session_offset = 12 + data.len();
            data.extend([2, 255, 0, 255, 255]);
            let mut input = vec![12, 0x20, 0, 0];
            input.extend((data.len() as u32).to_le_bytes());
            input.extend(b".FIT");
            input.extend(data);
            input.extend(crc(&input).to_le_bytes());
            let output = modify_base(&input, 4062, None).unwrap();
            assert_eq!(crc(&output), 0);
            if manufacturer == 331 {
                assert_eq!(
                    &output[record_offset..record_offset + 18],
                    &[1, 127, 100, 0, 0, 0, 1, 127, 200, 90, 200, 0, 1, 127, 0, 255, 255, 255]
                );
                assert_eq!(
                    &output[session_offset..session_offset + 5],
                    &[2, 150, 45, 100, 0]
                );
            } else {
                assert_eq!(
                    &output[record_offset..session_offset + 5],
                    &input[record_offset..session_offset + 5]
                );
            }
        }
    }
    #[test]
    fn rejects_corruption_and_every_truncation() {
        let mut input = fixture(false);
        for n in 0..input.len() {
            assert!(modify_base(&input[..n], 4062, None).is_err());
        }
        input[31] ^= 1;
        assert!(modify_base(&input, 4062, None).is_err());
    }
}
