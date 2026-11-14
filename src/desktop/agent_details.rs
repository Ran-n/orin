// -----------------------------------------------------------------------------
//+ Authors: 	Ran# <ran.hash@proton.me>
//+ Created: 	2026/11/14 01:02:21.096378
//+ Revised: 	2026/11/14 01:02:28.367362
// -----------------------------------------------------------------------------

//! How one tool call or plan step is shown, and what an approval request says it would touch.

use super::{assets::Icon, controls::icon, theme};
use crate::agents::{PlanEntry, ToolCall};
use gpui::{AnyElement, div, prelude::*, rgb};
use std::path::Path;

/// The icon and colour for a tool call's or plan step's status, as ACP names them.
fn status_mark(status: &str) -> (Icon, u32) {
    let colors = theme::colors();
    match status {
        "completed" => (Icon::Check, colors.accent),
        "failed" => (Icon::Error, colors.error),
        "in_progress" => (Icon::Working, colors.walnut),
        _ => (Icon::Pending, colors.faint),
    }
}

/// What a tool call's status means, for its tooltip.
fn tool_label(status: &str) -> &'static str {
    match status {
        "completed" => "Done",
        "failed" => "Failed",
        "in_progress" => "Running",
        "pending" => "Pending",
        _ => "Activity",
    }
}

pub(super) fn tool_row(tool: &ToolCall) -> AnyElement {
    let colors = theme::colors();
    let (glyph, color) = status_mark(&tool.status);
    theme::row()
        .id(gpui::SharedString::from(format!("tool-{}", tool.id)))
        .flex_shrink_0()
        .items_start()
        .gap_2()
        .text_size(theme::scaled(theme::SMALL_TEXT))
        .tooltip(super::controls::hint(tool_label(&tool.status)))
        .child(div().pt(theme::scaled(2.)).child(icon(glyph, color)))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .whitespace_normal()
                .text_color(rgb(if tool.status == "failed" {
                    colors.error
                } else {
                    colors.soft_text
                }))
                .child(tool.title.clone()),
        )
        .into_any_element()
}

pub(super) fn plan_row(entry: &PlanEntry) -> AnyElement {
    let colors = theme::colors();
    let (glyph, color) = status_mark(&entry.status);
    theme::row()
        .flex_shrink_0()
        .items_start()
        .gap_2()
        .child(div().pt(theme::scaled(2.)).child(icon(glyph, color)))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .whitespace_normal()
                .text_color(rgb(if entry.status == "completed" {
                    colors.muted
                } else {
                    colors.text
                }))
                .child(entry.content.clone()),
        )
        .into_any_element()
}

/// The most lines of detail an approval request lists.
const MAX_DETAIL_LINES: usize = 12;
/// The longest a single detail line may be before it is cut.
const MAX_DETAIL_CHARS: usize = 400;

/// `path` as it is shown: relative to the project `root` when inside it, with `/` separators.
/// Agents report absolute paths; the project part is noise beside the file's place in it.
pub fn shown_path(path: &Path, root: &Path) -> String {
    use crate::process::launch::native_path;
    let (path, root) = (native_path(path), native_path(root));
    let inside = (!root.as_os_str().is_empty())
        .then(|| path.strip_prefix(&root).ok())
        .flatten()
        .filter(|relative| !relative.as_os_str().is_empty());
    inside
        .unwrap_or(&path)
        .display()
        .to_string()
        .replace('\\', "/")
}

