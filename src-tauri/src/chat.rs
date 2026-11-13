//! The AI chat (F8): runs a conversation with a local model through Ollama, answers the
//! model's tool calls from the current scan, the storage history and the library, and
//! streams the reply to the webview.
//!
//! Tools only read. Paths go to the model as they are, because the model runs on this PC;
//! they are never logged. The conversation lives in memory only.

use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use unhoard_core::chat::{ToolCall, cap, parse_call, system_prompt, tools_schema};
use unhoard_core::classify::describe;
use unhoard_core::tree::{NodeId, NodeKind, Tree};
use unhoard_platform::ollama::{self, ChatError};

use crate::service::Service;

/// The most tool rounds in one answer, so a confused model cannot loop.
const MAX_ROUNDS: usize = 6;
/// The most messages and characters a conversation may send.
const MAX_MESSAGES: usize = 40;
const MAX_CHARS: usize = 8_000;
/// The model Unhoard recommends: tested for Dutch and tool calls ([Decisions]).
pub const RECOMMENDED: &str = "qwen3:8b";

/// One message of the conversation, as the webview keeps it.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Message {
    /// `user` or `assistant`.
    pub role: String,
    /// The text.
    pub content: String,
}

/// What the webview hears while an answer forms.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum ChatEvent {
    /// More text of the answer.
    Delta {
        /// The new text.
        text: String,
    },
    /// The model asked a tool.
    Tool {
        /// The tool's name.
        name: String,
    },
    /// The answer is complete.
    Done,
}

/// A model Ollama has installed.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelInfo {
    /// Its name, for example `qwen3:8b`.
    pub name: String,
    /// Its size on disk.
    pub size: u64,
    /// True for the model Unhoard recommends.
    pub recommended: bool,
}

/// The installed models.
pub fn models() -> Result<Vec<ModelInfo>, ChatError> {
    let text = ollama::tags()?;
    let v: Value = serde_json::from_str(&text).map_err(|_| ChatError::BadReply)?;
    Ok(v["models"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|m| {
                    let name = m["name"].as_str()?.to_string();
                    Some(ModelInfo {
                        recommended: name == RECOMMENDED,
                        size: m["size"].as_u64().unwrap_or(0),
                        name,
                    })
                })
                .take(200)
                .collect()
        })
        .unwrap_or_default())
}

/// The UI texts in both languages, for rule titles and explanations the model reads.
fn locale(lang: &str) -> &'static Value {
    static EN: OnceLock<Value> = OnceLock::new();
    static NL: OnceLock<Value> = OnceLock::new();
    let (cell, text) = if lang == "nl" {
        (&NL, include_str!("../../src/i18n/nl.json"))
    } else {
        (&EN, include_str!("../../src/i18n/en.json"))
    };
    cell.get_or_init(|| serde_json::from_str(text).unwrap_or(Value::Null))
}

/// A size as text, the way the app shows it.
fn size_text(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["bytes", "KB", "MB", "GB", "TB"];
    let mut v = bytes as f64;
    let mut u = 0;
    while v >= 1024.0 && u < UNITS.len() - 1 {
        v /= 1024.0;
        u += 1;
    }
    if u == 0 {
        format!("{bytes} bytes")
    } else {
        format!("{v:.1} {}", UNITS[u])
    }
}

/// A short description of one node for the model.
fn item(tree: &Tree, id: NodeId, svc: &Service, lang: &str) -> Value {
    let ctx = svc.context();
    let Some(n) = tree.get(id) else {
        return Value::Null;
    };
    let rule = n.rule.and_then(|r| ctx.rules.get(r));
    let title = rule.and_then(|r| locale(lang)["knownLocations"][&r.id]["title"].as_str());
    json!({
        "id": id.0,
        "name": n.name.as_ref(),
        "path": tree.path(id),
        "kind": match n.kind { NodeKind::Folder => "folder", NodeKind::File => "file", NodeKind::Link => "link" },
        "size": size_text(n.on_disk),
        "bytes": n.on_disk,
        "category": format!("{:?}", tree.dominant_category(id)).to_lowercase(),
        "what": title,
    })
}

