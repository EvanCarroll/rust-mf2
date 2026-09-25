//! `third_party/*/PIN` files: ordered `key = value` fields; a value continues on
//! following lines that start with whitespace.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

use crate::error::{Error, Result};

/// Width the keys are padded to (`upstream` is the longest key in use).
const KEY_WIDTH: usize = 8;

/// A parsed PIN file. Rendering preserves field order and the house layout.
pub(crate) struct Pin {
    path: PathBuf,
    fields: Vec<(String, String)>,
}

impl Pin {
    pub(crate) fn load(path: &Path) -> Result<Self> {
        let text = fs::read_to_string(path).map_err(|source| Error::IoAt {
            path: path.to_path_buf(),
            source,
        })?;
        let mut fields: Vec<(String, String)> = Vec::new();
        for line in text.lines() {
            if line.trim().is_empty() {
                continue;
            }
            if line.starts_with([' ', '\t']) {
                let Some((_, value)) = fields.last_mut() else {
                    return Err(Error::Pin {
                        path: path.to_path_buf(),
                        message: "continuation line before the first field".to_owned(),
                    });
                };
                value.push('\n');
                value.push_str(line.trim());
            } else {
                let (key, value) = line.split_once('=').ok_or_else(|| Error::Pin {
                    path: path.to_path_buf(),
                    message: format!("expected `key = value`, found {line:?}"),
                })?;
                fields.push((key.trim().to_owned(), value.trim().to_owned()));
            }
        }
        Ok(Self {
            path: path.to_path_buf(),
            fields,
        })
    }

    pub(crate) fn get(&self, key: &str) -> Result<&str> {
        self.fields
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
            .ok_or_else(|| Error::Pin {
                path: self.path.clone(),
                message: format!("missing field `{key}`"),
            })
    }

    /// The `digests` field — `<sha256>  <path>` lines, as `sha256sum` writes
    /// them — by path; `None` when the PIN has no such field.
    pub(crate) fn digests(&self) -> Result<Option<BTreeMap<String, String>>> {
        let Ok(text) = self.get("digests") else {
            return Ok(None);
        };
        parse_digests(text).map(Some).map_err(|message| Error::Pin {
            path: self.path.clone(),
            message,
        })
    }

    /// Sets `key`, keeping its position; a new key goes after `after` (or last).
    pub(crate) fn set(&mut self, key: &str, value: &str, after: Option<&str>) {
        if let Some(slot) = self.fields.iter_mut().find(|(k, _)| k == key) {
            value.clone_into(&mut slot.1);
            return;
        }
        let at = after
            .and_then(|a| self.fields.iter().position(|(k, _)| k == a))
            .map_or(self.fields.len(), |i| i + 1);
        self.fields.insert(at, (key.to_owned(), value.to_owned()));
    }

    pub(crate) fn render(&self) -> String {
        let mut out = String::new();
        for (key, value) in &self.fields {
            let width = KEY_WIDTH.max(key.len());
            let indent = " ".repeat(width + 3);
            let mut lines = value.lines();
            let _ = writeln!(out, "{key:<width$} = {}", lines.next().unwrap_or(""));
            for line in lines {
                out.push_str(&indent);
                out.push_str(line);
                out.push('\n');
            }
        }
        out
    }

    pub(crate) fn save(&self) -> Result<()> {
        fs::write(&self.path, self.render()).map_err(|source| Error::IoAt {
            path: self.path.clone(),
            source,
        })
    }
}

/// `<sha256>  <path>` lines by path.
fn parse_digests(text: &str) -> std::result::Result<BTreeMap<String, String>, String> {
    let mut out = BTreeMap::new();
    for line in text.lines().filter(|l| !l.trim().is_empty()) {
        let (digest, path) = line
            .split_once("  ")
            .ok_or_else(|| format!("digests: malformed line {line:?}"))?;
        out.insert(path.trim().to_owned(), digest.trim().to_owned());
    }
    Ok(out)
}

/// Lower-case hex SHA-256 of `bytes`.
pub(crate) fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    Sha256::digest(bytes)
        .iter()
        .fold(String::new(), |mut s, b| {
            let _ = write!(s, "{b:02x}");
            s
        })
}

#[cfg(test)]
mod tests {
    use super::{Pin, parse_digests};

    #[test]
    fn digests_are_sha256sum_lines() {
        let d = parse_digests("ab  schemas/a.xsd\ncd  b.html").unwrap();
        assert_eq!(d["schemas/a.xsd"], "ab");
        assert_eq!(d["b.html"], "cd");
        assert!(parse_digests("nospace").is_err());
    }

    #[test]
    fn round_trips_the_house_layout() {
        let text = "upstream = https://example.org/x\n\
                    commit   = 0123\n\
                    vendored = first line\n           second line\n\
                    refresh  = cargo xtask x-sync [--rev <sha>]   (note)\n";
        let dir = std::env::temp_dir().join(format!("xtask-pin-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("PIN");
        std::fs::write(&path, text).unwrap();
        let mut pin = Pin::load(&path).unwrap();
        assert_eq!(pin.render(), text);
        assert_eq!(pin.get("vendored").unwrap(), "first line\nsecond line");
        pin.set("layout", "a\nb", Some("commit"));
        assert!(
            pin.render()
                .contains("commit   = 0123\nlayout   = a\n           b\n")
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
