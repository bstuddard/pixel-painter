// SPDX-License-Identifier: GPL-3.0-only
//! Terminal-safe JSON encoding without changing decoded strings or buffering reports.

use painter::Result;
use serde::Serialize;
use std::io::{self, Write};

pub(crate) fn write(writer: &mut dyn Write, value: &impl Serialize, pretty: bool) -> Result<()> {
    let mut output = EscapingWriter {
        inner: writer,
        pending_c2: false,
    };
    if pretty {
        serde_json::to_writer_pretty(&mut output, value)?;
    } else {
        serde_json::to_writer(&mut output, value)?;
    }
    output.flush()?;
    Ok(())
}

struct EscapingWriter<'a> {
    inner: &'a mut dyn Write,
    pending_c2: bool,
}
impl Write for EscapingWriter<'_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let mut start = 0;
        for (index, &byte) in bytes.iter().enumerate() {
            // serde_json emits valid UTF-8, but may split a codepoint across writes.
            // Its existing escapes cover C0; raw C1 uses the two bytes C2 80..9F.
            if self.pending_c2 {
                self.pending_c2 = false;
                if (0x80..=0x9f).contains(&byte) {
                    const HEX: &[u8; 16] = b"0123456789abcdef";
                    self.inner.write_all(&[
                        b'\\',
                        b'u',
                        b'0',
                        b'0',
                        HEX[(byte >> 4) as usize],
                        HEX[(byte & 15) as usize],
                    ])?;
                    start = index + 1;
                    continue;
                }
                self.inner.write_all(&[0xc2])?;
            }
            if byte == 0xc2 || byte == 0x7f {
                self.inner.write_all(&bytes[start..index])?;
                if byte == 0xc2 {
                    self.pending_c2 = true;
                } else {
                    self.inner.write_all(b"\\u007f")?;
                }
                start = index + 1;
            }
        }
        self.inner.write_all(&bytes[start..])?;
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        if self.pending_c2 {
            self.inner.write_all(&[0xc2])?;
            self.pending_c2 = false;
        }
        self.inner.flush()
    }
}

#[cfg(test)]
mod tests {
    use super::{EscapingWriter, write};
    use serde_json::json;
    use std::io::Write;

    fn fixture() -> serde_json::Value {
        let controls: String = (0..=0x9f).filter_map(char::from_u32).collect();
        json!({"text":format!("{controls}é\u{00a0}漢字😀 literal \\u009b")})
    }
    #[test]
    fn controls_are_escaped_and_json_strings_round_trip_in_both_modes() {
        for pretty in [false, true] {
            let value = fixture();
            let mut bytes = Vec::new();
            write(&mut bytes, &value, pretty).unwrap();
            let encoded = String::from_utf8(bytes.clone()).unwrap();
            assert!(encoded.chars().all(|c| !c.is_control() || c == '\n'));
            assert!(encoded.contains("é\u{00a0}漢字😀"));
            assert_eq!(
                serde_json::from_slice::<serde_json::Value>(&bytes).unwrap(),
                value
            );
        }
    }
    #[test]
    fn utf8_and_control_escapes_survive_every_write_boundary() {
        let value = fixture();
        let bytes = serde_json::to_vec(&value).unwrap();
        for split in 0..=bytes.len() {
            let mut output = Vec::new();
            let mut writer = EscapingWriter {
                inner: &mut output,
                pending_c2: false,
            };
            writer.write_all(&bytes[..split]).unwrap();
            writer.write_all(&bytes[split..]).unwrap();
            writer.flush().unwrap();
            assert!(
                String::from_utf8_lossy(&output)
                    .chars()
                    .all(|c| !c.is_control())
            );
            assert_eq!(
                serde_json::from_slice::<serde_json::Value>(&output).unwrap(),
                value
            );
        }
    }
}
