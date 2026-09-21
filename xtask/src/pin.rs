//! `third_party/*/PIN` files: ordered `key = value` fields; a value continues on
//! following lines that start with whitespace.

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

#[cfg(test)]
mod tests {
    use super::Pin;

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
