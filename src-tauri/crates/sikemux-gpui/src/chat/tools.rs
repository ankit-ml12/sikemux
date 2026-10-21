//! A run of tool calls in a chat: each call's row as the core worked it out,
//! and what the call changed or printed once the reader opens it.

use std::collections::{HashMap, HashSet};
use std::f32::consts::FRAC_PI_2;
use std::time::{SystemTime, UNIX_EPOCH};

use gpui_kit::prelude::FluentBuilder;
use gpui_kit::{
    div, px, relative, AnyElement, App, Div, HighlightStyle, Hsla, InteractiveElement, IntoElement,
    ParentElement, Radians, SharedString, StatefulInteractiveElement, Styled, StyledText,
    Transformation,
};
use sikemux_wire::chat::{
    ChatPart, DiffLine, DiffSummary, OutputSummary, ToolAction, ToolCall, ToolDetail, ToolRow,
};

use super::{duration, Looks};
use crate::core::ChatAsk;
use crate::icons::Icon;
use crate::theme;
use crate::ui;

/// An output longer than this opens on its first lines, and the reader asks for the rest.
const OUTPUT_FOLD_LINES: usize = 12;
/// Unchanged lines kept either side of a change; the rest fold into a gap the reader opens.
const DIFF_CONTEXT: usize = 3;
/// The width of a character of the mono face the run is set in, at its size.
const MONO_CHAR: f32 = 6.6;

/// What the reader opened among a chat's tool calls, and what the core sent of each one opened.
#[derive(Default)]
pub struct Opened {
    /// Runs the reader opened or closed, by their first call's part.
    runs: HashMap<String, bool>,
    /// Calls the reader opened, by call.
    calls: HashSet<String>,
    /// Outputs shown whole and diffs shown with every unchanged line, by call.
    whole: HashSet<String>,
    details: HashMap<String, Asked>,
}

/// A call's detail as asked for: when the call had ended as of asking, and what came back.
struct Asked {
    ended: Option<u64>,
    detail: Option<ToolDetail>,
}

impl Opened {
    /// The open call this part is, when its detail was asked for before it ended, to ask again.
    pub fn stale(&self, part: &ChatPart) -> Option<String> {
        let ChatPart::Tool { tool, ended_at, .. } = part else {
            return None;
        };
        let asked = self.details.get(&tool.tool_call_id)?;
        (self.calls.contains(&tool.tool_call_id) && asked.ended != *ended_at)
            .then(|| tool.tool_call_id.clone())
    }

    pub fn held(&mut self, tool: String, detail: ToolDetail) {
        if let Some(asked) = self.details.get_mut(&tool) {
            asked.detail = Some(detail);
        }
    }
}

/// A tool part's fields, as a row draws them.
struct Call<'a> {
    tool: &'a ToolCall,
    diff: Option<&'a DiffSummary>,
    output: Option<&'a OutputSummary>,
    failure: Option<&'a str>,
    started: Option<u64>,
    ended: Option<u64>,
}

impl<'a> Call<'a> {
    fn of(part: &'a ChatPart) -> Option<Call<'a>> {
        let ChatPart::Tool {
            tool,
            diff,
            output,
            failure,
            started_at,
            ended_at,
            ..
        } = part
        else {
            return None;
        };
        Some(Call {
            tool,
            diff: diff.as_ref(),
            output: output.as_ref(),
            failure: failure.as_deref(),
            started: *started_at,
            ended: *ended_at,
        })
    }

    fn status(&self) -> &str {
        self.tool.status.as_deref().unwrap_or("pending")
    }

    fn running(&self) -> bool {
        !matches!(self.status(), "completed" | "failed" | "cancelled")
    }

    fn took(&self) -> u64 {
        self.ended
            .zip(self.started)
            .map_or(0, |(ended, started)| ended.saturating_sub(started))
    }

    /// The command a call ran, as it was typed.
    fn command(&self) -> Option<&str> {
        (self.tool.kind.as_deref() == Some("execute")).then(|| self.tool.title.trim())
    }

    /// Whether the row has more to show: a change, what it printed, why it failed, or a command longer than its line.
    fn opens(&self) -> bool {
        let cut_off = self
            .command()
            .is_some_and(|command| self.running() || command != self.tool.row.target);
        self.diff.is_some() || self.output.is_some() || self.failure.is_some() || cut_off
    }
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| {
            u64::try_from(since.as_millis()).unwrap_or(u64::MAX)
        })
}

