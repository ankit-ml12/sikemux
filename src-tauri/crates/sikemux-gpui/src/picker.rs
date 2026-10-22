//! ⌘O, ⌘⇧S and ⌘⇧O: the projects with the folders under the person's project
//! roots, the SSH hosts, or both.

use std::path::Path;

use gpui_kit::{Context, Window};
use serde_json::{json, Value};

use crate::chat::tilde;
use crate::core::Ask;
use crate::icons::Icon;
use crate::palette::{Mark, Row, Spec};
use crate::workbench::{Strip, Workbench};

#[derive(Clone, Default, PartialEq, gpui::Action)]
#[action(name = "project.open")]
pub struct OpenProject;

#[derive(Clone, Default, PartialEq, gpui::Action)]
#[action(name = "ssh.open")]
pub struct OpenHost;

#[derive(Clone, Default, PartialEq, gpui::Action)]
#[action(name = "session.open")]
pub struct OpenPlace;

const PROJECT: &str = "project:";
const FOLDER: &str = "folder:";
const HOST: &str = "host:";

/// What happens once a place is picked, past bringing it forward.
#[derive(Clone, Copy, PartialEq)]
pub enum ThenPicked {
    Open,
    StartChat,
}

#[derive(Clone, Copy, PartialEq)]
pub enum Picking {
    Projects,
    Hosts,
    Both,
}

impl Picking {
    const ALL: [Picking; 3] = [Picking::Projects, Picking::Hosts, Picking::Both];

    fn id(self) -> &'static str {
        match self {
            Picking::Projects => "project.open",
            Picking::Hosts => "ssh.open",
            Picking::Both => "session.open",
        }
    }

    fn projects(self) -> bool {
        self != Picking::Hosts
    }

    fn hosts(self) -> bool {
        self != Picking::Projects
    }
}

fn listed(value: Option<&Value>) -> impl Iterator<Item = &Value> {
    value.and_then(Value::as_array).into_iter().flatten()
}

fn host_line(host: &Value) -> String {
    let alias = host["alias"].as_str().unwrap_or_default();
    let user = host["user"]
        .as_str()
        .map(|user| format!("{user}@"))
        .unwrap_or_default();
    let port = host["port"]
        .as_u64()
        .filter(|port| *port != 22)
        .map(|port| format!(":{port}"))
        .unwrap_or_default();
    format!("{user}{}{port}", host["hostname"].as_str().unwrap_or(alias))
}

impl Workbench {
    pub fn toggle_picker(&mut self, picking: Picking, window: &mut Window, cx: &mut Context<Self>) {
        self.open_picker(picking, ThenPicked::Open, window, cx);
    }

    /// The project picker, doing `then` with the project picked.
    pub fn pick_project(&mut self, then: ThenPicked, window: &mut Window, cx: &mut Context<Self>) {
        self.open_picker(Picking::Projects, then, window, cx);
    }

    fn open_picker(
        &mut self,
        picking: Picking,
        then: ThenPicked,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if picking.projects() {
            let roots = self.preferences().project_roots;
            if !roots.is_empty() {
                let _ = self.asks.send(Ask::Answer {
                    key: "pickerFolders",
                    method: "projectRoots",
                    params: json!({ "roots": roots }),
                });
            }
        }
        if picking.hosts() {
            let _ = self.asks.send(Ask::Answer {
                key: "sshHosts",
                method: "sshHosts",
                params: json!({}),
            });
        }
        let placeholder = match (picking, then) {
            (_, ThenPicked::StartChat) => "start a chat in a project…",
            (Picking::Projects, _) => "open a project…",
            (Picking::Hosts, _) => "connect to a host…",
            (Picking::Both, _) => "open a project or host…",
        };
        let spec = Spec::new(
            picking.id(),
            placeholder,
            self.places(picking),
            move |row, bench, window, cx| {
                bench.open_place(&row.key, then, window, cx);
            },
        )
        .empty(if picking == Picking::Hosts {
            "no hosts in ~/.ssh/config match"
        } else {
            "no projects match"
        });
        self.toggle_palette(spec, window, cx);
    }

    /// Lists what the core answered since the picker opened.
    pub fn picker_answered(&mut self, cx: &mut Context<Self>) {
        let Some(picking) = Picking::ALL.into_iter().find(|picking| {
            self.palette
                .as_ref()
                .is_some_and(|open| open.id() == picking.id())
        }) else {
            return;
        };
        let rows = self.places(picking);
        if let Some(palette) = self.palette.as_mut() {
            palette.refill(rows, cx);
        }
        self.parts.redraw(&self.parts.finder, cx);
    }

    fn places(&self, picking: Picking) -> Vec<Row> {
        let mut rows = Vec::new();
        if picking.projects() {
            let projects = self.projects();
            for project in projects {
                let path = tilde(&project.path);
                rows.push(
                    Row::new(format!("{PROJECT}{}", project.id), project.name.clone())
                        .meta(path.clone())
                        .mark(Mark::Icon(Icon::Folder))
                        .group("Projects")
                        .also(&path),
                );
            }
            let added = |path: &str| {
                projects
                    .iter()
                    .any(|project| project.path == Path::new(path))
            };
            for folder in listed(self.read("pickerFolders")) {
                let (Some(name), Some(path)) = (folder["name"].as_str(), folder["path"].as_str())
                else {
                    continue;
                };
                if added(path) {
                    continue;
                }
                let shown = tilde(Path::new(path));
                rows.push(
                    Row::new(format!("{FOLDER}{path}"), name.to_owned())
                        .meta(shown.clone())
                        .mark(Mark::Icon(Icon::FolderPlus))
                        .group("Folders")
                        .also(&shown),
                );
            }
        }
        if picking.hosts() {
            for host in listed(self.read("sshHosts")) {
                let Some(alias) = host["alias"].as_str() else {
                    continue;
                };
                let line = host_line(host);
                rows.push(
                    Row::new(format!("{HOST}{alias}"), alias.to_owned())
                        .meta(line.clone())
                        .mark(Mark::Icon(Icon::Server))
                        .group("SSH hosts")
                        .also(&line),
                );
            }
        }
        rows
    }

    pub(crate) fn open_place(
        &mut self,
        key: &str,
        then: ThenPicked,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(project) = key.strip_prefix(PROJECT) {
            self.activate_project(project);
            if then == ThenPicked::StartChat {
                self.start_chat_in(project, window, cx);
            }
        } else if let Some(path) = key.strip_prefix(FOLDER) {
            let added = crate::core::request(&self.asks, "addProject", json!({ "path": path }));
            cx.spawn_in(window, async move |bench, cx| {
                let project = match added.await {
                    Ok(project) => project["id"].as_str().unwrap_or_default().to_owned(),
                    Err(error) => {
                        return eprintln!("sikemux: the core would not add the project: {error}")
                    }
                };
                let _ = bench.update_in(cx, |bench, window, cx| {
                    bench.activate_project(&project);
                    if then == ThenPicked::StartChat {
                        bench.start_chat_in(&project, window, cx);
                    }
                });
            })
            .detach();
        } else if let Some(alias) = key.strip_prefix(HOST) {
            self.show_strip(&Strip::Host(alias.to_owned()));
        }
    }
}
