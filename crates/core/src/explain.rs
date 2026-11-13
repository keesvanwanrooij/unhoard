//! What an item is, worked out from where it lies and what it holds.
//!
//! Every item gets an explanation. A known-locations rule gives a written one; for
//! everything else the UI builds a sentence from the facts collected here: the place the
//! item lies in (Downloads, an app's data folder, Program Files, …), the app that place
//! belongs to, the type of a file from its extension, and the kind of content that fills
//! most of a folder. Nothing here reads a file: it only looks at the path and the totals
//! the scan already has.

use serde::{Deserialize, Serialize};

use crate::roots::{Roots, is_within, split_ext};
use crate::tree::{Category, NodeId, NodeKind, Tree};

/// The place an item lies in.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Place {
    /// The user's Downloads folder.
    Downloads,
    /// The user's Desktop.
    Desktop,
    /// The user's Documents.
    Documents,
    /// The user's Pictures.
    Pictures,
    /// The user's Videos.
    Videos,
    /// The user's Music.
    Music,
    /// A cloud sync folder, outside its Desktop, Documents and media folders.
    Cloud,
    /// An app's data in `AppData` (Local, LocalLow or Roaming).
    AppData,
    /// A program installed for this user only (`AppData\Local\Programs`).
    UserPrograms,
    /// The data of a Microsoft Store app (`AppData\Local\Packages`).
    StoreApp,
    /// The user's temporary files.
    Temp,
    /// Programs installed for every user.
    ProgramFiles,
    /// Data that programs share between users.
    ProgramData,
    /// Windows itself.
    Windows,
    /// The user's own profile folder, outside the places above.
    Profile,
    /// The profile of another user of this PC.
    OtherUser,
    /// The Recycle Bin.
    RecycleBin,
    /// Unhoard's quarantine.
    Quarantine,
    /// Anywhere else on a drive.
    #[default]
    Drive,
}

/// The type of a file, from its extension.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FileType {
    /// A video.
    Video,
    /// A photo or other image.
    Photo,
    /// A camera's raw photo.
    RawPhoto,
    /// Music or another sound recording.
    Audio,
    /// A compressed archive.
    Archive,
    /// A copy of a whole disc or drive.
    DiskImage,
    /// The disk of a virtual machine or container.
    VirtualDisk,
    /// An installer.
    Installer,
    /// A program.
    Program,
    /// A code library that programs load.
    Library,
    /// A text document.
    Document,
    /// A spreadsheet.
    Spreadsheet,
    /// A presentation.
    Presentation,
    /// A PDF.
    Pdf,
    /// An e-book.
    Ebook,
    /// Plain text.
    Text,
    /// Source code.
    Code,
    /// Structured data.
    Data,
    /// A database.
    Database,
    /// A log.
    Log,
    /// A backup copy.
    Backup,
    /// A memory dump of a crash.
    Dump,
    /// The weights of an AI model.
    AiModel,
    /// Packed game data.
    GameData,
    /// A font.
    Font,
    /// A mailbox.
    Mail,
    /// A design or editing project.
    Design,
    /// A download that did not finish.
    Partial,
    /// A temporary file.
    Temporary,
    /// A shortcut.
    Shortcut,
    /// A Windows system file or driver.
    System,
}

