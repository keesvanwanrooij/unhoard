//! Known processes (F16): what a running program or startup entry is, found by its
//! executable name. The list lives in `data/known-processes.toml`; the texts in the UI's
//! language files under `processes.<id>`.

use std::sync::OnceLock;

use serde::Deserialize;

/// One known program, with the executable names it runs as.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct KnownProcess {
    /// A stable ID; it picks the texts.
    pub id: String,
    /// Executable names in lower case, for example `chrome.exe`.
    pub exe: Vec<String>,
    /// The group: `windows`, `security`, `driver`, `app`, `browser`, `cloud`, `chat`,
    /// `media`, `game` or `dev`.
    pub kind: String,
}

#[derive(Deserialize)]
struct File {
    process: Vec<KnownProcess>,
}

/// Every known process. The list is built in, so a broken file fails the tests, not the
/// app; a list that does not parse is empty.
pub fn all() -> &'static [KnownProcess] {
    static LIST: OnceLock<Vec<KnownProcess>> = OnceLock::new();
    LIST.get_or_init(|| {
        toml::from_str::<File>(include_str!("../../../data/known-processes.toml"))
            .map(|f| f.process)
            .unwrap_or_default()
    })
}

/// The known process for an executable name or a path to one; the case does not matter.
pub fn known(exe: &str) -> Option<&'static KnownProcess> {
    let name = exe
        .rsplit(['\\', '/'])
        .next()
        .unwrap_or(exe)
        .trim()
        .to_lowercase();
    if name.is_empty() {
        return None;
    }
    all().iter().find(|p| p.exe.contains(&name))
}
