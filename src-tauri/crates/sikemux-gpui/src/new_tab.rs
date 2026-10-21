//! New…: every kind of tab the window can open, each by the command that opens
//! it, so a kind joins the list once the window can do its command.

use gpui_kit::{Context, Window};

use crate::icons::Icon;
use crate::keymap;
use crate::palette::{Mark, Row, Spec};
use crate::workbench::{Strip, Workbench};

#[derive(Clone, Default, PartialEq, gpui::Action)]
#[action(name = "palette.newTab")]
pub struct NewTab;

struct Choice {
    command: &'static str,
    label: &'static str,
    detail: &'static str,
    icon: Icon,
    /// Opens only in a project, not on an SSH host.
    in_project: bool,
}

const CHOICES: [Choice; 8] = [
    Choice {
        command: "terminal.new",
        label: "Terminal",
        detail: "A new shell",
        icon: Icon::Terminal,
        in_project: false,
    },
    Choice {
        command: "agent.choose",
        label: "Agent",
        detail: "Start or resume an agent",
        icon: Icon::Agent,
        in_project: true,
    },
    Choice {
        command: "browser.tabNew",
        label: "Browser",
        detail: "A page on the agent's desk",
        icon: Icon::Globe,
        in_project: true,
    },
    Choice {
        command: "simulator.new",
        label: "iOS Simulator",
        detail: "Drive an iPhone on the agent's desk",
        icon: Icon::Phone,
        in_project: true,
    },
    Choice {
        command: "palette.files",
        label: "Files",
        detail: "Open a project file",
        icon: Icon::Editor,
        in_project: true,
    },
    Choice {
        command: "window.git",
        label: "Git",
        detail: "Changes, branches, remotes and stashes",
        icon: Icon::Git,
        in_project: true,
    },
    Choice {
        command: "search.global",
        label: "Search",
        detail: "Search across the project",
        icon: Icon::Search,
        in_project: true,
    },
    Choice {
        command: "window.board",
        label: "Agent board",
        detail: "What the project's agents are doing",
        icon: Icon::Board,
        in_project: true,
    },
];

impl Workbench {
    pub fn toggle_new_tab(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let in_project = matches!(self.strip(), Some(Strip::Project(_)));
        let rows = CHOICES
            .iter()
            .filter(|choice| cx.build_action(choice.command, None).is_ok())
            .map(|choice| {
                let shut = choice.in_project && !in_project;
                Row::new(choice.command, choice.label)
                    .meta(if shut {
                        "open a project first"
                    } else {
                        choice.detail
                    })
                    .keys(keymap::hint(choice.command, cx))
                    .mark(Mark::Icon(choice.icon))
                    .disabled(shut)
            })
            .collect();
        let spec = Spec::new(
            "palette.newTab",
            "open a new tab…",
            rows,
            |row, _, window, cx| {
                keymap::dispatch(&row.key, window, cx);
            },
        )
        .width(440.);
        self.toggle_palette(spec, window, cx);
    }
}