/// Whole seconds, as a running call counts them.
fn seconds(ms: u64) -> String {
    if ms < 1000 {
        return "0s".into();
    }
    duration(ms - ms % 1000)
}

/// The word a row names its call by: the call's own, its server's, or the app's for what it did.
pub(super) fn word(row: &ToolRow) -> &str {
    if let Some(word) = row.verb.as_deref().or(row.server.as_deref()) {
        return word;
    }
    match row.action {
        ToolAction::Read => "read",
        ToolAction::Edit => "edit",
        ToolAction::Delete => "delete",
        ToolAction::Move => "move",
        ToolAction::Search => "search",
        ToolAction::Execute => "run",
        ToolAction::Think => "think",
        ToolAction::Fetch => "fetch",
        ToolAction::SwitchMode => "mode",
        ToolAction::Mcp => "mcp",
        ToolAction::Other => "tool",
    }
}

fn icon(call: &Call) -> Icon {
    let row = &call.tool.row;
    if call.status() == "failed" {
        return Icon::Warning;
    }
    let plugin = row
        .plugin
        .as_deref()
        .and_then(|plugin| Icon::named(plugin.strip_prefix("sikemux.").unwrap_or(plugin)));
    let own = row.icon.as_deref().and_then(Icon::named);
    plugin.or(own).unwrap_or(match row.action {
        ToolAction::Mcp => Icon::Plug,
        ToolAction::Read => Icon::File,
        ToolAction::Search => Icon::Search,
        ToolAction::Edit => Icon::Pencil,
        ToolAction::Move => Icon::Move,
        ToolAction::Delete => Icon::Trash,
        ToolAction::Execute => Icon::Command,
        ToolAction::Fetch => Icon::Globe,
        _ => Icon::Agent,
    })
}

/// A call takes the colour of what it did, mixed back toward the row's text so a run reads as a column.
fn tint(call: &Call, looks: &Looks) -> Hsla {
    let tints = &looks.tints;
    if call.status() == "failed" {
        return tints.failed;
    }
    match call.tool.row.action {
        ToolAction::Read | ToolAction::Search | ToolAction::Fetch => tints.looked,
        ToolAction::Edit | ToolAction::Move => tints.changed,
        ToolAction::Delete => tints.deleted,
        ToolAction::Execute => tints.ran,
        ToolAction::Mcp => tints.called,
        _ => looks.quiet,
    }
}

fn chevron(open: bool, tint: Hsla) -> gpui_kit::Svg {
    ui::icon(Icon::Chevron, 10.)
        .text_color(tint)
        .when(open, |chevron| {
            chevron.with_transformation(Transformation::rotate(Radians(FRAC_PI_2)))
        })
}

