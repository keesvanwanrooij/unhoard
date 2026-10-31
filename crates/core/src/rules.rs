//! The known-locations library: a switch-case kept as data.
//!
//! Rules live in TOML files in `data/known-locations/`. Each rule names a parent folder
//! pattern and a child name pattern, plus optional conditions (a sibling marker, a minimum
//! age or size). The scanner asks for matches once per directory listing, so conditions
//! are checked against the listing itself and nothing extra is read. When several rules
//! match, the most specific pattern wins; on a tie the strictest tier wins, so an item is
//! never shown as safer than it is. The explanations live in the locale files under
//! `knownLocations.<id>`.

use std::collections::HashSet;

use globset::{Glob, GlobBuilder, GlobMatcher, GlobSet, GlobSetBuilder};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::fs::DirEntry;
use crate::roots::Roots;
use crate::time::filetime_to_unix;
use crate::tree::Category;

/// The risk tier of an item. Ordered from least to most strict.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Tier {
    /// Regenerable: the app or Windows rebuilds it.
    Safe,
    /// Usually unneeded, but it was yours or an app's.
    LikelySafe,
    /// Only the user can judge.
    Review,
    /// Needed by Windows or an app: explain only.
    Keep,
}

/// What a rule matches.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RuleKind {
    /// Folders only.
    Dir,
    /// Files only.
    File,
    /// Folders and files.
    #[default]
    Any,
}

/// A Windows tool Unhoard can open (an allowlist; nothing else is ever started).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum WindowsTool {
    /// Settings › System › Storage.
    StorageSettings,
    /// Settings › Storage Sense.
    StorageSense,
    /// Disk Cleanup (`cleanmgr.exe`).
    DiskCleanup,
    /// Settings › Apps › Installed apps.
    Apps,
    /// System Properties › System Protection.
    SystemProtection,
    /// The Recycle Bin in Explorer.
    RecycleBin,
}

/// What Unhoard offers to do with an item.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "type", content = "tool")]
pub enum Action {
    /// Move to the quarantine (local, or the cloud quarantine inside a sync root).
    Quarantine,
    /// Make cloud files online-only.
    FreeUpSpace,
    /// Open a Windows tool that cleans this safely.
    WindowsTool(WindowsTool),
    /// Remove through the app's own uninstaller or launcher.
    Uninstaller,
    /// Open the folder in Explorer.
    ShowInExplorer,
    /// Explain only.
    ExplainOnly,
}

/// One known-locations rule, with patterns already expanded.
#[derive(Clone, Debug)]
pub struct Rule {
    /// Stable ID, used for text keys, the manifest and the CLI output.
    pub id: String,
    /// Folders, files or both.
    pub kind: RuleKind,
    /// A file or folder (glob) that must exist next to the match.
    pub sibling: Option<String>,
    /// A folder named like the match without its extension must exist next to it (an
    /// archive that was already extracted).
    pub stem_sibling: bool,
    /// The item must not have changed for this many days.
    pub min_age_days: Option<u32>,
    /// A file must be at least this large.
    pub min_bytes: Option<u64>,
    /// The kind of content.
    pub category: Category,
    /// The risk tier.
    pub tier: Tier,
    /// The primary action.
    pub action: Action,
    /// The action needs admin rights, so Unhoard explains and points to the Windows tool.
    pub requires_admin: bool,
    /// Official pages the explanation is based on.
    pub docs: Vec<String>,
    /// A path that must match this rule (generated fixture).
    pub example: Option<String>,
    /// A similar path that must not match this rule.
    pub near_miss: Option<String>,
    name: GlobMatcher,
    sibling_glob: Option<GlobMatcher>,
    specificity: usize,
}

/// A rule file could not be read.
#[derive(Debug, Error)]
pub enum RuleError {
    /// The TOML is invalid.
    #[error("invalid rule file: {0}")]
    Toml(String),
    /// A pattern is invalid.
    #[error("rule {id}: invalid pattern: {message}")]
    Pattern {
        /// The rule ID.
        id: String,
        /// What is wrong.
        message: String,
    },
    /// An action is unknown.
    #[error("rule {id}: unknown action {action}")]
    Action {
        /// The rule ID.
        id: String,
        /// The action text.
        action: String,
    },
    /// Two rules share an ID.
    #[error("duplicate rule id {0}")]
    Duplicate(String),
}