/// The file type that `name`'s extension stands for, if it is a known one.
pub fn file_type(name: &str) -> Option<FileType> {
    let (_, ext) = split_ext(name);
    let ext = ext.trim_start_matches('.').to_ascii_lowercase();
    const TABLE: [(FileType, &[&str]); 31] = [
        (
            FileType::Video,
            &[
                "mp4", "mov", "avi", "mkv", "wmv", "m4v", "3gp", "mts", "m2ts", "webm", "flv",
                "mpg", "mpeg", "ts", "vob",
            ],
        ),
        (
            FileType::Photo,
            &[
                "jpg", "jpeg", "png", "gif", "heic", "heif", "webp", "bmp", "tif", "tiff", "avif",
                "svg", "ico",
            ],
        ),
        (
            FileType::RawPhoto,
            &[
                "raw", "cr2", "cr3", "nef", "arw", "dng", "orf", "rw2", "raf",
            ],
        ),
        (
            FileType::Audio,
            &[
                "mp3", "flac", "wav", "m4a", "aac", "ogg", "opus", "wma", "aiff", "mid",
            ],
        ),
        (
            FileType::Archive,
            &[
                "zip", "7z", "rar", "tar", "gz", "tgz", "bz2", "xz", "zst", "cab",
            ],
        ),
        (
            FileType::DiskImage,
            &["iso", "img", "dmg", "wim", "esd", "nrg"],
        ),
        (
            FileType::VirtualDisk,
            &["vhd", "vhdx", "vmdk", "vdi", "qcow2", "avhdx"],
        ),
        (
            FileType::Installer,
            &["msi", "msix", "msixbundle", "appx", "appxbundle", "msp"],
        ),
        (FileType::Program, &["exe", "com", "bat", "cmd", "ps1"]),
        (FileType::Library, &["dll", "ocx", "so", "lib", "pyd"]),
        (FileType::Document, &["doc", "docx", "odt", "rtf", "pages"]),
        (
            FileType::Spreadsheet,
            &["xls", "xlsx", "xlsm", "ods", "numbers"],
        ),
        (FileType::Presentation, &["ppt", "pptx", "odp", "key"]),
        (FileType::Pdf, &["pdf"]),
        (FileType::Ebook, &["epub", "mobi", "azw3"]),
        (FileType::Text, &["txt", "md", "nfo"]),
        (
            FileType::Code,
            &[
                "rs", "py", "js", "ts", "tsx", "jsx", "c", "cpp", "h", "cs", "java", "go", "html",
                "css", "php", "rb", "sh", "kt", "swift",
            ],
        ),
        (
            FileType::Data,
            &[
                "json", "xml", "csv", "yaml", "yml", "toml", "parquet", "ndjson",
            ],
        ),
        (
            FileType::Database,
            &[
                "db", "sqlite", "sqlite3", "mdb", "accdb", "mdf", "ldf", "edb",
            ],
        ),
        (FileType::Log, &["log", "etl", "evtx"]),
        (FileType::Backup, &["bak", "old", "bkp", "tib", "mrimg"]),
        (FileType::Dump, &["dmp", "mdmp", "hdmp"]),
        (
            FileType::AiModel,
            &["gguf", "safetensors", "ckpt", "onnx", "pt", "pth", "tflite"],
        ),
        (
            FileType::GameData,
            &[
                "pak", "vpk", "bsa", "ucas", "utoc", "wad", "assets", "bundle",
            ],
        ),
        (FileType::Font, &["ttf", "otf", "woff", "woff2", "ttc"]),
        (FileType::Mail, &["pst", "ost", "mbox", "eml", "msg"]),
        (
            FileType::Design,
            &[
                "psd", "psb", "ai", "indd", "xcf", "blend", "prproj", "aep", "drp", "fig",
                "sketch", "afphoto", "kra",
            ],
        ),
        (
            FileType::Partial,
            &["crdownload", "part", "partial", "download"],
        ),
        (FileType::Temporary, &["tmp", "temp"]),
        (FileType::Shortcut, &["lnk", "url"]),
        (FileType::System, &["sys", "drv", "mui", "cat", "inf"]),
    ];
    TABLE
        .iter()
        .find(|(_, exts)| exts.contains(&ext.as_str()))
        .map(|(t, _)| *t)
}

/// The kind of content that fills most of a folder.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Mix {
    /// The kind of content.
    pub category: Category,
    /// Its share of the folder's bytes on disk, in percent.
    pub percent: u8,
}

/// The facts an explanation is built from.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Explain {
    /// The place the item lies in.
    pub place: Place,
    /// The app, program or user that place belongs to, from the folder name.
    pub owner: Option<String>,
    /// True when the item is the owner's own folder, not something inside it.
    pub owner_root: bool,
    /// The type of a file.
    pub file_type: Option<FileType>,
    /// What a folder mostly holds, when one kind of content fills at least half of it.
    pub mix: Option<Mix>,
}

/// The path components of `path` below `base`, or `None` when it is not inside `base`.
fn below<'a>(path: &'a str, base: &str) -> Option<Vec<&'a str>> {
    if !is_within(path, base) {
        return None;
    }
    let rest = path.get(base.trim_end_matches('\\').len()..).unwrap_or("");
    Some(rest.split('\\').filter(|c| !c.is_empty()).collect())
}

/// The user folder a top-level name stands for, in English and in the Dutch names
/// OneDrive gives its copies.
fn user_folder(name: &str) -> Option<Place> {
    Some(match name.to_lowercase().as_str() {
        "desktop" | "bureaublad" => Place::Desktop,
        "documents" | "documenten" => Place::Documents,
        "pictures" | "afbeeldingen" => Place::Pictures,
        "videos" | "video's" => Place::Videos,
        "music" | "muziek" => Place::Music,
        "downloads" => Place::Downloads,
        _ => return None,
    })
}

/// A Store package folder is `Name_publisherId`; the name is what the user knows.
fn package_name(folder: &str) -> &str {
    folder.split('_').next().unwrap_or(folder)
}

/// Where a path lies: the place, the owner of that place, and whether the path is the
/// owner's own folder.
struct Location {
    place: Place,
    owner: Option<String>,
    owner_root: bool,
}

impl Location {
    /// A place that belongs to nobody in particular.
    fn plain(place: Place) -> Self {
        Self {
            place,
            owner: None,
            owner_root: false,
        }
    }

