//! Steam's records: `libraryfolders.vdf` lists the libraries, and each library's
//! `steamapps\appmanifest_<id>.acf` describes one installed game. Both use Valve's
//! KeyValues text format: quoted keys and values, and braces for nested objects.

use crate::knowledge::SteamGame;
use crate::roots::join;

/// The largest file this parser accepts.
const MAX_TEXT: usize = 1 << 20;
/// How deep objects may nest.
const MAX_DEPTH: usize = 16;

/// A KeyValues value: a string, or an object of key-value pairs in file order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Vdf {
    /// A quoted string.
    Str(String),
    /// A braced object.
    Obj(Vec<(String, Vdf)>),
}

impl Vdf {
    /// The value of `key` in an object, compared without case as Steam does.
    pub fn get(&self, key: &str) -> Option<&Vdf> {
        match self {
            Vdf::Obj(pairs) => pairs
                .iter()
                .find(|(k, _)| k.eq_ignore_ascii_case(key))
                .map(|(_, v)| v),
            Vdf::Str(_) => None,
        }
    }

    /// The string, if this is one.
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Vdf::Str(s) => Some(s),
            Vdf::Obj(_) => None,
        }
    }
}

enum Token {
    Str(String),
    Open,
    Close,
}

fn tokens(text: &str) -> Option<Vec<Token>> {
    let mut out = vec![];
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '{' => out.push(Token::Open),
            '}' => out.push(Token::Close),
            '"' => {
                let mut s = String::new();
                loop {
                    match chars.next()? {
                        '"' => break,
                        '\\' => match chars.next()? {
                            'n' => s.push('\n'),
                            't' => s.push('\t'),
                            other => s.push(other),
                        },
                        other => s.push(other),
                    }
                }
                out.push(Token::Str(s));
            }
            '/' if chars.peek() == Some(&'/') => {
                for c in chars.by_ref() {
                    if c == '\n' {
                        break;
                    }
                }
            }
            c if c.is_whitespace() => {}
            // Conditions such as [$WIN32] and unquoted words are not used by these files.
            _ => return None,
        }
    }
    Some(out)
}

fn object(tokens: &[Token], pos: &mut usize, depth: usize) -> Option<Vec<(String, Vdf)>> {
    if depth > MAX_DEPTH {
        return None;
    }
    let mut pairs = vec![];
    while let Some(t) = tokens.get(*pos) {
        match t {
            Token::Close => {
                *pos += 1;
                return Some(pairs);
            }
            Token::Str(k) => {
                *pos += 1;
                let value = match tokens.get(*pos)? {
                    Token::Str(v) => {
                        *pos += 1;
                        Vdf::Str(v.clone())
                    }
                    Token::Open => {
                        *pos += 1;
                        Vdf::Obj(object(tokens, pos, depth + 1)?)
                    }
                    Token::Close => return None,
                };
                pairs.push((k.clone(), value));
            }
            Token::Open => return None,
        }
    }
    // The top level ends at the end of the text; nested objects need their brace.
    (depth == 0).then_some(pairs)
}

/// Parses KeyValues text into its top-level object, or `None` if it is malformed.
pub fn parse_vdf(text: &str) -> Option<Vdf> {
    if text.len() > MAX_TEXT {
        return None;
    }
    let tokens = tokens(text)?;
    let mut pos = 0;
    let pairs = object(&tokens, &mut pos, 0)?;
    (pos == tokens.len()).then_some(Vdf::Obj(pairs))
}

/// The library folders listed in `libraryfolders.vdf`.
pub fn library_paths(libraryfolders: &str) -> Vec<String> {
    let Some(root) = parse_vdf(libraryfolders) else {
        return vec![];
    };
    let Some(Vdf::Obj(entries)) = root.get("libraryfolders") else {
        return vec![];
    };
    entries
        .iter()
        .filter_map(|(_, v)| v.get("path")?.as_str())
        .filter(|p| !p.is_empty())
        .map(str::to_string)
        .collect()
}

/// The game an `appmanifest_<id>.acf` describes, in the library at `library`.
pub fn parse_appmanifest(text: &str, library: &str) -> Option<SteamGame> {
    let root = parse_vdf(text)?;
    let state = root.get("AppState")?;
    let field = |k: &str| state.get(k).and_then(Vdf::as_str);
    let install = field("installdir")?;
    // The install folder is one name inside steamapps\common, never a path.
    if install.is_empty() || install.contains(['\\', '/', ':']) || install == ".." {
        return None;
    }
    Some(SteamGame {
        app_id: field("appid")?.parse().ok()?,
        name: field("name").unwrap_or(install).to_string(),
        install_dir: join(&join(&join(library, "steamapps"), "common"), install),
        size_on_disk: field("SizeOnDisk")
            .and_then(|s| s.parse().ok())
            .unwrap_or(0),
        last_played: field("LastPlayed")
            .and_then(|s| s.parse::<i64>().ok())
            .filter(|t| *t > 0),
    })
}