/// A run of calls: a line counting them and how long they took, and while the
/// agent is still adding to it, or the reader opened it, each call's row.
pub fn run(parts: &[&ChatPart], live: bool, looks: &Looks) -> AnyElement {
    let calls: Vec<Call> = parts.iter().filter_map(|part| Call::of(part)).collect();
    let Some(first) = parts.first() else {
        return div().into_any_element();
    };
    let current = calls.iter().find(|call| call.running());
    let spent: u64 = calls.iter().map(Call::took).sum();
    let key = first.id().to_owned();
    let open = looks
        .tools
        .borrow()
        .runs
        .get(&key)
        .copied()
        .unwrap_or(live || current.is_some());
    let count = calls.len();
    let said = current.and_then(|call| call.tool.description.clone());
    let label = match &said {
        Some(said) => div()
            .min_w_0()
            .truncate()
            .text_size(px(12.))
            .text_color(looks.quiet)
            .child(said.clone()),
        None => div()
            .min_w_0()
            .truncate()
            .text_color(looks.quiet)
            .child(format!(
                "{count} tool {}",
                if count == 1 { "call" } else { "calls" }
            )),
    };
    let time = match current {
        Some(call) => Some(seconds(
            now().saturating_sub(call.started.unwrap_or_else(now)) + spent,
        )),
        None => (spent > 0).then(|| duration(spent)),
    };
    let (tools, hover, ink) = (looks.tools.clone(), looks.hover, looks.ink);
    let sum = div()
        .id(SharedString::from(format!("run:{key}")))
        .h(px(22.))
        .ml(px(-6.))
        .px(px(6.))
        .flex()
        .items_center()
        .gap(px(8.))
        .max_w_full()
        .rounded(theme::RADIUS_2)
        .cursor_pointer()
        .hover(move |sum| sum.bg(hover).text_color(ink))
        .child(label)
        .when(said.is_some() && count > 1, |sum| {
            sum.child(
                div()
                    .flex_none()
                    .text_color(looks.faint)
                    .child(format!("{count} calls")),
            )
        })
        .children(time.map(|time| div().flex_none().text_color(looks.quiet).child(time)))
        .child(
            div()
                .flex_none()
                .opacity(0.7)
                .child(chevron(open, looks.quiet)),
        )
        .on_click(move |_, window, _| {
            tools.borrow_mut().runs.insert(key.clone(), !open);
            window.refresh();
        });
    let kind = calls
        .iter()
        .map(|call| word(&call.tool.row).chars().count())
        .max()
        .unwrap_or(4)
        .clamp(4, 16);
    let rows = calls
        .iter()
        .enumerate()
        .map(|(ix, call)| row(call, ix + 1 == count, kind, looks));
    div()
        .my(px(7.))
        .font_family(theme::MONO)
        .text_size(px(11.))
        .child(div().flex().child(sum))
        .when(open, |run| {
            run.child(div().ml(px(6.)).py(px(2.)).flex().flex_col().children(rows))
        })
        .into_any_element()
}

/// One call: a tick off the run's spine, its icon and word in the colour of what
/// it did, what it was about, and how long it took, opening onto its detail.
fn row(call: &Call, last: bool, kind: usize, looks: &Looks) -> AnyElement {
    let id = call.tool.tool_call_id.clone();
    let row = &call.tool.row;
    let running = call.running();
    let opens = call.opens();
    let open = opens && looks.tools.borrow().calls.contains(&id);
    let tint = tint(call, looks);
    let target = (!row.target.is_empty()).then(|| {
        div()
            .min_w_0()
            .truncate()
            .text_color(if running { looks.quiet } else { looks.ink })
            .child(row.target.clone())
            .children(row.detail.clone().map(|detail| {
                div()
                    .ml(px(MONO_CHAR))
                    .text_color(looks.quiet)
                    .child(detail)
            }))
            .flex()
    });
    let elapsed = if call.status() == "cancelled" {
        Some("stopped".to_owned())
    } else if running {
        call.started
            .map(|started| seconds(now().saturating_sub(started)))
    } else {
        call.ended
            .zip(call.started)
            .map(|(ended, started)| duration(ended.saturating_sub(started)))
    };
    let quiet = call.diff.is_none() && call.status() != "failed";
    let group = SharedString::from(format!("tool:{id}"));
    let end = div()
        .flex()
        .items_center()
        .gap(px(7.))
        .text_size(px(10.))
        .text_color(looks.quiet)
        .children(call.diff.map(|diff| {
            div()
                .flex()
                .gap(px(5.))
                .child(
                    div()
                        .text_color(looks.tints.added)
                        .child(format!("+{}", diff.adds)),
                )
                .child(
                    div()
                        .text_color(looks.tints.deleted_line)
                        .child(format!("−{}", diff.dels)),
                )
        }))
        .children(elapsed)
        .when(opens, |end| {
            let shown = div()
                .opacity(if quiet && !open {
                    0.
                } else if open {
                    1.
                } else {
                    0.55
                })
                .child(chevron(open, looks.quiet));
            end.child(shown.group_hover(group.clone(), |chevron| chevron.opacity(0.55)))
        });
    let line = div()
        .relative()
        .h(px(22.))
        .pr(px(6.))
        .flex()
        .items_center()
        .gap(px(8.))
        .max_w_full()
        .child(div().w(px(9.)).flex_none())
        .child(
            div()
                .absolute()
                .top_0()
                .bottom_0()
                .left(px(13.))
                .right_0()
                .rounded(theme::RADIUS_2)
                .when(opens, |fill| {
                    fill.group_hover(group.clone(), |fill| fill.bg(looks.hover))
                }),
        )
        .child(
            div()
                .w(px(16.))
                .flex_none()
                .flex()
                .justify_center()
                .child(ui::icon(icon(call), 11.).text_color(tint)),
        )
        .child(
            div()
                .w(px(kind as f32 * MONO_CHAR + 1.))
                .flex_none()
                .truncate()
                .text_color(tint)
                .child(word(row).to_owned()),
        )
        .children(target)
        .child(end);
    let tools = looks.tools.clone();
    let (bench, agent, ended) = (looks.bench.clone(), looks.agent.clone(), call.ended);
    let toggle = move |_: &gpui_kit::ClickEvent, window: &mut gpui_kit::Window, cx: &mut App| {
        let ask = {
            let mut tools = tools.borrow_mut();
            if !tools.calls.remove(&id) {
                tools.calls.insert(id.clone());
            }
            let held = tools
                .details
                .get(&id)
                .is_some_and(|asked| asked.ended == ended);
            let ask = tools.calls.contains(&id) && !held;
            if ask {
                tools.details.insert(
                    id.clone(),
                    Asked {
                        ended,
                        detail: None,
                    },
                );
            }
            ask
        };
        if ask {
            let tool = id.clone();
            bench
                .update(cx, |bench, _| {
                    bench.ask_chat(ChatAsk::Detail {
                        agent: agent.clone(),
                        tool,
                    })
                })
                .ok();
        }
        window.refresh();
    };
    let node = div()
        .id(group.clone())
        .group(group)
        .relative()
        .flex()
        .flex_col()
        .child(spine(last, looks))
        .child(line)
        .when(opens, |node| node.cursor_pointer().on_click(toggle))
        .when(open, |node| node.child(detail(call, looks)));
    node.into_any_element()
}

