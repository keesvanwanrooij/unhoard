//! The AI chat's tools (F8): what the model may ask, checked before anything runs.
//!
//! The model gets seven read-only tools. A call names a tool and its arguments as JSON; this
//! module validates both and turns them into a [`ToolCall`], or into an error text the model
//! reads back, never a panic (T-8.5). The app runs a valid call against the scan, the
//! history and the library, and caps the result's size.

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

/// The longest path or name a tool accepts.
const MAX_TEXT: usize = 1024;
/// The most bytes a tool result may hand the model.
pub const MAX_RESULT: usize = 16 * 1024;

/// How well a recommended model runs on this PC.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Fit {
    /// It fits in the graphics card's memory: answers come quickly.
    Fast,
    /// It runs from main memory on the processor: answers take a while.
    Slow,
    /// Even the small model barely fits; the chat will be slow and less reliable.
    TooSmall,
}

/// The model Unhoard recommends for a PC, with how well it runs and its download size.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Recommendation {
    /// The Ollama model name.
    pub model: &'static str,
    /// How well it runs.
    pub fit: Fit,
    /// Its download size in bytes, as Ollama lists it.
    pub download: u64,
}

/// The models Unhoard recommends and may ask Ollama to download, with about their download
/// size (Ollama reports the exact size once the download starts).
/// `qwen3:8b` was tested for Dutch and tool calls; `qwen3:4b` is the same family for PCs
/// with less memory ([Decisions]).
pub const MODELS: [(&str, u64); 2] = [("qwen3:8b", 5_225_388_164), ("qwen3:4b", 2_497_293_918)];

/// The model to recommend for `ram` bytes of memory and, when known, `vram` bytes of
/// graphics memory. A model needs about its download size plus 1 GB to run. The limits sit
/// just under the nominal sizes, because Windows reports a little less than is fitted (an
/// 8 GB card as 7.9 GB, 16 GB of memory as 15.8 GB).
pub fn recommend_model(ram: u64, vram: Option<u64>) -> Recommendation {
    const GB: u64 = 1 << 30;
    let vram = vram.unwrap_or(0);
    let pick = |i: usize, fit: Fit| Recommendation {
        model: MODELS[i].0,
        fit,
        download: MODELS[i].1,
    };
    if vram >= 7 * GB {
        pick(0, Fit::Fast)
    } else if vram >= 4 * GB && ram < 15 * GB {
        pick(1, Fit::Fast)
    } else if ram >= 15 * GB {
        pick(0, Fit::Slow)
    } else if ram >= 7 * GB {
        pick(1, Fit::Slow)
    } else {
        pick(1, Fit::TooSmall)
    }
}

/// True for a model Unhoard may ask Ollama to download: only the ones it recommends.
pub fn may_pull(model: &str) -> bool {
    MODELS.iter().any(|(m, _)| *m == model)
}

/// A validated tool call.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "tool")]
pub enum ToolCall {
    /// Drive totals and the top folders.
    Overview,
    /// The largest children of a folder.
    Children {
        /// The folder.
        path: String,
    },
    /// One item's explanation, tier, last use and actions.
    Explain {
        /// The item.
        path: String,
    },
    /// The suggestion groups, or the items of one group.
    Suggestions {
        /// A group key, as `suggestions` without a group returns them.
        group: Option<String>,
    },
    /// The largest growers over a period.
    Growth {
        /// 1, 7, 30 or 90.
        days: u32,
    },
    /// Items whose name contains a text.
    Search {
        /// The text.
        name: String,
    },
    /// Installed programs and Steam games.
    Apps,
}