#[derive(Deserialize)]
struct RuleFile {
    #[serde(default)]
    rule: Vec<RawRule>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RawRule {
    id: String,
    parent: String,
    name: String,
    #[serde(default)]
    kind: RuleKind,
    sibling: Option<String>,
    #[serde(default)]
    stem_sibling: bool,
    min_age_days: Option<u32>,
    min_bytes: Option<u64>,
    category: Category,
    tier: Tier,
    action: String,
    #[serde(default)]
    requires_admin: bool,
    #[serde(default)]
    docs: Vec<String>,
    example: Option<String>,
    near_miss: Option<String>,
}

/// The compiled rule library.
#[derive(Clone, Debug)]
pub struct Rules {
    rules: Vec<Rule>,
    parents: GlobSet,
    /// Rules left out because a variable they use is not set on this PC.
    pub skipped: Vec<String>,
}

impl Default for Rules {
    fn default() -> Self {
        Self::empty()
    }
}

const BUILTIN: [&str; 6] = [
    include_str!("../../../data/known-locations/windows.toml"),
    include_str!("../../../data/known-locations/caches.toml"),
    include_str!("../../../data/known-locations/containers.toml"),
    include_str!("../../../data/known-locations/games.toml"),
    include_str!("../../../data/known-locations/developer.toml"),
    include_str!("../../../data/known-locations/user.toml"),
];

/// Replaces `%VAR%` with its value; `None` if a variable is not set.
fn expand(pattern: &str, roots: &Roots) -> Option<String> {
    let mut out = String::new();
    let mut rest = pattern;
    while let Some(start) = rest.find('%') {
        out.push_str(&rest[..start]);
        let after = &rest[start + 1..];
        let end = after.find('%')?;
        out.push_str(roots.var(&after[..end])?);
        rest = &after[end + 1..];
    }
    out.push_str(rest);
    Some(out)
}

/// Lower case with forward slashes and no trailing separator, the form globs match on.
fn norm(p: &str) -> String {
    p.replace('\\', "/").trim_end_matches('/').to_lowercase()
}

fn glob(pattern: &str) -> Result<Glob, globset::Error> {
    GlobBuilder::new(pattern)
        .case_insensitive(true)
        .literal_separator(true)
        .backslash_escape(false)
        .build()
}

fn parse_action(s: &str) -> Option<Action> {
    Some(match s {
        "quarantine" => Action::Quarantine,
        "freeUpSpace" => Action::FreeUpSpace,
        "uninstaller" => Action::Uninstaller,
        "showInExplorer" => Action::ShowInExplorer,
        "explainOnly" => Action::ExplainOnly,
        other => {
            let tool = other.strip_prefix("windowsTool:")?;
            Action::WindowsTool(
                serde_json::from_value(serde_json::Value::String(tool.to_string())).ok()?,
            )
        }
    })
}

impl Rules {
    /// A library without rules.
    pub fn empty() -> Self {
        Self {
            rules: vec![],
            parents: GlobSet::empty(),
            skipped: vec![],
        }
    }

    /// The built-in library from `data/known-locations/`.
    pub fn builtin(roots: &Roots) -> Result<Self, RuleError> {
        Self::from_toml(&BUILTIN, roots)
    }