/// The run's spine down the left of a row, and the tick that joins the row to it;
/// the last row's turns the corner instead.
fn spine(last: bool, looks: &Looks) -> Div {
    let (spine, tick) = (looks.tints.spine, looks.tints.tick);
    if last {
        return div()
            .absolute()
            .top_0()
            .left_0()
            .w(px(9.))
            .h(px(11.5))
            .border_l_1()
            .border_b_1()
            .border_color(spine)
            .rounded_bl(px(4.));
    }
    div()
        .absolute()
        .top_0()
        .bottom_0()
        .left_0()
        .w(px(9.))
        .child(
            div()
                .absolute()
                .top_0()
                .bottom_0()
                .left_0()
                .w(px(1.))
                .bg(spine),
        )
        .child(
            div()
                .absolute()
                .top(px(11.))
                .left_0()
                .w(px(9.))
                .h(px(1.))
                .bg(tick),
        )
}

/// What an opened call did: its change, the command and what it printed, or why it failed.
fn detail(call: &Call, looks: &Looks) -> AnyElement {
    let id = &call.tool.tool_call_id;
    let held = looks
        .tools
        .borrow()
        .details
        .get(id)
        .and_then(|asked| asked.detail.clone());
    let body = if call.diff.is_some() {
        held.and_then(|held| held.diff).map(|diff| {
            diff_body(id, &diff.path, diff.adds, diff.dels, &diff.lines, looks).into_any_element()
        })
    } else if call.command().is_some() || call.output.is_some() {
        Some(terminal(call, held.as_ref(), looks).into_any_element())
    } else {
        call.failure.map(|failure| {
            div()
                .text_size(px(11.))
                .line_height(relative(1.55))
                .text_color(looks.tints.failed)
                .child(failure.to_owned())
                .into_any_element()
        })
    };
    div()
        .ml(px(33.))
        .mt(px(1.))
        .mb(px(6.))
        .children(body)
        .into_any_element()
}

fn boxed(looks: &Looks) -> Div {
    div()
        .flex()
        .flex_col()
        .overflow_hidden()
        .border_1()
        .border_color(looks.tints.line_soft)
        .rounded(theme::RADIUS_2)
        .bg(looks.tints.well)
        .text_size(px(11.))
        .line_height(px(17.))
}

