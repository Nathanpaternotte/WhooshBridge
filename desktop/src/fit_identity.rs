//! Normalize standard recorder metadata without changing accessory identities.
use super::{crc, word, Definition};
use anyhow::{ensure, Context, Result};

pub(super) fn normalize(input: &[u8], product: u16, unit: Option<u32>) -> Result<Vec<u8>> {
    let software = crate::devices::firmware(product).context("Firmware du modèle non vérifié")?;
    let name = crate::devices::DEVICES
        .iter()
        .find(|d| d.1 == product)
        .context("Appareil Garmin inconnu")?
        .0;
    let header = input[0] as usize;
    let end = input.len() - 2;
    let mut out = input[..header].to_vec();
    let mut pos = header;
    let mut defs: [Option<Definition>; 16] = std::array::from_fn(|_| None);
    let mut creator = false;
    while pos < end {
        let start = pos;
        let tag = input[pos];
        pos += 1;
        let compressed = tag & 128 != 0;
        let local = if compressed {
            ((tag >> 5) & 3) as usize
        } else {
            (tag & 15) as usize
        };
        if !compressed && tag & 64 != 0 {
            let big = input[pos + 1] == 1;
            let global = word(&input[pos + 2..pos + 4], big);
            let count = input[pos + 4] as usize;
            pos += 5;
            let mut fields = Vec::new();
            for _ in 0..count {
                fields.push((input[pos], input[pos + 1] as usize));
                pos += 3;
            }
            let mut developer_size = 0;
            if tag & 32 != 0 {
                let count = input[pos] as usize;
                pos += 1;
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
                raw: input[start..pos].to_vec(),
            });
            out.extend_from_slice(&input[start..pos]);
            continue;
        }
        let def = defs[local].as_ref().context("Définition FIT absente")?;
        let mut fields = Vec::new();
        for &(n, s) in &def.fields {
            if compressed && n == 253 {
                continue;
            }
            fields.push((n, pos, s));
            pos += s;
        }
        let developer_start = pos;
        pos += def.developer_size;
        let get = |n| super::summary::number(input, &fields, n, def.big);
        let recorder = def.global == 23
            && (get(0) == Some(0) || (get(0).is_none_or(|i| i == 255) && get(2) == Some(331)));
        if !matches!(def.global, 0 | 49 | 72) && !recorder {
            out.extend_from_slice(&input[start..pos]);
            continue;
        }
        let u16bytes = |v: u16| {
            if def.big {
                v.to_be_bytes().to_vec()
            } else {
                v.to_le_bytes().to_vec()
            }
        };
        let serial = if def.big {
            unit.unwrap_or(0).to_be_bytes()
        } else {
            unit.unwrap_or(0).to_le_bytes()
        };
        let mut text = name.as_bytes().to_vec();
        text.push(0);
        let mut replacements: Vec<(u8, u8, Vec<u8>)> = match def.global {
            49 => {
                creator = true;
                vec![(0, 0x84, u16bytes(software))]
            }
            23 => vec![
                (0, 2, vec![0]),
                (2, 0x84, u16bytes(1)),
                (3, 0x8c, serial.to_vec()),
                (4, 0x84, u16bytes(product)),
                (5, 0x84, u16bytes(software)),
                (19, 7, text.clone()),
                (27, 7, text.clone()),
            ],
            _ => vec![
                (1, 0x84, u16bytes(1)),
                (2, 0x84, u16bytes(product)),
                (3, 0x8c, serial.to_vec()),
            ],
        };
        if def.global == 0 {
            replacements.push((8, 7, text));
        }
        // A hardware revision from MyWhoosh must not masquerade as Garmin's.
        let hardware = if def.global == 23 { 6 } else { 1 };
        if matches!(def.global, 23 | 49) && fields.iter().any(|f| f.0 == hardware) {
            replacements.push((hardware, 2, vec![255]));
        }
        let mut new_fields = Vec::new();
        let mut data = vec![tag];
        for (i, &(n, s)) in def.fields.iter().enumerate() {
            if let Some(index) = replacements.iter().position(|r| r.0 == n) {
                let (_, base, bytes) = replacements.remove(index);
                new_fields.extend([n, bytes.len() as u8, base]);
                data.extend(bytes);
            } else {
                new_fields.extend_from_slice(&def.raw[6 + i * 3..9 + i * 3]);
                if !(compressed && n == 253) {
                    let &(_, p, _) = fields.iter().find(|f| f.0 == n).unwrap();
                    data.extend_from_slice(&input[p..p + s]);
                }
            }
        }
        for (n, base, bytes) in replacements {
            new_fields.extend([n, bytes.len() as u8, base]);
            data.extend(bytes);
        }
        ensure!(new_fields.len() / 3 <= 255, "Trop de champs FIT");
        let mut raw = def.raw[..6].to_vec();
        raw[5] = (new_fields.len() / 3) as u8;
        raw.extend(new_fields);
        raw.extend_from_slice(&def.raw[6 + def.fields.len() * 3..]);
        data.extend_from_slice(&input[developer_start..pos]);
        out.extend(raw);
        out.extend(data);
        // Restore a shared definition so accessory records retain their layout.
        out.extend_from_slice(&def.raw);
    }
    if !creator {
        out.extend([0x40, 0, 0, 49, 0, 1, 0, 2, 0x84, 0]);
        out.extend(software.to_le_bytes());
    }
    let length = (out.len() - header) as u32;
    out[4..8].copy_from_slice(&length.to_le_bytes());
    if header == 14 {
        let c = crc(&out[..12]);
        out[12..14].copy_from_slice(&c.to_le_bytes());
    }
    out.extend(crc(&out).to_le_bytes());
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    // Decode via the independent FIT library in the Python integration check too.
    #[test]
    fn normalizes_all_identity_messages_and_preserves_shared_sensor_definition() {
        for big in [false, true] {
            let u16b = |v: u16| {
                if big {
                    v.to_be_bytes()
                } else {
                    v.to_le_bytes()
                }
            };
            let mut body = Vec::new();
            let mut message = |global: u16, fields: &[(u8, u8, u8)], data: &[u8]| {
                body.extend([0x40, 0, big as u8]);
                body.extend(u16b(global));
                body.push(fields.len() as u8);
                for &(n, s, b) in fields {
                    body.extend([n, s, b]);
                }
                body.push(0);
                body.extend(data);
            };
            let mut file = vec![4];
            file.extend(u16b(331));
            file.extend(u16b(3570));
            message(0, &[(0, 1, 0), (1, 2, 0x84), (2, 2, 0x84)], &file);
            let mut dev = vec![0];
            dev.extend(u16b(331));
            dev.extend(u16b(3570));
            dev.extend(u16b(0));
            dev.extend(b"Old\0");
            message(
                23,
                &[
                    (0, 1, 2),
                    (2, 2, 0x84),
                    (4, 2, 0x84),
                    (5, 2, 0x84),
                    (27, 4, 7),
                ],
                &dev,
            );
            let mut sensor = vec![0, 1];
            sensor.extend(u16b(32));
            sensor.extend(u16b(77));
            sensor.extend(u16b(123));
            sensor.extend(b"HRM\0");
            body.extend(&sensor);
            // file_creator without firmware and training_file with old identity.
            body.extend([0x41, 0, 0, 49, 0, 1, 1, 1, 2, 1, 9]);
            body.extend([
                0x42, 0, 0, 72, 0, 2, 1, 2, 0x84, 2, 2, 0x84, 2, 75, 1, 242, 13,
            ]);
            let mut input = vec![12, 32, 0, 0];
            input.extend((body.len() as u32).to_le_bytes());
            input.extend(b".FIT");
            input.extend(body);
            input.extend(crc(&input).to_le_bytes());
            let output = super::super::modify(&input, 4440, Some(123456789)).unwrap();
            assert_eq!(crc(&output), 0);
            assert!(output.windows(sensor.len()).any(|w| w == sensor));
            assert!(!output.windows(4).any(|w| w == b"Old\0"));
            assert!(output.windows(10).any(|w| w == b"Edge 1050\0"));
            // Re-editing verifies that inserted/restored definitions remain parseable.
            let again = super::super::modify(&output, 4440, Some(123456789)).unwrap();
            assert_eq!(crc(&again), 0);
        }
    }
}