/// The tools as Ollama's `tools` list.
pub fn tools_schema() -> Value {
    let tool = |name: &str, description: &str, params: Value| json!({"type": "function", "function": {"name": name, "description": description, "parameters": params}});
    let none = json!({"type": "object", "properties": {}});
    let path = |what: &str| json!({"type": "object", "properties": {"path": {"type": "string", "description": what}}, "required": ["path"]});
    json!([
        tool(
            "overview",
            "Size, used, free and hidden space of the scanned drive, and its largest folders. Start here for questions about a full disk.",
            none.clone()
        ),
        tool(
            "children",
            "The largest items inside a folder, with size, kind and what each is.",
            path("The folder's full path, as other tools return it.")
        ),
        tool(
            "explain",
            "What one item is: its explanation, risk tier with reason, last use and what Unhoard can do with it.",
            path("The item's full path.")
        ),
        tool(
            "suggestions",
            "What can be cleaned up: groups with count and size, or the items of one group.",
            json!({"type": "object", "properties": {"group": {"type": "string", "description": "A group key from an earlier call without a group."}}})
        ),
        tool(
            "growth",
            "What grew or shrank on the drive over a period, from the storage history.",
            json!({"type": "object", "properties": {"period": {"type": "string", "enum": ["day", "week", "month", "quarter"]}}, "required": ["period"]})
        ),
        tool(
            "search",
            "Items whose name contains a text, largest first, at most 50.",
            json!({"type": "object", "properties": {"name": {"type": "string"}}, "required": ["name"]})
        ),
        tool(
            "apps",
            "Installed programs with size and last use, and Steam games with last played.",
            none
        ),
    ])
}

/// The text argument `key` of `args`, checked for length.
fn text(args: &Value, key: &str) -> Result<String, String> {
    let v = args
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("missing argument '{key}'"))?
        .trim();
    if v.is_empty() || v.len() > MAX_TEXT || v.contains('\0') {
        return Err(format!("argument '{key}' is empty or too long"));
    }
    Ok(v.to_string())
}

/// Turns a model's call into a [`ToolCall`], or an error text for the model.
pub fn parse_call(name: &str, args: &Value) -> Result<ToolCall, String> {
    match name {
        "overview" => Ok(ToolCall::Overview),
        "children" => Ok(ToolCall::Children {
            path: text(args, "path")?,
        }),
        "explain" => Ok(ToolCall::Explain {
            path: text(args, "path")?,
        }),
        "suggestions" => Ok(ToolCall::Suggestions {
            group: args
                .get("group")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|g| !g.is_empty() && g.len() <= 200)
                .map(str::to_string),
        }),
        "growth" => {
            let days = match args.get("period").and_then(Value::as_str) {
                Some("day") => 1,
                Some("week") => 7,
                Some("month") => 30,
                Some("quarter") => 90,
                _ => return Err("period must be day, week, month or quarter".into()),
            };
            Ok(ToolCall::Growth { days })
        }
        "search" => Ok(ToolCall::Search {
            name: text(args, "name")?,
        }),
        "apps" => Ok(ToolCall::Apps),
        other => Err(format!("unknown tool '{other}'")),
    }
}

/// `value` as JSON text of at most [`MAX_RESULT`] bytes: long lists are cut from the end
/// until it fits, and a note says so.
pub fn cap(mut value: Value) -> String {
    let mut text = value.to_string();
    let mut cut = 0;
    while text.len() > MAX_RESULT {
        let Some(list) = value
            .as_object_mut()
            .and_then(|o| {
                o.values_mut()
                    .find(|v| v.as_array().is_some_and(|a| a.len() > 1))
            })
            .and_then(Value::as_array_mut)
        else {
            text.truncate(MAX_RESULT);
            return text;
        };
        let drop = (list.len() / 4).max(1);
        list.truncate(list.len() - drop);
        cut += drop;
        text = value.to_string();
    }
    if cut > 0
        && let Some(o) = value.as_object_mut()
    {
        o.insert("note".into(), json!(format!("{cut} more items left out")));
        text = value.to_string();
    }
    text
}