    /// A place whose owner is the component at `index` of the path below the place.
    fn owned(place: Place, parts: &[&str], index: usize, name: fn(&str) -> &str) -> Self {
        Self {
            place,
            owner: parts.get(index).map(|s| name(s).to_string()),
            owner_root: parts.len() == index + 1,
        }
    }
}

/// The folder name as it is: the owner's name for every place but a Store app's.
fn whole(name: &str) -> &str {
    name
}

/// Finds where `path` lies. The most specific place wins: Temp before AppData, AppData
/// and the user folders before the profile.
fn locate(path: &str, roots: &Roots) -> Location {
    let p = path.replace('/', "\\");
    let p = p.trim_end_matches('\\');
    if roots.in_quarantine(p) {
        return Location::plain(Place::Quarantine);
    }
    if p.get(3..)
        .and_then(|rest| rest.split('\\').next())
        .is_some_and(|c| c.eq_ignore_ascii_case("$recycle.bin"))
    {
        return Location::plain(Place::RecycleBin);
    }
    if roots.var("TEMP").is_some_and(|t| is_within(p, t)) {
        return Location::plain(Place::Temp);
    }
    if roots.windows().is_some_and(|w| is_within(p, w)) {
        return Location::plain(Place::Windows);
    }
    for pf in roots.program_files() {
        if let Some(parts) = below(p, pf) {
            return Location::owned(Place::ProgramFiles, &parts, 0, whole);
        }
    }
    if let Some(parts) = roots.program_data().and_then(|pd| below(p, pd)) {
        return Location::owned(Place::ProgramData, &parts, 0, whole);
    }
    if roots.downloads.as_deref().is_some_and(|d| is_within(p, d)) {
        return Location::plain(Place::Downloads);
    }
    if let Some(parts) = roots.sync_root_of(p).and_then(|sr| below(p, sr)) {
        let place = parts.first().and_then(|n| user_folder(n));
        return Location::plain(place.unwrap_or(Place::Cloud));
    }
    if let Some(parts) = roots.var("LOCALAPPDATA").and_then(|l| below(p, l)) {
        return match parts.first().map(|s| s.to_lowercase()).as_deref() {
            Some("programs") => Location::owned(Place::UserPrograms, &parts, 1, whole),
            Some("packages") => Location::owned(Place::StoreApp, &parts, 1, package_name),
            _ => Location::owned(Place::AppData, &parts, 0, whole),
        };
    }
    if let Some(parts) = roots.var("APPDATA").and_then(|a| below(p, a)) {
        return Location::owned(Place::AppData, &parts, 0, whole);
    }
    if let Some(profile) = roots.profile() {
        if let Some(parts) = below(p, &format!("{profile}\\AppData\\LocalLow")) {
            return Location::owned(Place::AppData, &parts, 0, whole);
        }
        if let Some(parts) = below(p, profile) {
            let place = parts.first().and_then(|n| user_folder(n));
            return Location::plain(place.unwrap_or(Place::Profile));
        }
    }
    if let Some(parts) = roots.users().and_then(|u| below(p, &u))
        && !parts.is_empty()
    {
        return Location::owned(Place::OtherUser, &parts, 0, whole);
    }
    Location::plain(Place::Drive)
}

/// The place `path` lies in, and the app, program or user that place belongs to.
pub fn place_of(path: &str, roots: &Roots) -> (Place, Option<String>) {
    let l = locate(path, roots);
    (l.place, l.owner)
}

/// The kind of content that fills at least half of the folder `id`, if one does. "Other"
/// says nothing about the content, so it never counts as a kind.
fn mix_of(tree: &Tree, id: NodeId) -> Option<Mix> {
    let bytes = tree.category_bytes(id)?;
    let total: u64 = bytes.iter().sum();
    if total == 0 {
        return None;
    }
    let (category, most) = Category::ALL
        .into_iter()
        .map(|c| (c, bytes[c.index()]))
        .filter(|(c, _)| !matches!(c, Category::Other | Category::Hidden))
        .max_by_key(|(_, b)| *b)?;
    let percent = u8::try_from(most.saturating_mul(100) / total).unwrap_or(100);
    (percent >= 50).then_some(Mix { category, percent })
}

/// The facts that explain the node `id`.
pub fn explain(tree: &Tree, id: NodeId, roots: &Roots) -> Explain {
    let Some(node) = tree.get(id) else {
        return Explain::default();
    };
    let at = locate(&tree.path(id), roots);
    let is_file = node.kind == NodeKind::File;
    Explain {
        place: at.place,
        owner: at.owner,
        owner_root: at.owner_root,
        file_type: is_file.then(|| file_type(&node.name)).flatten(),
        mix: (node.kind == NodeKind::Folder)
            .then(|| mix_of(tree, id))
            .flatten(),
    }
}