/// A command and what it printed, the way a terminal would have shown them.
fn terminal(call: &Call, held: Option<&ToolDetail>, looks: &Looks) -> Div {
    let id = call.tool.tool_call_id.clone();
    let output = held.and_then(|held| held.output.as_ref());
    let whole = looks.tools.borrow().whole.contains(&id);
    let text = output.map_or("", |output| output.text.as_str());
    let lines: Vec<&str> = if text.is_empty() {
        Vec::new()
    } else {
        text.split('\n').collect()
    };
    let folds = lines.len() > OUTPUT_FOLD_LINES;
    let shown = if folds && !whole {
        lines[..OUTPUT_FOLD_LINES].join("\n")
    } else {
        text.to_owned()
    };
    let rule = looks.tints.line_soft;
    let command = call.command().map(|command| {
        div()
            .flex()
            .gap(px(7.))
            .pl(px(9.))
            .pr(px(5.))
            .py(px(5.))
            .child(div().flex_none().text_color(looks.tints.ran).child("$"))
            .child(
                div()
                    .min_w_0()
                    .flex_1()
                    .text_color(looks.ink)
                    .child(command.to_owned()),
            )
    });
    let printed = output.map(|_| {
        if shown.is_empty() {
            return div()
                .px(px(9.))
                .py(px(5.))
                .text_size(px(10.))
                .text_color(looks.faint)
                .child("No output");
        }
        div()
            .pl(px(9.))
            .pr(px(30.))
            .pt(px(6.))
            .pb(px(7.))
            .text_color(looks.quiet)
            .child(shown)
    });
    let more = folds.then(|| {
        let (tools, hover) = (looks.tools.clone(), looks.hover);
        let label = if whole {
            "Show fewer lines".to_owned()
        } else {
            format!("Show all {} lines", lines.len())
        };
        div()
            .id(SharedString::from(format!("more:{id}")))
            .h(px(22.))
            .pl(px(9.))
            .flex()
            .items_center()
            .text_size(px(10.))
            .text_color(looks.faint)
            .cursor_pointer()
            .hover(move |more| more.bg(hover))
            .on_click(move |_, window, cx| {
                cx.stop_propagation();
                let mut tools = tools.borrow_mut();
                if !tools.whole.remove(&id) {
                    tools.whole.insert(id.clone());
                }
                window.refresh();
            })
            .child(label)
    });
    let cut = output.is_some_and(|output| output.cut) && (!folds || whole);
    let exit = call
        .output
        .and_then(|output| output.exit_code)
        .filter(|_| call.status() == "failed");
    let mut parts: Vec<AnyElement> = command
        .into_iter()
        .map(IntoElement::into_any_element)
        .collect();
    parts.extend(printed.map(IntoElement::into_any_element));
    parts.extend(more.map(IntoElement::into_any_element));
    if cut {
        parts.push(
            div()
                .px(px(9.))
                .py(px(5.))
                .text_size(px(10.))
                .text_color(looks.faint)
                .child("The rest of the output was not kept")
                .into_any_element(),
        );
    }
    let ruled = parts.into_iter().enumerate().map(|(ix, part)| {
        div()
            .when(ix > 0, |part| part.border_t_1().border_color(rule))
            .child(part)
    });
    boxed(looks).children(ruled).children(exit.map(|code| {
        div()
            .px(px(9.))
            .pb(px(6.))
            .text_size(px(10.))
            .text_color(looks.tints.failed)
            .child(format!("exit {code}"))
    }))
}