/// Runs one validated tool call and returns its result for the model.
pub fn run_tool(svc: &Service, call: &ToolCall, lang: &str) -> Value {
    let Ok(session) = svc.current() else {
        return json!({"error": "no scan yet; ask the user to scan a drive first"});
    };
    let tree_guard = session
        .tree
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let tree = &*tree_guard;
    let ctx = svc.context();
    let find = |path: &str| tree.find(path);
    let top = |id: NodeId, n: usize| -> Vec<Value> {
        tree.top_children(id, n, false)
            .0
            .into_iter()
            .map(|c| item(tree, c, svc, lang))
            .collect()
    };
    match call {
        ToolCall::Overview => {
            let root = tree.root();
            let scanned = tree.get(root).map(|r| r.on_disk).unwrap_or(0);
            let volume = session.volume.as_ref();
            json!({
                "drive": session.root,
                "size": volume.map(|v| size_text(v.total_bytes)),
                "used": volume.map(|v| size_text(v.total_bytes.saturating_sub(v.free_bytes))),
                "free": volume.map(|v| size_text(v.free_bytes)),
                "scanned": size_text(scanned),
                "hidden": volume.filter(|_| session.drive_scan).map(|v| size_text(unhoard_core::hidden::hidden_bytes(v, scanned, 0))),
                "largest": top(root, 10),
            })
        }
        ToolCall::Children { path } => match find(path) {
            Some(id) => json!({"path": tree.path(id), "children": top(id, 20)}),
            None => json!({"error": "not in the scan"}),
        },
        ToolCall::Explain { path } => match find(path) {
            Some(id) => {
                let d = describe(tree, id, &ctx);
                let texts = d
                    .rule_id
                    .as_ref()
                    .map(|r| &locale(lang)["knownLocations"][r]);
                json!({
                    "item": item(tree, id, svc, lang),
                    "title": texts.and_then(|t| t["title"].as_str()),
                    "explanation": texts.and_then(|t| t["explain"].as_str()),
                    "reason": texts.and_then(|t| t["reason"].as_str()),
                    "whatToDo": texts.and_then(|t| t["action"].as_str()),
                    "tier": d.tier.map(|t| format!("{t:?}")),
                    "protected": d.protected.is_some(),
                    "place": d.explain.place,
                    "owner": d.explain.owner,
                    "fileType": d.explain.file_type,
                    "leftover": d.leftover,
                    "program": d.program.as_ref().map(|p| &p.name),
                    "lastUse": d.last_use.map(|u| unhoard_core::time::fmt_iso(u.at)),
                    "canQuarantine": d.actions.contains(&unhoard_core::classify::Action::Quarantine),
                })
            }
            None => json!({"error": "not in the scan"}),
        },
        ToolCall::Suggestions { group } => {
            drop(tree_guard);
            let Ok((_, list)) = svc.suggestions(2000) else {
                return json!({"error": "no scan yet"});
            };
            let key = |s: &unhoard_core::view::Suggestion| {
                let d = &s.description;
                match (&d.steam, &d.rule_id, &d.program, &d.leftover) {
                    (Some(unhoard_core::knowledge::SteamInfo::Game { .. }), ..) => {
                        "steam:game".to_string()
                    }
                    (Some(_), ..) => "steam:leftover".to_string(),
                    (None, Some(r), ..) => format!("rule:{r}"),
                    (None, None, Some(_), _) => "program".to_string(),
                    (None, None, None, Some(_)) => "leftover".to_string(),
                    _ => "large".to_string(),
                }
            };
            match group {
                None => {
                    let mut groups: Vec<(String, usize, u64)> = vec![];
                    for s in &list {
                        let k = key(s);
                        match groups.iter_mut().find(|g| g.0 == k) {
                            Some(g) => {
                                g.1 += 1;
                                g.2 += s.node.on_disk;
                            }
                            None => groups.push((k, 1, s.node.on_disk)),
                        }
                    }
                    groups.sort_by_key(|g| std::cmp::Reverse(g.2));
                    let texts = locale(lang);
                    json!({"groups": groups.iter().map(|(k, n, b)| json!({
                        "key": k,
                        "title": k.strip_prefix("rule:").and_then(|r| texts["knownLocations"][r]["title"].as_str()),
                        "count": n,
                        "size": size_text(*b),
                    })).collect::<Vec<_>>()})
                }
                Some(g) => {
                    json!({"items": list.iter().filter(|s| key(s) == *g).take(60).map(|s| json!({
                    "id": s.node.id,
                    "name": s.node.name,
                    "path": s.path,
                    "size": size_text(s.node.on_disk),
                    "tier": format!("{:?}", s.tier),
                })).collect::<Vec<_>>()})
                }
            }
        }
        ToolCall::Growth { days } => {
            drop(tree_guard);
            let g = svc.history_growth(&session.root, *days);
            if g.from.is_none() {
                return json!({"note": "no history yet for this period; storage history fills with every scan and the daily measurement"});
            }
            json!({
                "from": g.from.map(unhoard_core::time::fmt_iso),
                "to": g.to.map(unhoard_core::time::fmt_iso),
                "changes": g.items.iter().take(25).map(|i| json!({
                    "id": i.id,
                    "name": i.name,
                    "path": i.path,
                    "change": format!("{}{}", if i.delta < 0 { "-" } else { "+" }, size_text(i.delta.unsigned_abs())),
                    "now": size_text(i.after),
                    "what": i.rule_id.as_ref().and_then(|r| locale(lang)["knownLocations"][r]["title"].as_str()),
                })).collect::<Vec<_>>(),
            })
        }
        ToolCall::Search { name } => {
            let needle = name.to_lowercase();
            let mut hits: Vec<NodeId> = tree
                .iter()
                .filter(|(_, n)| n.name.to_lowercase().contains(&needle))
                .map(|(id, _)| id)
                .collect();
            hits.sort_by_key(|id| std::cmp::Reverse(tree.get(*id).map(|n| n.on_disk).unwrap_or(0)));
            json!({"results": hits.into_iter().take(50).map(|id| item(tree, id, svc, lang)).collect::<Vec<_>>()})
        }
        ToolCall::Apps => {
            let k = &ctx.knowledge;
            let size_of = |path: &str| {
                tree.find(path)
                    .and_then(|id| tree.get(id))
                    .map(|n| n.on_disk)
            };
            let mut programs: Vec<Value> = k
                .programs
                .iter()
                .map(|p| {
                    let bytes = size_of(&p.install_location).or(p.estimated_bytes).unwrap_or(0);
                    let used = k.last_use(&p.install_location, 0).map(|u| unhoard_core::time::fmt_iso(u.at));
                    json!({"name": p.name, "publisher": p.publisher, "size": size_text(bytes), "bytes": bytes, "lastUsed": used})
                })
                .collect();
            programs.sort_by_key(|p| std::cmp::Reverse(p["bytes"].as_u64().unwrap_or(0)));
            let games: Vec<Value> = k
                .steam
                .iter()
                .flat_map(|l| l.games.iter())
                .map(|g| json!({"name": g.name, "lastPlayed": g.last_played.map(unhoard_core::time::fmt_iso), "size": size_of(&g.install_dir).map(size_text)}))
                .collect();
            json!({"programs": programs.into_iter().take(60).collect::<Vec<_>>(), "steamGames": games})
        }
    }
}

