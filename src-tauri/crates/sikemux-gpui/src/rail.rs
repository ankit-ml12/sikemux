use std::rc::Rc;

use gpui_kit::base::Popover;
use gpui_kit::component::scroll::ScrollableElement;
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::{
    div, px, Anchor, AnyElement, App, Context, FontWeight, InteractiveElement, IntoElement,
    ParentElement, SharedString, StatefulInteractiveElement, Styled,
};
use serde_json::Value;
use sikemux_wire::protocol::{PaneKind, ProjectInfo};

use crate::icons::Icon;
use crate::keymap;
use crate::tabs;
use crate::theme::{Tokens, RADIUS_1, RADIUS_2};
use crate::ui::{self, Button, Group, IconButton, Menu, MenuItem, Row, Tone, Variant};
use crate::workbench::{AgentRow, Edge, Strip, Workbench};

pub const SIDE_RAIL_WIDTH: f32 = 258.;

pub fn side_rail(bench: &Workbench, cx: &mut Context<Workbench>) -> impl IntoElement {
    let projects: Vec<_> = bench
        .projects()
        .iter()
        .map(|project| project_tree(bench, project, cx).into_any_element())
        .collect();
    ui::panel(cx)
        .w(px(SIDE_RAIL_WIDTH))
        .flex_none()
        .child(masthead(bench, cx))
        .child(
            div()
                .id("side-rail-scroll")
                .flex_1()
                .min_h_0()
                .pt(px(10.))
                .px(px(10.))
                .pb(px(16.))
                .overflow_y_scrollbar()
                .child(
                    Group::new("Projects", "no projects")
                        .first()
                        .actions(actions(
                            keymap::hint("project.open", cx),
                            [
                                IconButton::new("add-project", Icon::Plus, "Open project")
                                    .on_click(|_, window, cx| {
                                        keymap::dispatch("project.open", window, cx)
                                    }),
                            ],
                            cx,
                        ))
                        .child(div().mb(px(6.)).child(
                            Button::new("new-space", "New space", Variant::Dashed).icon(Icon::Plus),
                        ))
                        .children(projects),
                )
                .child(
                    Group::new("Hosts", "no hosts")
                        .actions(actions(
                            keymap::hint("ssh.open", cx),
                            [
                                IconButton::new("edit-ssh", Icon::Pencil, "Edit ~/.ssh/config"),
                                IconButton::new("add-host", Icon::Plus, "Find a host").on_click(
                                    |_, window, cx| keymap::dispatch("ssh.open", window, cx),
                                ),
                            ],
                            cx,
                        ))
                        .child(this_host(bench, cx))
                        .children(hosts(bench, cx)),
                )
                .child(Group::new("Plugins", "no plugins").children(plugins(bench, cx)))
                .child(
                    Group::new("Terminals", "no terminals")
                        .actions(actions(
                            None,
                            [IconButton::new("add-terminal", Icon::Plus, "New terminal")
                                .on_click(cx.listener(|bench, _, _, _| bench.new_terminal()))],
                            cx,
                        ))
                        .children(terminals(bench, cx)),
                ),
        )
        .child(account(bench, cx))
}

/// The host this window runs on, where new work starts.
fn this_host(bench: &Workbench, cx: &App) -> impl IntoElement {
    let t = Tokens::get(cx).clone();
    let agents = bench
        .view
        .as_ref()
        .map_or(0, |view| view.workspace.agents.len());
    Row::new("this-host", "This host")
        .mark(
            16.,
            ui::icon(Icon::Laptop, 14.)
                .text_color(t.text_secondary)
                .group_hover(ui::ROW, move |style| style.text_color(t.text_primary)),
        )
        .quiet()
        .tag("here")
        .when(agents > 0, |row| {
            row.child(ui::meta(
                format!("{agents} {}", if agents == 1 { "agent" } else { "agents" }),
                cx,
            ))
        })
        .status(Some(ui::dot(Tone::Ok, cx)))
}