    /// Compiles rules from TOML sources, expanding `%VAR%` with this PC's values.
    pub fn from_toml(sources: &[&str], roots: &Roots) -> Result<Self, RuleError> {
        let mut rules = vec![];
        let mut parents = GlobSetBuilder::new();
        let mut ids = HashSet::new();
        let mut skipped = vec![];
        for src in sources {
            let file: RuleFile = toml::from_str(src).map_err(|e| RuleError::Toml(e.to_string()))?;
            for raw in file.rule {
                if !ids.insert(raw.id.clone()) {
                    return Err(RuleError::Duplicate(raw.id));
                }
                let (Some(parent), Some(name)) =
                    (expand(&raw.parent, roots), expand(&raw.name, roots))
                else {
                    skipped.push(raw.id);
                    continue;
                };
                let (parent, name) = (norm(&parent), norm(&name));
                let pattern_err = |e: globset::Error| RuleError::Pattern {
                    id: raw.id.clone(),
                    message: e.to_string(),
                };
                let parent_glob = glob(&parent).map_err(pattern_err)?;
                let name_glob = glob(&name).map_err(pattern_err)?.compile_matcher();
                let action = parse_action(&raw.action).ok_or_else(|| RuleError::Action {
                    id: raw.id.clone(),
                    action: raw.action.clone(),
                })?;
                let sibling_glob = match &raw.sibling {
                    Some(sib) => Some(
                        glob(&sib.to_lowercase())
                            .map_err(pattern_err)?
                            .compile_matcher(),
                    ),
                    None => None,
                };
                let specificity = format!("{parent}/{name}")
                    .chars()
                    .filter(|c| !"*?[]{}".contains(*c))
                    .count();
                parents.add(parent_glob);
                rules.push(Rule {
                    id: raw.id,
                    kind: raw.kind,
                    sibling: raw.sibling.map(|s| s.to_lowercase()),
                    stem_sibling: raw.stem_sibling,
                    min_age_days: raw.min_age_days,
                    min_bytes: raw.min_bytes,
                    category: raw.category,
                    tier: raw.tier,
                    action,
                    requires_admin: raw.requires_admin,
                    docs: raw.docs,
                    example: raw.example.and_then(|e| expand(&e, roots)),
                    near_miss: raw.near_miss.and_then(|e| expand(&e, roots)),
                    name: name_glob,
                    sibling_glob,
                    specificity,
                });
            }
        }
        let parents = parents.build().map_err(|e| RuleError::Pattern {
            id: "*".into(),
            message: e.to_string(),
        })?;
        Ok(Self {
            rules,
            parents,
            skipped,
        })
    }

    /// Every rule.
    pub fn all(&self) -> &[Rule] {
        &self.rules
    }

    /// The rule at an index returned by [`Rules::match_listing`].
    pub fn get(&self, index: u16) -> Option<&Rule> {
        self.rules.get(usize::from(index))
    }

    /// The index of the rule with this ID.
    pub fn index_of(&self, id: &str) -> Option<u16> {
        self.rules
            .iter()
            .position(|r| r.id == id)
            .and_then(|i| u16::try_from(i).ok())
    }

    /// For each entry of the listing of `dir`, the index of the winning rule, if any.
    pub fn match_listing(&self, dir: &str, entries: &[DirEntry], now: i64) -> Vec<Option<u16>> {
        let mut out = vec![None; entries.len()];
        if self.rules.is_empty() {
            return out;
        }
        let candidates = self.parents.matches(norm(dir));
        if candidates.is_empty() {
            return out;
        }
        let names: HashSet<String> = entries.iter().map(|e| e.name.to_lowercase()).collect();
        for (slot, e) in out.iter_mut().zip(entries) {
            if e.is_link() {
                continue;
            }
            let lower = e.name.to_lowercase();
            let mut best: Option<(usize, Tier, u16)> = None;
            for &ri in &candidates {
                let r = &self.rules[ri];
                let kind_ok = match r.kind {
                    RuleKind::Dir => e.is_dir,
                    RuleKind::File => !e.is_dir,
                    RuleKind::Any => true,
                };
                if !kind_ok || !r.name.is_match(&lower) {
                    continue;
                }
                if r.sibling_glob
                    .as_ref()
                    .is_some_and(|g| !names.iter().any(|n| g.is_match(n)))
                {
                    continue;
                }
                if r.stem_sibling && !names.contains(crate::roots::split_ext(&lower).0) {
                    continue;
                }
                if r.min_age_days
                    .is_some_and(|d| now - filetime_to_unix(e.modified) < i64::from(d) * 86_400)
                {
                    continue;
                }
                if r.min_bytes.is_some_and(|b| !e.is_dir && e.logical < b) {
                    continue;
                }
                let Ok(idx) = u16::try_from(ri) else { continue };
                let key = (r.specificity, r.tier);
                if best.is_none_or(|(s, t, _)| key > (s, t)) {
                    best = Some((r.specificity, r.tier, idx));
                }
            }
            *slot = best.map(|(_, _, i)| i);
        }
        out
    }
}