/// Answers the last message of `history` with `model`, calling tools as the model asks,
/// and reports the answer through `emit`. `stop` ends it early.
pub fn converse(
    svc: &Service,
    model: &str,
    history: &[Message],
    lang: &str,
    emit: &dyn Fn(ChatEvent),
    stop: &AtomicBool,
) -> Result<(), ChatError> {
    let mut messages = vec![
        json!({"role": "system", "content": system_prompt(if lang == "nl" { "Dutch" } else { "English" })}),
    ];
    for m in history.iter().rev().take(MAX_MESSAGES).rev() {
        if m.role == "user" || m.role == "assistant" {
            let content: String = m.content.chars().take(MAX_CHARS).collect();
            messages.push(json!({"role": m.role, "content": content}));
        }
    }
    let tools = tools_schema();
    for _ in 0..MAX_ROUNDS {
        let body = json!({
            "model": model,
            "messages": messages,
            "tools": tools,
            "stream": true,
            "think": false,
            "options": {"temperature": 0.3},
        });
        let bytes = serde_json::to_vec(&body).map_err(|_| ChatError::BadReply)?;
        let mut content = String::new();
        let mut calls: Vec<Value> = vec![];
        ollama::chat_stream(&bytes, &mut |line| {
            if stop.load(Ordering::SeqCst) {
                return false;
            }
            let Ok(v) = serde_json::from_str::<Value>(line) else {
                return true;
            };
            if let Some(t) = v["message"]["content"].as_str()
                && !t.is_empty()
            {
                content.push_str(t);
                emit(ChatEvent::Delta {
                    text: t.to_string(),
                });
            }
            if let Some(tc) = v["message"]["tool_calls"].as_array() {
                calls.extend(tc.iter().cloned());
            }
            true
        })?;
        if stop.load(Ordering::SeqCst) || calls.is_empty() {
            emit(ChatEvent::Done);
            return Ok(());
        }
        messages.push(json!({"role": "assistant", "content": content, "tool_calls": calls}));
        for c in &calls {
            let name = c["function"]["name"].as_str().unwrap_or("");
            emit(ChatEvent::Tool {
                name: name.to_string(),
            });
            let result = match parse_call(name, &c["function"]["arguments"]) {
                Ok(call) => cap(run_tool(svc, &call, lang)),
                Err(e) => json!({"error": e}).to_string(),
            };
            messages.push(json!({"role": "tool", "content": result, "tool_name": name}));
        }
    }
    emit(ChatEvent::Done);
    Ok(())
}
