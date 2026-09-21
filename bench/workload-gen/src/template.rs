//! Pluggable call-site templates (format documented in the crate README).
//!
//! A template is a directory holding `template.toml` and, optionally, a
//! support module (`support.rs`). It says, per call-site shape × argument
//! mode, which Rust expression a site becomes; placeholders are written
//! `{{name}}`. Swapping the template swaps the implementation under test
//! without touching the generator.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::error::Error;
use crate::model::VarKind;
use crate::sites::{Mode, Shape};

/// Built-in templates: `(name, template.toml, support.rs)`.
const BUILTIN: &[(&str, &str, &str)] = &[
    (
        "literal",
        include_str!("../templates/literal/template.toml"),
        include_str!("../templates/literal/support.rs"),
    ),
    (
        "closure",
        include_str!("../templates/closure/template.toml"),
        include_str!("../templates/closure/support.rs"),
    ),
];

/// Names of the built-in templates.
pub fn builtin_names() -> impl Iterator<Item = &'static str> {
    BUILTIN.iter().map(|b| b.0)
}

/// The two files of a built-in template, for `templates --dump`.
pub fn builtin_files(name: &str) -> Option<(&'static str, &'static str)> {
    BUILTIN.iter().find(|b| b.0 == name).map(|b| (b.1, b.2))
}

