//! Jira's pane: the person's issues from the plugin's list view.

use gpui_kit::{
    div, px, AnyElement, Context, FontWeight, InteractiveElement, IntoElement, ParentElement,
    SharedString, StatefulInteractiveElement, Styled, Window,
};
use serde_json::json;
use sikemux_wire::jira::{IssueList, IssueListKind, IssueSummary};

use super::{Host, Panel, PluginPane};
use crate::theme::Tokens;
use crate::ui;

pub struct Jira;

impl Panel for Jira {
    fn render(
        &mut self,
        host: &mut Host,
        _: &mut Window,
        cx: &mut Context<PluginPane>,
    ) -> AnyElement {
        if host.signed_in(cx) == Some(false) {
            return super::signed_out(
                "Jira",
                "The tickets you are working on, beside the code.",
                cx,
            );
        }
        let read = host
            .view("listView", json!({ "list": IssueListKind::Mine }), cx)
            .read::<IssueList>();
        match read {
            Ok(None) => super::loading(cx),
            Ok(Some(list)) if list.issues.is_empty() => super::empty(
                "Nothing is assigned to you that is open or was done in the last two weeks.",
                cx,
            ),
            Ok(Some(list)) => div()
                .id("jira-issues")
                .size_full()
                .overflow_y_scroll()
                .p(px(8.))
                .flex()
                .flex_col()
                .gap(px(2.))
                .children(list.issues.iter().map(|issue| row(issue, cx)))
                .into_any_element(),
            Err(failure) if failure.signed_out() => super::signed_out(
                "Jira",
                "The tickets you are working on, beside the code.",
                cx,
            ),
            Err(failure) => super::failure(&failure, cx),
        }
    }
}

fn row(issue: &IssueSummary, cx: &Context<PluginPane>) -> AnyElement {
    let t = Tokens::get(cx);
    let assignee: SharedString = issue
        .assignee
        .as_ref()
        .map_or("Unassigned".into(), |person| person.name.clone().into());
    div()
        .px(px(10.))
        .py(px(6.))
        .rounded(crate::theme::RADIUS_2)
        .flex()
        .flex_col()
        .gap(px(2.))
        .child(
            div()
                .flex()
                .gap(px(8.))
                .child(ui::mono(issue.key.clone(), 11., cx))
                .child(ui::meta(issue.status.clone(), cx)),
        )
        .child(ui::text(issue.summary.clone(), 12.5, FontWeight::MEDIUM).text_color(t.text_primary))
        .child(ui::meta(assignee, cx))
        .into_any_element()
}