/// The account's other hosts, then the SSH hosts a terminal is open to.
fn hosts(bench: &Workbench, cx: &mut Context<Workbench>) -> Vec<AnyElement> {
    let t = Tokens::get(cx).clone();
    rows(bench.read("hosts"))
        .map(|host| {
            let name = host["name"].as_str().unwrap_or_default().to_owned();
            let linked = !host["id"].is_null();
            let reach = host["reach"].as_str().unwrap_or_default();
            let tone = match reach {
                "connected" => Tone::Ok,
                "connecting" => Tone::Pending,
                "failed" => Tone::Failed,
                _ => Tone::Idle,
            };
            let detail = match reach {
                "away" if linked => Some("out of reach"),
                "connecting" if linked => Some("connecting"),
                _ => None,
            };
            let alias = host["ssh"].as_str().filter(|_| !linked).map(str::to_owned);
            let front = alias.is_some() && bench.strip() == alias.clone().map(Strip::Host);
            Row::new(SharedString::from(format!("host-{name}")), name)
                .chosen(front)
                .when_some(alias, |row, alias| {
                    row.on_click(cx.listener(move |bench, _, _, _| bench.open_ssh_terminal(&alias)))
                })
                .mark(
                    16.,
                    ui::icon(Icon::Server, 14.).text_color(if linked { t.host } else { t.ssh }),
                )
                .quiet()
                .children(detail.map(|detail| ui::meta(detail, cx)))
                .status(Some(ui::dot(tone, cx)))
                .into_any_element()
        })
        .collect()
}

/// The plugins that are on and draw in the rail, signed in or not.
fn plugins(bench: &Workbench, cx: &App) -> Vec<AnyElement> {
    let ink = Tokens::get(cx).text_secondary;
    rows(bench.read("plugins"))
        .filter(|plugin| plugin["enabled"].as_bool() == Some(true))
        .filter_map(|plugin| {
            let mark = plugin_mark(plugin["id"].as_str()?)?;
            let tone =
                plugin["signedIn"]
                    .as_bool()
                    .map(|signed_in| if signed_in { Tone::Ok } else { Tone::Idle });
            Some(
                Row::new(
                    SharedString::from(format!("plugin-{}", plugin["id"].as_str()?)),
                    plugin["name"].as_str()?.to_owned(),
                )
                .mark(16., ui::picture(mark, 13., ink))
                .quiet()
                .status(tone.map(|tone| ui::dot(tone, cx)))
                .into_any_element(),
            )
        })
        .collect()
}

/// The mark of a plugin that has a place of its own in the rail; code hosts show
/// under their projects instead.
fn plugin_mark(id: &str) -> Option<Icon> {
    Some(match id.strip_prefix("sikemux.")? {
        "aws" => Icon::Aws,
        "bruno" => Icon::Bruno,
        "database" => Icon::Database,
        "jira" => Icon::Jira,
        "linear" => Icon::Linear,
        "rundeck" => Icon::Rundeck,
        "signoz" => Icon::Signoz,
        "slack" => Icon::Slack,
        "vercel" => Icon::Vercel,
        _ => return None,
    })
}

/// Terminals started outside every project.
fn terminals(bench: &Workbench, cx: &App) -> Vec<AnyElement> {
    let live = Tokens::get(cx).live;
    bench
        .loose_terminals()
        .into_iter()
        .map(|session| {
            Row::new(
                SharedString::from(format!("terminal-{}", session.id)),
                session.rail_name.clone().unwrap_or_default(),
            )
            .mark(16., ui::icon(Icon::Terminal, 13.).text_color(live))
            .quiet()
            .into_any_element()
        })
        .collect()
}

fn rows(read: Option<&Value>) -> impl Iterator<Item = &Value> {
    read.and_then(Value::as_array).into_iter().flatten()
}

