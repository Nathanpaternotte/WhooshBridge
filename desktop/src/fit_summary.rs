//! Repair missing MyWhoosh session summaries from recorded samples.
//! Keep valid summaries, zero power/cadence samples, opaque/developer bytes.
use super::{word, Definition};
use anyhow::{ensure, Result};

pub(super) type Fields = Vec<(u8, usize, usize)>;
pub(super) struct Sample {
    pub timestamp: Option<u32>,
    pub values: [Option<u32>; 3], // heart rate, cadence, power
}
pub(super) struct Session {
    pub start: usize,
    pub end: usize,
    pub definition: Definition,
    pub fields: Fields,
    pub compressed: bool,
}
pub(super) fn number(input: &[u8], fields: &Fields, field: u8, big: bool) -> Option<u32> {
    let &(_, p, size) = fields.iter().find(|f| f.0 == field)?;
    let value = match size {
        1 => u32::from(input[p]),
        2 => u32::from(word(&input[p..p + 2], big)),
        4 => {
            if big {
                u32::from_be_bytes(input[p..p + 4].try_into().ok()?)
            } else {
                u32::from_le_bytes(input[p..p + 4].try_into().ok()?)
            }
        }
        _ => return None,
    };
    let invalid = match size {
        1 => 255,
        2 => 65535,
        _ => u32::MAX,
    };
    (value != invalid).then_some(value)
}
pub(super) fn repair(
    output: &mut Vec<u8>,
    input: &[u8],
    sessions: &[Session],
    samples: &[Sample],
) -> Result<usize> {
    let mut repaired = 0;
    // Reverse order keeps all offsets into the original buffer valid.
    for session in sessions.iter().rev() {
        // Do not rewrite a compressed session definition without expanding its
        // implicit timestamp. Unusual source files retain their original summary.
        if session.compressed {
            continue;
        }
        let big = session.definition.big;
        let start = number(input, &session.fields, 2, big);
        let end = number(input, &session.fields, 253, big);
        if sessions.len() > 1 && (start.is_none() || end.is_none() || start > end) {
            continue;
        }
        let mut sums = [0u64; 3];
        let mut counts = [0u64; 3];
        for sample in samples {
            if sessions.len() > 1
                && !sample
                    .timestamp
                    .is_some_and(|t| t >= start.unwrap() && t <= end.unwrap())
            {
                continue;
            }
            for i in 0..3 {
                if let Some(v) = sample.values[i] {
                    sums[i] += u64::from(v);
                    counts[i] += 1;
                }
            }
        }
        let mut data = output[session.start..session.end].to_vec();
        let mut added_defs = Vec::new();
        let mut added_data = Vec::new();
        for (i, (field, size, base)) in [(16u8, 1usize, 2u8), (18, 1, 2), (20, 2, 0x84)]
            .into_iter()
            .enumerate()
        {
            if counts[i] == 0 {
                continue;
            }
            let existing = session.fields.iter().find(|f| f.0 == field);
            // An unexpected field representation is preserved, never guessed.
            if existing.is_some_and(|f| f.2 != size) {
                continue;
            }
            if number(input, &session.fields, field, big).is_some_and(|v| v > 0) {
                continue;
            }
            let value = (sums[i] / counts[i]) as u16;
            let bytes = if size == 1 {
                vec![value as u8]
            } else if big {
                value.to_be_bytes().to_vec()
            } else {
                value.to_le_bytes().to_vec()
            };
            if let Some(&(_, p, _)) = existing {
                let offset = p - session.start;
                if data[offset..offset + size] == bytes {
                    continue;
                }
                data[offset..offset + size].copy_from_slice(&bytes);
            } else {
                added_defs.extend([field, size as u8, base]);
                added_data.extend(bytes);
            }
            repaired += 1;
        }
        if added_defs.is_empty() {
            output[session.start..session.end].copy_from_slice(&data);
        } else {
            let original = &session.definition.raw;
            let fields = original[5] as usize;
            let count = fields + added_defs.len() / 3;
            ensure!(count <= 255, "Trop de champs dans le résumé FIT");
            let mut definition = original.clone();
            definition[5] = count as u8;
            definition.splice(6 + fields * 3..6 + fields * 3, added_defs);
            let insert = data.len() - session.definition.developer_size;
            data.splice(insert..insert, added_data);
            definition.extend(data);
            // Restore the prior local definition for subsequent records.
            definition.extend(original);
            output.splice(session.start..session.end, definition);
        }
    }
    Ok(repaired)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn session(
        big: bool,
        fields: &[(u8, u8, u8)],
        values: &[u8],
        developer: bool,
    ) -> (Vec<u8>, Session) {
        let mut raw = vec![if developer { 0x60 } else { 0x40 }, 0, big as u8];
        raw.extend(if big {
            18u16.to_be_bytes()
        } else {
            18u16.to_le_bytes()
        });
        raw.push(fields.len() as u8);
        for &(n, s, t) in fields {
            raw.extend([n, s, t]);
        }
        if developer {
            raw.extend([1, 9, 2, 0]);
        }
        let mut input = raw.clone();
        let start = input.len();
        input.push(0);
        input.extend(values);
        if developer {
            input.extend([0xaa, 0xbb]);
        }
        let mut pos = start + 1;
        let offsets = fields
            .iter()
            .map(|&(n, s, _)| {
                let p = pos;
                pos += s as usize;
                (n, p, s as usize)
            })
            .collect();
        let end = input.len();
        let definition = Definition {
            global: 18,
            big,
            fields: fields.iter().map(|&(n, s, _)| (n, s as usize)).collect(),
            developer_size: if developer { 2 } else { 0 },
            raw,
        };
        (
            input,
            Session {
                start,
                end,
                definition,
                fields: offsets,
                compressed: false,
            },
        )
    }
    fn samples() -> Vec<Sample> {
        vec![
            Sample {
                timestamp: Some(10),
                values: [Some(100), Some(0), Some(0)],
            },
            Sample {
                timestamp: Some(20),
                values: [Some(200), Some(90), Some(200)],
            },
            Sample {
                timestamp: Some(30),
                values: [None, None, None],
            },
        ]
    }
    #[test]
    fn repairs_invalid_summaries_in_both_byte_orders_preserving_valid_values() {
        for big in [false, true] {
            let (input, s) = session(
                big,
                &[(16, 1, 2), (18, 1, 2), (20, 2, 0x84)],
                &[255, 0, 255, 255],
                false,
            );
            let start = s.start;
            let mut output = input.clone();
            assert_eq!(repair(&mut output, &input, &[s], &samples()).unwrap(), 3);
            assert_eq!(&output[start + 1..start + 3], &[150, 45]);
            assert_eq!(word(&output[start + 3..], big), 100);
            let (input, s) = session(
                big,
                &[(16, 1, 2), (18, 1, 2), (20, 2, 0x84)],
                &[140, 80, 1, 1],
                false,
            );
            let mut output = input.clone();
            assert_eq!(repair(&mut output, &input, &[s], &samples()).unwrap(), 0);
            assert_eq!(output, input);
        }
    }
    #[test]
    fn inserts_missing_fields_before_opaque_developer_bytes_and_restores_definition() {
        let (input, s) = session(false, &[(250, 3, 13)], &[4, 5, 6], true);
        let raw = s.definition.raw.clone();
        let start = s.start;
        let mut output = input.clone();
        assert_eq!(repair(&mut output, &input, &[s], &samples()).unwrap(), 3);
        let mut expected = raw.clone();
        expected[5] = 4;
        expected.splice(9..9, [16, 1, 2, 18, 1, 2, 20, 2, 0x84]);
        expected.extend([0, 4, 5, 6, 150, 45, 100, 0, 0xaa, 0xbb]);
        expected.extend(&raw);
        assert_eq!(&output[start..], &expected);
        assert_eq!(&output[..start], &input[..start]);
    }
    #[test]
    fn no_measurements_means_no_invented_summary() {
        let (input, s) = session(false, &[(16, 1, 2)], &[255], false);
        let mut output = input.clone();
        assert_eq!(repair(&mut output, &input, &[s], &[]).unwrap(), 0);
        assert_eq!(output, input);
    }
    #[test]
    fn multisession_uses_each_sessions_time_range() {
        let fields = [(2, 4, 0x86), (253, 4, 0x86), (16, 1, 2)];
        let mut values = 10u32.to_le_bytes().to_vec();
        values.extend(19u32.to_le_bytes());
        values.push(255);
        let (mut input, one) = session(false, &fields, &values, false);
        values[0..4].copy_from_slice(&20u32.to_le_bytes());
        values[4..8].copy_from_slice(&30u32.to_le_bytes());
        let (second, mut two) = session(false, &fields, &values, false);
        let offset = input.len();
        two.start += offset;
        two.end += offset;
        for f in &mut two.fields {
            f.1 += offset;
        }
        input.extend(second);
        let p1 = one.fields[2].1;
        let p2 = two.fields[2].1;
        let mut output = input.clone();
        assert_eq!(
            repair(&mut output, &input, &[one, two], &samples()).unwrap(),
            6
        );
        // Each session acquired two missing fields; compare preserved start/end + HR sequences.
        assert!(output
            .windows(9)
            .any(|w| w == [10, 0, 0, 0, 19, 0, 0, 0, 100]));
        assert!(output
            .windows(9)
            .any(|w| w == [20, 0, 0, 0, 30, 0, 0, 0, 200]));
        assert_eq!(input[p1], 255);
        assert_eq!(input[p2], 255);
    }
}