/// What a tool call would touch, for the person approving it: the command it runs, when the
/// agent names one, then the files it reads or changes. Only what the agent reported; nothing
/// is guessed.
pub fn tool_details(tool: &ToolCall, root: &Path) -> Vec<String> {
    let mut lines = Vec::new();
    if let Some(input) = &tool.raw_input {
        let command = match &input["command"] {
            serde_json::Value::String(command) => Some(command.clone()),
            serde_json::Value::Array(parts) => Some(
                parts
                    .iter()
                    .filter_map(serde_json::Value::as_str)
                    .collect::<Vec<_>>()
                    .join(" "),
            ),
            _ => None,
        };
        lines.extend(
            command
                .iter()
                .flat_map(|command| command.lines())
                .filter(|line| !line.trim().is_empty())
                .map(|line| format!("$ {line}")),
        );
    }
    for path in tool.locations.iter().chain(&tool.diff_paths) {
        let line = shown_path(path, root);
        if !lines.contains(&line) {
            lines.push(line);
        }
    }
    let hidden = lines.len().saturating_sub(MAX_DETAIL_LINES);
    lines.truncate(MAX_DETAIL_LINES);
    for line in &mut lines {
        if line.chars().count() > MAX_DETAIL_CHARS {
            *line = line.chars().take(MAX_DETAIL_CHARS).collect::<String>() + "…";
        }
    }
    if hidden > 0 {
        lines.push(format!("… and {hidden} more"));
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn tool(input: Option<serde_json::Value>, locations: &[&str], diffs: &[&str]) -> ToolCall {
        ToolCall {
            raw_input: input,
            locations: locations.iter().map(PathBuf::from).collect(),
            diff_paths: diffs.iter().map(PathBuf::from).collect(),
            ..ToolCall::default()
        }
    }

    #[test]
    fn unit_approval_details_show_the_command_and_each_file_once() {
        let details = tool_details(
            &tool(
                Some(serde_json::json!({"command": "cargo test\n\nrm -rf target"})),
                &["src\\main.rs"],
                &["src/main.rs", "README.md"],
            ),
            Path::new(""),
        );
        assert_eq!(
            details,
            [
                "$ cargo test",
                "$ rm -rf target",
                "src/main.rs",
                "README.md"
            ]
        );
        let argv = tool_details(
            &tool(
                Some(serde_json::json!({"command": ["git", "push", 7]})),
                &[],
                &[],
            ),
            Path::new(""),
        );
        assert_eq!(argv, ["$ git push"]);
    }

    #[test]
    fn unit_approval_details_are_empty_when_the_agent_reports_nothing() {
        let root = Path::new("");
        assert!(tool_details(&tool(None, &[], &[]), root).is_empty());
        let number = tool(Some(serde_json::json!({"command": 3})), &[], &[]);
        assert!(tool_details(&number, root).is_empty());
        let text = tool(Some(serde_json::json!("text")), &[], &[]);
        assert!(tool_details(&text, root).is_empty());
    }

    #[test]
    fn unit_approval_details_are_bounded() {
        let many: Vec<String> = (0..40).map(|index| format!("file-{index}.rs")).collect();
        let names: Vec<&str> = many.iter().map(String::as_str).collect();
        let details = tool_details(&tool(None, &names, &[]), Path::new(""));
        assert_eq!(details.len(), MAX_DETAIL_LINES + 1);
        assert_eq!(details.last().unwrap(), "… and 28 more");

        let long = "ñ".repeat(MAX_DETAIL_CHARS + 50);
        let details = tool_details(
            &tool(Some(serde_json::json!({"command": long})), &[], &[]),
            Path::new(""),
        );
        // "$ " plus the kept characters plus the ellipsis; cut on a character, not a byte.
        assert_eq!(details[0].chars().count(), MAX_DETAIL_CHARS + 1);
        assert!(details[0].ends_with('…'));
    }

    #[test]
    fn unit_paths_inside_the_project_are_shown_relative_to_it() {
        let root = std::env::temp_dir().join("orin-project");
        let inside = root.join("src").join("main.rs");
        assert_eq!(shown_path(&inside, &root), "src/main.rs");
        // The root itself, a path outside it, and an unknown root are shown whole.
        assert_eq!(
            shown_path(&root, &root),
            root.display().to_string().replace('\\', "/")
        );
        let outside = std::env::temp_dir().join("elsewhere").join("notes.md");
        assert!(shown_path(&outside, &root).ends_with("elsewhere/notes.md"));
        assert_eq!(
            shown_path(Path::new("src/lib.rs"), Path::new("")),
            "src/lib.rs"
        );
        // A sibling whose name merely starts with the root's is not inside it.
        let sibling = std::env::temp_dir().join("orin-project-old").join("a.rs");
        assert!(shown_path(&sibling, &root).ends_with("orin-project-old/a.rs"));
    }

    #[test]
    fn unit_every_status_has_a_mark_and_unknown_ones_read_as_pending() {
        assert_eq!(status_mark("completed").0, Icon::Check);
        assert_eq!(status_mark("failed").0, Icon::Error);
        assert_eq!(status_mark("in_progress").0, Icon::Working);
        assert_eq!(status_mark("pending").0, Icon::Pending);
        assert_eq!(status_mark("something new").0, Icon::Pending);
        assert_eq!(tool_label("completed"), "Done");
        assert_eq!(tool_label("something new"), "Activity");
    }
}