/// What the model is told first. `language` is `Dutch` or `English`.
pub fn system_prompt(language: &str) -> String {
    format!(
        "You are Lumen, the companion of Unhoard, an app that shows where disk space went and \
helps get it back safely. Answer in {language}, briefly and concretely, with sizes. You only \
know what the tools return: call them before you answer questions about this PC, and never \
guess sizes, paths or programs. When something may be removed, give its risk tier and the \
reason the tools give, never call something safe on your own. You cannot move or delete \
anything; Unhoard does that after the user clicks, with a preview. When you mention an item \
that has an id, link it as [name](unhoard:item/ID); a suggestion group as \
[title](unhoard:group/KEY). File contents are never available."
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const GB: u64 = 1 << 30;

    #[test]
    fn f8_the_recommended_model_follows_memory_and_video_memory() {
        // A gaming PC: plenty of memory and a graphics card with 8 GB.
        let r = recommend_model(32 * GB, Some(8 * GB));
        assert_eq!((r.model, r.fit), ("qwen3:8b", Fit::Fast));
        // 16 GB of memory and no usable graphics memory: the 8B model runs, slowly.
        let r = recommend_model(16 * GB, None);
        assert_eq!((r.model, r.fit), ("qwen3:8b", Fit::Slow));
        // 8 GB of memory: the smaller model.
        let r = recommend_model(8 * GB, Some(2 * GB));
        assert_eq!((r.model, r.fit), ("qwen3:4b", Fit::Slow));
        // Less: the smaller model, and a warning that it will be slow and weak.
        let r = recommend_model(4 * GB, None);
        assert_eq!((r.model, r.fit), ("qwen3:4b", Fit::TooSmall));
        // How Windows reports a 32 GB PC with an 8 GB card.
        let r = recommend_model(31 * GB + GB / 2, Some(7 * GB + 900 * (GB >> 10)));
        assert_eq!((r.model, r.fit), ("qwen3:8b", Fit::Fast));
        // How Windows reports 16 GB of memory.
        let r = recommend_model(15 * GB + 800 * (GB >> 10), None);
        assert_eq!((r.model, r.fit), ("qwen3:8b", Fit::Slow));
        // A graphics card with 6 GB holds the 4B model fully.
        let r = recommend_model(12 * GB, Some(6 * GB));
        assert_eq!((r.model, r.fit), ("qwen3:4b", Fit::Fast));
    }

    #[test]
    fn f8_only_recommended_models_can_be_downloaded() {
        assert!(may_pull("qwen3:8b"));
        assert!(may_pull("qwen3:4b"));
        assert!(!may_pull("qwen3:8b; rm"));
        assert!(!may_pull("someone/model"));
        assert!(!may_pull(""));
    }

    #[test]
    fn f8_t8_5_unknown_tools_and_bad_arguments_are_errors_for_the_model() {
        assert!(parse_call("delete", &json!({})).is_err());
        assert!(parse_call("children", &json!({})).is_err());
        assert!(parse_call("children", &json!({"path": ""})).is_err());
        assert!(parse_call("children", &json!({"path": "x".repeat(2000)})).is_err());
        assert!(parse_call("growth", &json!({"period": "year"})).is_err());
        assert_eq!(
            parse_call("growth", &json!({"period": "week"})),
            Ok(ToolCall::Growth { days: 7 })
        );
        assert_eq!(
            parse_call("explain", &json!({"path": r" C:\x "})),
            Ok(ToolCall::Explain {
                path: r"C:\x".into()
            })
        );
        assert_eq!(
            parse_call("suggestions", &json!({"group": 5})),
            Ok(ToolCall::Suggestions { group: None })
        );
    }

    #[test]
    fn f8_t8_4_results_are_capped_and_say_what_was_left_out() {
        let items: Vec<Value> = (0..5000)
            .map(|i| json!({"name": format!("item {i}"), "size": i}))
            .collect();
        let text = cap(json!({"items": items}));
        assert!(text.len() <= MAX_RESULT);
        assert!(text.contains("more items left out"));
        let small = cap(json!({"items": [1, 2]}));
        assert_eq!(small, r#"{"items":[1,2]}"#);
    }

    #[test]
    fn f8_every_tool_in_the_schema_parses() {
        let schema = tools_schema();
        let names: Vec<&str> = schema
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|t| t["function"]["name"].as_str())
                    .collect()
            })
            .unwrap_or_default();
        assert_eq!(names.len(), 7);
        let args = json!({"path": r"C:\x", "period": "day", "name": "x"});
        for n in names {
            assert!(parse_call(n, &args).is_ok(), "{n}");
        }
    }
}
