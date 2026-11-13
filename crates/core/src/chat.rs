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