fn masthead(bench: &Workbench, cx: &mut Context<Workbench>) -> impl IntoElement {
    let t = Tokens::get(cx).clone();
    let (release, channel) = split_version(&bench.version);
    div()
        .flex()
        .flex_none()
        .items_center()
        .gap(px(6.))
        .h(px(40.))
        .pl(px(18.))
        .pr(px(8.))
        .border_b_1()
        .border_color(t.border)
        .child(ui::icon(Icon::Logo, 14.).text_color(t.accent))
        .child(
            ui::text("Sikemux", 13., FontWeight::SEMIBOLD)
                .text_color(t.text_primary)
                .ml(px(1.)),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(4.))
                .pt(px(1.))
                .child(ui::mono(format!("v{release}"), 10., cx))
                .children(channel.map(|channel| {
                    ui::text(channel, 10., FontWeight::MEDIUM)
                        .px(px(4.))
                        .py(px(1.))
                        .line_height(px(12.))
                        .border_1()
                        .border_color(t.line)
                        .rounded(RADIUS_1)
                        .text_color(t.text_tertiary)
                })),
        )
        .child(
            div().ml_auto().child(
                IconButton::new(
                    "hide-side-rail",
                    Icon::PanelLeft,
                    if bench.side_rail_open {
                        "Hide the side rail"
                    } else {
                        "Keep the side rail open"
                    },
                )
                .on_click(cx.listener(|bench, _, _, cx| bench.toggle_rail(Edge::Start, cx))),
            ),
        )
}

/// `0.4.0-nightly.10` as the release `0.4.0` and the channel `nightly 10`.
fn split_version(version: &str) -> (String, Option<String>) {
    match version.split_once('-') {
        Some((release, channel)) => (release.into(), Some(channel.replacen('.', " ", 1))),
        None => (version.into(), None),
    }
}

fn actions<const N: usize>(
    keys: Option<SharedString>,
    buttons: [IconButton; N],
    cx: &App,
) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .gap(px(6.))
        .children(keys.map(|keys| ui::kbd(keys, cx)))
        .children(buttons.map(IconButton::small))
}

fn project_tree(
    bench: &Workbench,
    project: &ProjectInfo,
    cx: &mut Context<Workbench>,
) -> impl IntoElement {
    let t = Tokens::get(cx).clone();
    let active = bench.strip() == Some(Strip::Project(project.id.clone()));
    let agents = bench.agents_of(&project.id);
    let id = project.id.clone();
    div()
        .flex()
        .flex_col()
        .child(
            Row::new(
                SharedString::from(format!("project-{}", project.id)),
                project.name.clone(),
            )
            .mark(16., ui::icon(Icon::Folder, 13.).text_color(t.live))
            .quiet()
            .bold(active)
            .chosen(active)
            .padding_right(4.)
            .on_click(cx.listener(move |bench, _, _, _| bench.activate_project(&id)))
            .child(faces(&agents, cx)),
        )
        .when(active, |tree| {
            tree.pb(px(4.)).child(surfaces(
                project,
                bench
                    .tab()
                    .and_then(|tab| tabs::first_view(&tab.pane))
                    .map(|(kind, _)| kind),
                cx,
            ))
        })
}

/// Up to five agents' faces, each overlapping the one before by 6px.
fn faces(agents: &[AgentRow], cx: &App) -> impl IntoElement {
    let shown = agents.len().min(5);
    div()
        .relative()
        .flex_none()
        .h(px(20.))
        .w(px(if shown == 0 {
            0.
        } else {
            20. + 14. * (shown - 1) as f32
        }))
        .children(agents.iter().take(shown).enumerate().map(|(index, agent)| {
            ui::mark_box(20., ui::agent_mark(agent, 20., cx))
                .absolute()
                .left(px(14. * index as f32))
        }))
}

