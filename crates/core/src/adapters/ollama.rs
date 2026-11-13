//! Ollama's model store: every model is a manifest file under `models\manifests` that
//! lists the blobs (`models\blobs\sha256-<hex>`) it is built from. Models share blobs, so
//! a model's own size is only the blobs no other model uses.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::roots::join;

/// The largest manifest this parser accepts.
const MAX_MANIFEST: usize = 1 << 20;

/// One manifest file as the platform layer read it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ManifestFile {
    /// Its path below `models\manifests`, with forward slashes:
    /// `registry.ollama.ai/library/llama3/latest`.
    pub relative: String,
    /// Its last write time, in seconds since 1970 (when the model was pulled).
    pub modified: i64,
    /// Its contents.
    pub json: String,
}

/// One model.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OllamaModel {
    /// The name `ollama rm` takes, such as `llama3:latest`.
    pub name: String,
    /// The manifest file.
    pub manifest: String,
    /// Every blob the model uses.
    pub blobs: Vec<String>,
    /// The blobs no other model uses: what removing the model frees.
    pub unique_blobs: Vec<String>,
    /// The size of all its blobs.
    pub bytes: u64,
    /// The size of its unique blobs.
    pub unique_bytes: u64,
    /// When it was pulled, in seconds since 1970.
    pub modified: i64,
}

#[derive(Deserialize)]
struct Layer {
    digest: String,
    size: u64,
}

#[derive(Deserialize)]
struct Manifest {
    config: Option<Layer>,
    #[serde(default)]
    layers: Vec<Layer>,
}

/// The name `ollama` shows for a manifest path: `library` models drop the namespace.
pub fn model_name(relative: &str) -> Option<String> {
    let parts: Vec<&str> = relative.split('/').filter(|p| !p.is_empty()).collect();
    let [host, namespace, model, tag] = parts.as_slice() else {
        return None;
    };
    Some(match (*host, *namespace) {
        ("registry.ollama.ai", "library") => format!("{model}:{tag}"),
        ("registry.ollama.ai", ns) => format!("{ns}/{model}:{tag}"),
        (h, ns) => format!("{h}/{ns}/{model}:{tag}"),
    })
}

/// The blob file name for a digest, or `None` unless it is `sha256:` and 64 hex digits.
fn blob_name(digest: &str) -> Option<String> {
    let hex = digest.strip_prefix("sha256:")?;
    (hex.len() == 64 && hex.bytes().all(|b| b.is_ascii_hexdigit())).then(|| format!("sha256-{hex}"))
}

/// A model's name, its manifest file, and its blobs with their sizes.
type Parsed<'a> = (String, &'a ManifestFile, Vec<(String, u64)>);

/// The models in the store at `models_dir`, from its manifests. Malformed manifests and
/// invalid digests are skipped, so a hostile file can never point outside the store.
pub fn models(models_dir: &str, manifests: &[ManifestFile]) -> Vec<OllamaModel> {
    let blobs_dir = join(models_dir, "blobs");
    let parsed: Vec<Parsed<'_>> = manifests
        .iter()
        .filter(|m| m.json.len() <= MAX_MANIFEST)
        .filter_map(|m| {
            let name = model_name(&m.relative)?;
            let manifest: Manifest = serde_json::from_str(&m.json).ok()?;
            let layers: Option<Vec<(String, u64)>> = manifest
                .config
                .iter()
                .chain(&manifest.layers)
                .map(|l| Some((join(&blobs_dir, &blob_name(&l.digest)?), l.size)))
                .collect();
            Some((name, m, layers?))
        })
        .collect();
    let mut users: HashMap<&str, usize> = HashMap::new();
    for (_, _, layers) in &parsed {
        let mut seen: Vec<&str> = layers.iter().map(|(b, _)| b.as_str()).collect();
        seen.sort_unstable();
        seen.dedup();
        for b in seen {
            *users.entry(b).or_default() += 1;
        }
    }
    let mut out: Vec<OllamaModel> = parsed
        .iter()
        .map(|(name, m, layers)| {
            let unique: Vec<&(String, u64)> = layers
                .iter()
                .filter(|(b, _)| users.get(b.as_str()) == Some(&1))
                .collect();
            let manifest = m
                .relative
                .split('/')
                .fold(join(models_dir, "manifests"), |p, part| join(&p, part));
            OllamaModel {
                name: name.clone(),
                manifest,
                blobs: layers.iter().map(|(b, _)| b.clone()).collect(),
                unique_blobs: unique.iter().map(|(b, _)| b.clone()).collect(),
                bytes: layers.iter().map(|(_, s)| s).sum(),
                unique_bytes: unique.iter().map(|(_, s)| s).sum(),
                modified: m.modified,
            }
        })
        .collect();
    out.sort_by(|a, b| b.bytes.cmp(&a.bytes).then_with(|| a.name.cmp(&b.name)));
    out
}