/// The lines a change made, numbered, with the lines around it and a gap for the rest.
fn diff_body(id: &str, path: &str, adds: u64, dels: u64, lines: &[DiffLine], looks: &Looks) -> Div {
    let whole = looks.tools.borrow().whole.contains(id);
    let head = div()
        .h(px(22.))
        .px(px(9.))
        .flex()
        .items_center()
        .gap(px(8.))
        .border_b_1()
        .border_color(looks.tints.line_soft)
        .text_size(px(10.))
        .text_color(looks.faint)
        .child(ui::icon(Icon::File, 12.).text_color(looks.faint))
        .child(
            div()
                .min_w_0()
                .truncate()
                .text_color(looks.quiet)
                .child(path.to_owned()),
        )
        .child(
            div()
                .ml_auto()
                .text_color(looks.tints.added)
                .child(format!("+{adds}")),
        )
        .child(
            div()
                .text_color(looks.tints.deleted_line)
                .child(format!("−{dels}")),
        );
    let keep = kept(lines, if whole { usize::MAX } else { DIFF_CONTEXT });
    let mut body: Vec<AnyElement> = Vec::new();
    let mut gap = 0;
    for (line, kept) in lines.iter().zip(&keep) {
        if !*kept {
            gap += 1;
            continue;
        }
        if gap > 0 {
            body.push(diff_gap(id, gap, looks));
            gap = 0;
        }
        body.push(diff_line(line, looks));
    }
    if gap > 0 {
        body.push(diff_gap(id, gap, looks));
    }
    boxed(looks)
        .child(head)
        .child(div().py(px(3.)).flex().flex_col().children(body))
}

/// Which lines are within `context` of a change.
fn kept(lines: &[DiffLine], context: usize) -> Vec<bool> {
    let mut keep = vec![false; lines.len()];
    for (ix, line) in lines.iter().enumerate() {
        if line.sign != " " {
            let end = ix.saturating_add(context).min(lines.len() - 1);
            keep[ix.saturating_sub(context)..=end]
                .iter_mut()
                .for_each(|kept| *kept = true);
        }
    }
    keep
}

fn diff_gap(id: &str, gap: usize, looks: &Looks) -> AnyElement {
    let (tools, id, hover) = (looks.tools.clone(), id.to_owned(), looks.hover);
    div()
        .id(SharedString::from(format!("gap:{id}:{gap}")))
        .h(px(20.))
        .pl(px(42.))
        .flex()
        .items_center()
        .text_size(px(10.))
        .text_color(looks.faint)
        .cursor_pointer()
        .hover(move |gap| gap.bg(hover))
        .on_click(move |_, window, cx| {
            cx.stop_propagation();
            tools.borrow_mut().whole.insert(id.clone());
            window.refresh();
        })
        .child(format!(
            "{gap} unchanged {}",
            if gap == 1 { "line" } else { "lines" }
        ))
        .into_any_element()
}

fn diff_line(line: &DiffLine, looks: &Looks) -> AnyElement {
    let tints = &looks.tints;
    let (ground, sign, mark) = match line.sign.as_str() {
        "+" => (Some(tints.added_ground), tints.added, tints.added_mark),
        "-" => (
            Some(tints.deleted_ground),
            tints.deleted_line,
            tints.deleted_mark,
        ),
        _ => (None, looks.faint, tints.added_mark),
    };
    let number = if line.sign == "+" {
        line.new_line
    } else {
        line.old_line
    };
    let marked = line
        .mark
        .as_deref()
        .and_then(|mark| units_to_bytes(&line.text, mark));
    let text = StyledText::new(line.text.clone()).with_highlights(marked.map(|range| {
        (
            range,
            HighlightStyle {
                background_color: Some(mark),
                ..Default::default()
            },
        )
    }));
    div()
        .flex()
        .whitespace_nowrap()
        .when_some(ground, |line, ground| line.bg(ground))
        .child(
            div()
                .w(px(30.))
                .flex_none()
                .pr(px(8.))
                .flex()
                .justify_end()
                .text_size(px(10.))
                .text_color(looks.faint)
                .children(number.map(|n| n.to_string())),
        )
        .child(
            div()
                .w(px(12.))
                .flex_none()
                .text_color(sign)
                .child(line.sign.clone()),
        )
        .child(div().min_w_0().text_color(looks.ink).child(text))
        .into_any_element()
}

/// A span the core counts in UTF-16 units, as byte offsets into `text`.
fn units_to_bytes(text: &str, mark: &[u64]) -> Option<std::ops::Range<usize>> {
    let [start, end] = mark else {
        return None;
    };
    let byte = |unit: u64| {
        let mut units = 0;
        for (at, c) in text.char_indices() {
            if units >= unit {
                return at;
            }
            units += c.len_utf16() as u64;
        }
        text.len()
    };
    let range = byte(*start)..byte(*end);
    (range.start < range.end).then_some(range)
}