/// What a project opens, in the order the rail lists them: its own surfaces,
/// with its code host's sections after Git.
fn surface_rows(
    project: &ProjectInfo,
) -> Vec<(Icon, SharedString, &'static str, Option<PaneKind>)> {
    let mut rows = vec![
        (
            Icon::Editor,
            "Files".into(),
            "window.files",
            Some(PaneKind::Files),
        ),
        (
            Icon::Terminal,
            "Terminal".into(),
            "window.terminal",
            Some(PaneKind::Terminal),
        ),
        (
            Icon::Agent,
            "Agents".into(),
            "window.agents",
            Some(PaneKind::Agent),
        ),
        (Icon::Git, "Git".into(), "window.git", Some(PaneKind::Git)),
    ];
    if let Some(host) = &project.code_host {
        rows.extend(host.sections.iter().filter_map(|section| {
            Some(match section.as_str() {
                "pulls" => (Icon::PullRequest, "Pull requests".into(), "", None),
                "actions" => (Icon::Actions, host.ci_name.clone().into(), "", None),
                "issues" => (Icon::Issues, "Issues".into(), "", None),
                "releases" => (Icon::Releases, "Releases".into(), "", None),
                _ => return None,
            })
        }));
    }
    rows.extend([
        (
            Icon::Search,
            "Search".into(),
            "search.global",
            Some(PaneKind::Search),
        ),
        (
            Icon::Board,
            "Agent board".into(),
            "window.board",
            Some(PaneKind::Board),
        ),
    ]);
    rows
}

/// The project's surfaces, hung off a spine with a tick to each, the one in front lit.
fn surfaces(project: &ProjectInfo, front: Option<PaneKind>, cx: &App) -> impl IntoElement {
    let t = Tokens::get(cx).clone();
    let rows = surface_rows(project);
    let last = rows.len() - 1;
    div()
        .flex()
        .flex_col()
        .ml(px(20.))
        .py(px(2.))
        .pr(px(6.))
        .children(rows.into_iter().enumerate().map(
            move |(index, (glyph, label, command, kind))| {
                let tail = index == last;
                let lit = kind.is_some() && kind == front;
                let spine = div().absolute().left_0().top_0();
                div()
                    .id(SharedString::from(format!("surface-{index}")))
                    .relative()
                    .h(px(25.))
                    .when(!command.is_empty(), |row| {
                        row.on_click(move |_, window, cx| keymap::dispatch(command, window, cx))
                    })
                    .pr(px(8.))
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .rounded(RADIUS_2)
                    .text_color(if lit { t.accent } else { t.text_tertiary })
                    .when(!lit, |row| {
                        row.hover(move |style| style.text_color(t.text_primary))
                    })
                    .child(if tail {
                        spine
                            .w(px(9.))
                            .h(px(13.))
                            .border_l_1()
                            .border_b_1()
                            .border_color(t.tree_spine)
                            .rounded_bl(px(4.))
                    } else {
                        spine.bottom_0().w(px(1.)).bg(t.tree_spine)
                    })
                    .when(!tail, |row| {
                        row.child(
                            div()
                                .absolute()
                                .left_0()
                                .top(px(12.))
                                .w(px(9.))
                                .h(px(1.))
                                .bg(t.tree_tick),
                        )
                    })
                    .child(div().w(px(9.)).flex_none())
                    .child(ui::mark_box(
                        16.,
                        ui::icon(glyph, 13.).text_color(if lit { t.live } else { t.text_tertiary }),
                    ))
                    .child(ui::text(label, 12.5, FontWeight::MEDIUM).letter_spacing(0.01))
                    .when_some(keymap::hint(command, cx), |row, keys| {
                        let keys = ui::kbd(keys, cx).when(lit, |keys| keys.text_color(t.accent));
                        row.child(
                            div()
                                .ml_auto()
                                .when(!lit, |keys| keys.opacity(0.5))
                                .child(keys),
                        )
                    })
            },
        ))
}