fn default_sep() -> String {
    ", ".to_owned()
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct Args {
    #[serde(default = "default_sep")]
    sep: String,
    plain: Option<String>,
    get: Option<String>,
    signal: Option<String>,
}

impl Default for Args {
    fn default() -> Self {
        Self {
            sep: default_sep(),
            plain: None,
            get: None,
            signal: None,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct Deferred {
    #[serde(rename = "type")]
    ty: String,
    view: String,
    string: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct SiteTable {
    none: Option<String>,
    rich: Option<String>,
    plain: Option<String>,
    signal: Option<String>,
    get: Option<String>,
}

impl SiteTable {
    fn get(&self, mode: Mode) -> Option<&str> {
        let chain: &[&Option<String>] = match mode {
            Mode::None => &[&self.none],
            Mode::Rich => &[&self.rich, &self.none],
            Mode::Plain => &[&self.plain, &self.none],
            Mode::Get => &[&self.get, &self.plain, &self.none],
            Mode::Signal => &[&self.signal, &self.get, &self.plain, &self.none],
        };
        chain.iter().find_map(|o| o.as_deref())
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct Raw {
    name: String,
    description: String,
    #[serde(default)]
    dependencies: Vec<String>,
    #[serde(default)]
    prelude: String,
    #[serde(default)]
    support: Option<String>,
    #[serde(default)]
    boot: String,
    #[serde(default)]
    args: Args,
    deferred: Deferred,
    site: BTreeMap<String, SiteTable>,
}

/// A loaded, validated template.
#[derive(Debug, Clone)]
pub struct Template {
    /// Template name.
    pub name: String,
    /// One-paragraph description.
    pub description: String,
    /// Extra `[dependencies]` lines for the generated app (verbatim, with
    /// `{{template_dir}}` expanded).
    pub dependencies: Vec<String>,
    /// Rust items inserted at the top of every component module and of
    /// `src/tables.rs`.
    pub prelude: String,
    /// Contents of the generated `src/support.rs`.
    pub support: String,
    /// Statements run at the start of the `hydrate` entry point.
    pub boot: String,
    args: Args,
    deferred: Deferred,
    sites: BTreeMap<String, SiteTable>,
}

/// One argument at a call site.
#[derive(Debug, Clone)]
pub struct ArgCtx {
    /// MF2 variable name (= Rust argument name).
    pub name: &'static str,
    /// Positional slot: rank of the name in ascending bytewise order
    /// (plans/05 §3). Arguments are listed in slot order.
    pub slot: usize,
    /// Value kind.
    pub kind: VarKind,
}

impl ArgCtx {
    /// Plain (non-reactive) value expression in scope at every site.
    pub fn plain(&self) -> &'static str {
        match self.kind {
            VarKind::Num => "n",
            VarKind::Str => "who",
            VarKind::Date => "when",
        }
    }

    /// Signal in scope at every view site.
    pub fn signal(&self) -> &'static str {
        match self.kind {
            VarKind::Num => "count",
            VarKind::Str => "name",
            VarKind::Date => "stamp",
        }
    }
}

/// Everything a site snippet can refer to.
#[derive(Debug, Clone)]
pub struct SiteCtx {
    /// Message id.
    pub id: String,
    /// `MsgId` (dense index in sorted-id order).
    pub index: u32,
    /// Source-locale text, escaped for a Rust string literal.
    pub text: String,
    /// Arguments in slot order (empty for `none`/`rich`).
    pub args: Vec<ArgCtx>,
    /// Global site number.
    pub site: usize,
    /// Markup names, comma-separated (empty for most messages).
    pub markup: String,
}

/// Expands `{{name}}` placeholders in `text`.
fn expand(
    template: &str,
    text: &str,
    lookup: &dyn Fn(&str) -> Option<String>,
) -> Result<String, Error> {
    let mut out = String::with_capacity(text.len() + 32);
    let mut rest = text;
    while let Some(start) = rest.find("{{") {
        out.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        let end = after.find("}}").ok_or_else(|| Error::Template {
            template: template.to_owned(),
            message: format!("unclosed `{{{{` in `{text}`"),
        })?;
        let key = &after[..end];
        let value = lookup(key).ok_or_else(|| Error::Template {
            template: template.to_owned(),
            message: format!("unknown placeholder `{{{{{key}}}}}` in `{text}`"),
        })?;
        out.push_str(&value);
        rest = &after[end + 2..];
    }
    out.push_str(rest);
    Ok(out)
}

impl Template {
    /// A built-in template by name.
    pub fn builtin(name: &str) -> Result<Self, Error> {
        let (toml_src, support) = builtin_files(name).ok_or_else(|| Error::Template {
            template: name.to_owned(),
            message: format!(
                "no such built-in template (built-ins: {})",
                builtin_names().collect::<Vec<_>>().join(", ")
            ),
        })?;
        let raw: Raw = toml::from_str(toml_src)?;
        Self::from_raw(raw, None, support.to_owned())
    }

    /// A template directory (`template.toml` + optional support file).
    pub fn load(dir: &Path) -> Result<Self, Error> {
        let manifest = dir.join("template.toml");
        if !manifest.is_file() {
            return Err(Error::NotFound(manifest));
        }
        let raw: Raw = toml::from_str(&std::fs::read_to_string(&manifest)?)?;
        let support = match &raw.support {
            Some(file) => std::fs::read_to_string(dir.join(file))?,
            None => String::new(),
        };
        let dir = std::fs::canonicalize(dir)?;
        Self::from_raw(raw, Some(&dir), support)
    }

    /// A directory if `spec` names one that holds `template.toml`, otherwise
    /// a built-in name.
    pub fn resolve(spec: &str) -> Result<Self, Error> {
        let path = PathBuf::from(spec);
        if path.join("template.toml").is_file() {
            Self::load(&path)
        } else {
            Self::builtin(spec)
        }
    }

    fn from_raw(raw: Raw, dir: Option<&Path>, support: String) -> Result<Self, Error> {
        let name = raw.name.clone();
        for key in raw.site.keys() {
            let known = Shape::ALL.iter().any(|s| s.key() == key) || key == "reactive_prop";
            if !known {
                return Err(Error::Template {
                    template: name,
                    message: format!("unknown site table `[site.{key}]`"),
                });
            }
        }
        let dir_text = dir.map(|d| d.display().to_string());
        let dependencies = raw
            .dependencies
            .iter()
            .map(|line| {
                expand(&name, line, &|k| match k {
                    "template_dir" => dir_text.clone(),
                    _ => None,
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let template = Self {
            name: raw.name,
            description: raw.description,
            dependencies,
            prelude: raw.prelude,
            support,
            boot: raw.boot,
            args: raw.args,
            deferred: raw.deferred,
            sites: raw.site,
        };
        template.validate()?;
        Ok(template)
    }

    fn table(&self, shape: Shape) -> Option<&SiteTable> {
        self.sites.get(shape.key()).or_else(|| match shape {
            Shape::TextProp | Shape::SignalProp => self.sites.get("reactive_prop"),
            _ => None,
        })
    }

    fn error(&self, message: String) -> Error {
        Error::Template {
            template: self.name.clone(),
            message,
        }
    }

    /// The arguments' item template and the effective mode for values.
    fn arg_item(&self, mode: Mode) -> Option<(&str, Mode)> {
        match mode {
            Mode::Signal => self
                .args
                .signal
                .as_deref()
                .map(|s| (s, Mode::Signal))
                .or_else(|| self.arg_item(Mode::Get)),
            Mode::Get => self
                .args
                .get
                .as_deref()
                .or(self.args.plain.as_deref())
                .map(|s| (s, Mode::Get)),
            _ => self.args.plain.as_deref().map(|s| (s, Mode::Plain)),
        }
    }

    fn render_args(&self, mode: Mode, args: &[ArgCtx]) -> Result<String, Error> {
        let (item, effective) = self.arg_item(mode).ok_or_else(|| {
            self.error("a snippet uses `{{args}}` but `[args] plain` is missing".into())
        })?;
        let mut parts = Vec::with_capacity(args.len());
        for arg in args {
            let value = match effective {
                Mode::Signal => arg.signal().to_owned(),
                Mode::Get => format!("{}.get()", arg.signal()),
                _ => arg.plain().to_owned(),
            };
            parts.push(expand(&self.name, item, &|k| match k {
                "name" => Some(arg.name.to_owned()),
                "slot" => Some(arg.slot.to_string()),
                "value" => Some(value.clone()),
                "signal" => Some(arg.signal().to_owned()),
                "kind" => Some(arg.kind.name().to_owned()),
                _ => None,
            })?);
        }
        Ok(parts.join(&self.args.sep))
    }

    /// The Rust expression for one call site.
    pub fn render_site(&self, shape: Shape, mode: Mode, ctx: &SiteCtx) -> Result<String, Error> {
        let snippet = self.table(shape).and_then(|t| t.get(mode)).ok_or_else(|| {
            self.error(format!(
                "no snippet for `[site.{}] {}` (nor a fallback)",
                shape.key(),
                mode.key()
            ))
        })?;
        let args = if snippet.contains("{{args}}") {
            self.render_args(mode, &ctx.args)?
        } else {
            String::new()
        };
        expand(&self.name, snippet, &|k| match k {
            "id" => Some(ctx.id.clone()),
            "index" => Some(ctx.index.to_string()),
            "text" => Some(ctx.text.clone()),
            "args" => Some(args.clone()),
            "nargs" => Some(ctx.args.len().to_string()),
            "site" => Some(ctx.site.to_string()),
            "markup" => Some(ctx.markup.clone()),
            _ => None,
        })
    }

    /// Type of a deferred label field in a `static` table.
    pub fn deferred_type(&self) -> &str {
        &self.deferred.ty
    }

    /// A deferred label (`label` is an expression of the label type) as a
    /// view child.
    pub fn deferred_view(&self, label: &str) -> Result<String, Error> {
        expand(&self.name, &self.deferred.view, &|k| {
            (k == "label").then(|| label.to_owned())
        })
    }

    /// A deferred label as a `String`.
    pub fn deferred_string(&self, label: &str) -> Result<String, Error> {
        expand(&self.name, &self.deferred.string, &|k| {
            (k == "label").then(|| label.to_owned())
        })
    }

    /// Renders every shape × mode with a dummy context, so that a template is
    /// known complete before any app is generated.
    pub fn validate(&self) -> Result<(), Error> {
        let args = vec![
            ArgCtx {
                name: "count",
                slot: 0,
                kind: VarKind::Num,
            },
            ArgCtx {
                name: "user",
                slot: 1,
                kind: VarKind::Str,
            },
        ];
        for shape in Shape::ALL {
            for &mode in shape.modes() {
                let ctx = SiteCtx {
                    id: "a.b".into(),
                    index: 0,
                    text: "t".into(),
                    args: if mode.has_args() {
                        args.clone()
                    } else {
                        Vec::new()
                    },
                    site: 0,
                    markup: String::new(),
                };
                self.render_site(shape, mode, &ctx)?;
            }
        }
        self.deferred_view("row.label")?;
        self.deferred_string("row.label")?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{Template, builtin_names};

    #[test]
    fn builtins_validate() {
        for name in builtin_names() {
            Template::builtin(name).unwrap();
        }
        assert!(Template::builtin("nope").is_err());
    }
}
