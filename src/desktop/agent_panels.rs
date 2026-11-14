// -----------------------------------------------------------------------------
//+ Authors: 	Ran# <ran.hash@proton.me>
//+ Created: 	2026/11/14 01:02:21.268921
//+ Revised: 	2026/11/14 01:02:28.448120
// -----------------------------------------------------------------------------

//! What stays pinned above an agent's message box: its plan, the approvals and sign-in it is
//! waiting on, and the files it changed.

use super::{
    agent_details::{plan_row, shown_path, tool_details},
    agent_view::{AgentAction, AgentView},
    assets::Icon,
    controls::{ControlSpec, button, icon},
    file_icons::file_icon,
    theme,
};
use crate::agents::{AgentStatus, PendingPermission};
use gpui::{AnyElement, Context, div, prelude::*, rgb};

/// The most changed files listed as buttons; an agent can touch hundreds.
const MAX_REVIEW_BUTTONS: usize = 24;
/// How tall an approval request grows before it scrolls.
const PANEL_MAX_HEIGHT: f32 = 220.0;

impl AgentView {
    /// The agent's plan as a checklist, pinned above the message box while work remains.
    pub(super) fn plan_panel(&self) -> Option<AnyElement> {
        let plan = &self.snapshot.plan;
        let done = plan
            .iter()
            .filter(|entry| entry.status == "completed")
            .count();
        if plan.is_empty() || (done == plan.len() && !self.snapshot.is_busy()) {
            return None;
        }
        let colors = theme::colors();
        Some(
            theme::column()
                .id("agent-plan")
                .flex_shrink_0()
                .max_h(theme::scaled(140.))
                .overflow_y_scroll()
                .mx_3()
                .mb_2()
                .px_2()
                .py_1()
                .gap(theme::scaled(2.))
                .rounded(theme::scaled(6.))
                .border_1()
                .border_color(rgb(colors.border))
                .bg(rgb(colors.panel))
                .text_size(theme::scaled(theme::SMALL_TEXT))
                .child(
                    div()
                        .flex_shrink_0()
                        .text_size(theme::scaled(theme::TINY_TEXT))
                        .text_color(rgb(colors.muted))
                        .child(format!("Plan · {done} of {} done", plan.len())),
                )
                .children(plan.iter().map(plan_row))
                .into_any_element(),
        )
    }

    pub(super) fn permission_panels(&self, cx: &Context<Self>) -> Vec<AnyElement> {
        self.snapshot
            .pending_permissions
            .iter()
            .map(|request| self.permission_panel(request, cx))
            .collect()
    }

    /// One approval request: what the agent wants to do, what it touches, and the agent's own
    /// choices. The first way to allow it is the main action; Decline is always offered.
    fn permission_panel(&self, request: &PendingPermission, cx: &Context<Self>) -> AnyElement {
        let colors = theme::colors();
        let mut choices = Vec::new();
        let mut primary_given = false;
        for option in &request.options {
            let request_id = request.request_id.clone();
            let option_id = option.option_id.clone();
            let primary = !primary_given && option.kind.starts_with("allow");
            primary_given |= primary;
            choices.push(button(
                format!("permission-{}-{}", request_id, option_id),
                option.name.clone(),
                true,
                primary,
                &self.controls,
                cx,
                move |view, _, cx| view.permission(&request_id, Some(&option_id), cx),
            ));
        }
        let request_id = request.request_id.clone();
        choices.push(button(
            format!("permission-cancel-{request_id}"),
            "Decline",
            true,
            false,
            &self.controls,
            cx,
            move |view, _, cx| view.permission(&request_id, None, cx),
        ));
        let details = self
            .snapshot
            .tool_calls
            .iter()
            .find(|tool| tool.id == request.tool_call_id)
            .map(|tool| tool_details(tool, &self.root))
            .unwrap_or_default();
        theme::column()
            .id(gpui::SharedString::from(format!(
                "permission-{}",
                request.request_id
            )))
            .flex_shrink_0()
            .max_h(theme::scaled(PANEL_MAX_HEIGHT))
            .overflow_y_scroll()
            .mx_3()
            .mb_2()
            .p_2()
            .gap_2()
            .border_1()
            .border_color(rgb(colors.warning))
            .rounded(theme::scaled(8.))
            .bg(rgb(colors.panel))
            .child(
                theme::row()
                    .flex_shrink_0()
                    .items_start()
                    .gap_2()
                    .child(
                        div()
                            .pt(theme::scaled(2.))
                            .child(icon(Icon::Error, colors.warning)),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .whitespace_normal()
                            .child(request.title.clone()),
                    ),
            )
            .when(!details.is_empty(), |panel| {
                panel.child(
                    theme::column()
                        .flex_shrink_0()
                        .px_2()
                        .py_1()
                        .rounded(theme::scaled(theme::RADIUS))
                        .bg(rgb(colors.background))
                        .font_family(theme::code_font())
                        .text_size(theme::scaled(theme::SMALL_TEXT))
                        .text_color(rgb(colors.soft_text))
                        .children(
                            details
                                .into_iter()
                                .map(|line| div().whitespace_normal().child(line)),
                        ),
                )
            })
            .child(
                theme::row()
                    .flex_shrink_0()
                    .flex_wrap()
                    .gap_2()
                    .children(choices),
            )
            .into_any_element()
    }

    pub(super) fn authentication(&self, cx: &Context<Self>) -> Vec<AnyElement> {
        if self.snapshot.status != AgentStatus::AuthRequired {
            return vec![];
        }
        self.snapshot
            .auth_methods
            .iter()
            .enumerate()
            .map(|(index, method)| {
                let id = method.id.clone();
                button(
                    format!("authenticate-{id}"),
                    method.name.clone(),
                    true,
                    index == 0,
                    &self.controls,
                    cx,
                    move |view, _, cx| view.authenticate(&id, cx),
                )
            })
            .collect()
    }

    /// The files this agent changed, each opening its changes in Code.
    pub(super) fn changed_files(&self, cx: &Context<Self>) -> Option<AnyElement> {
        let files = &self.snapshot.changed_files;
        if files.is_empty() {
            return None;
        }
        let colors = theme::colors();
        let buttons = files.iter().take(MAX_REVIEW_BUTTONS).map(|path| {
            let name = path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned();
            let (glyph, _) = file_icon(&name);
            let open = path.clone();
            ControlSpec::new(
                format!("changed-file-{}", path.display()),
                &self.controls,
                cx,
            )
            .tooltip(format!("Review {}", shown_path(path, &self.root)))
            .icon_text(glyph, name, move |_, _, cx| {
                cx.emit(AgentAction::OpenFile(open.clone()))
            })
        });
        let hidden = files.len().saturating_sub(MAX_REVIEW_BUTTONS);
        Some(
            theme::column()
                .flex_shrink_0()
                .gap_1()
                .pt_1()
                .child(
                    div()
                        .text_size(theme::scaled(theme::TINY_TEXT))
                        .text_color(rgb(colors.muted))
                        .child(match files.len() {
                            1 => "1 changed file · click to review".to_owned(),
                            count => format!("{count} changed files · click to review"),
                        }),
                )
                .child(
                    theme::row()
                        .flex_wrap()
                        .gap_1()
                        .children(buttons)
                        .when(hidden > 0, |row| {
                            row.child(
                                div()
                                    .text_size(theme::scaled(theme::SMALL_TEXT))
                                    .text_color(rgb(colors.muted))
                                    .child(format!("and {hidden} more in Files")),
                            )
                        }),
                )
                .into_any_element(),
        )
    }
}