/// Who the host is signed in as, or the card that signs it in.
fn account(bench: &Workbench, cx: &mut Context<Workbench>) -> impl IntoElement {
    let t = Tokens::get(cx).clone();
    let Some(account) = bench.view.as_ref().and_then(|view| view.account.as_deref()) else {
        return sign_in(bench, cx).into_any_element();
    };
    let who = account
        .name
        .clone()
        .or_else(|| account.email.clone())
        .unwrap_or_else(|| "Signed in".into());
    let initials = initials(account.name.as_deref(), account.email.as_deref());
    let row = Row::new("account", who)
        .mark(
            24.,
            div()
                .size(px(24.))
                .rounded_full()
                .border_1()
                .border_color(t.border)
                .bg(t.surface_raised)
                .overflow_hidden()
                .flex()
                .items_center()
                .justify_center()
                .text_size(px(9.))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(t.text_secondary)
                .map(|face| match bench.picture() {
                    Some(picture) => face.child(gpui_kit::img(picture).size_full()),
                    None => face.child(initials),
                }),
        )
        .when_some(
            account.name.as_ref().and(account.email.clone()),
            |row, email| row.subtitle(ui::meta(email, cx)),
        );
    let sign_out = Rc::new(cx.listener(|bench, _, _, _| bench.sign_out()));
    let menu = Popover::new("account-menu")
        .anchor(Anchor::BottomLeft)
        .offset(px(4.))
        .trigger(row)
        .content(move |_, _, cx| {
            let popover = cx.entity();
            let sign_out = sign_out.clone();
            Menu::new().child(
                MenuItem::new("sign-out", "Sign out")
                    .icon(Icon::LogOut)
                    .on_click(move |event, window, cx| {
                        sign_out(event, window, cx);
                        popover.update(cx, |popover, cx| popover.dismiss(window, cx));
                    }),
            )
        });
    div()
        .flex_none()
        .flex()
        .items_center()
        .gap(px(4.))
        .px(px(10.))
        .py(px(6.))
        .border_t_1()
        .border_color(t.border)
        .child(div().flex_1().min_w_0().child(menu))
        .child(
            IconButton::new("settings", Icon::Settings, "Settings")
                .glyph(14.)
                .width(28.)
                .on_click(|_, window, cx| keymap::dispatch("settings.toggle", window, cx)),
        )
        .into_any_element()
}

/// The first letters of the first and last words of the name, else the email's first.
fn initials(name: Option<&str>, email: Option<&str>) -> String {
    let words: Vec<&str> = name.unwrap_or_default().split_whitespace().collect();
    let first = |word: &str| {
        word.chars()
            .next()
            .map(|letter| letter.to_uppercase().to_string())
            .unwrap_or_default()
    };
    match words.as_slice() {
        [] => email.map(first).unwrap_or_default(),
        [only] => first(only),
        [head, .., tail] => first(head) + &first(tail),
    }
}

fn sign_in(bench: &Workbench, cx: &mut Context<Workbench>) -> impl IntoElement {
    let t = Tokens::get(cx);
    let (copy, button) = if bench.signing_in {
        (
            "Finish signing in in your browser.",
            Button::new("sign-in", "Cancel", Variant::Outline)
                .on_click(cx.listener(|bench, _, _, _| bench.cancel_sign_in())),
        )
    } else {
        (
            "Sign in to reach this host from your phone and other devices.",
            Button::new("sign-in", "Sign in", Variant::Primary)
                .icon(Icon::LogIn)
                .on_click(cx.listener(|bench, _, _, cx| {
                    bench.sign_in();
                    cx.notify();
                })),
        )
    };
    div()
        .flex_none()
        .mx(px(10.))
        .mb(px(10.))
        .p(px(8.))
        .flex()
        .flex_col()
        .gap(px(8.))
        .rounded(RADIUS_2)
        .bg(gpui_kit::Hsla {
            a: 0.04,
            ..t.text_primary
        })
        .child(
            div()
                .text_size(px(12.))
                .line_height(px(12. * 1.4))
                .text_color(t.text_secondary)
                .child(copy),
        )
        .child(button)
}
