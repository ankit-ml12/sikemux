use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::f32::consts::FRAC_PI_2;
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use gpui_kit::base::Button as BaseButton;
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::{
    div, px, radians, uniform_list, AnyElement, ClickEvent, Context, FontWeight,
    InteractiveElement, IntoElement, ParentElement, SharedString, Styled, Transformation,
};
use serde_json::{json, Value};

use crate::core::Target;
use crate::hosts::Place;
use crate::icons::Icon;
use crate::theme::{self, Tokens};
use crate::ui::{self, IconButton};
use crate::workbench::Workbench;

/// One thing in a folder, as the core listed it: folders first, then by name.
#[derive(Clone, PartialEq)]
pub struct Entry {
    pub name: String,
    pub dir: bool,
    pub ignored: bool,
}

/// A row the tree draws: an entry at its depth under the project.
pub struct Row {
    pub path: PathBuf,
    pub depth: usize,
    pub entry: Entry,
    pub open: bool,
}

/// The project's folders as far as the person opened them. The listings are what the core
/// last answered for each; nothing here decides what a folder holds.
#[derive(Default)]
pub struct FileTree {
    pub root: Option<PathBuf>,
    place: Place,
    listed: HashMap<PathBuf, Vec<Entry>>,
    /// The pages of each listing on its way, shown only once the whole folder is in.
    incoming: HashMap<PathBuf, Vec<Entry>>,
    /// The folders being listed, and those that changed meanwhile and are listed once more after.
    asked: HashSet<PathBuf>,
    again: HashSet<PathBuf>,
    pub expanded: HashSet<PathBuf>,
    pub selected_dir: Option<PathBuf>,
    /// What git says of each changed file and the folders holding it, by absolute path.
    pub marks: HashMap<PathBuf, (Option<char>, Mark)>,
    /// The rows as last laid out, until a listing or an opened folder changes them.
    rows: RefCell<Option<Rc<Vec<Row>>>>,
}

impl FileTree {
    /// Starts over on `root`, the project in front, unless it already shows it from that host.
    pub fn show(&mut self, root: Option<PathBuf>, core: &Target) {
        if root == self.root && core.place() == &self.place {
            return;
        }
        *self = FileTree {
            root: root.clone(),
            place: core.place().clone(),
            ..FileTree::default()
        };
        if let Some(root) = root {
            self.relist(vec![root], core);
        }
    }

    /// Lists every shown folder again from scratch, for when answers on their way were lost.
    pub fn relist_shown(&mut self, core: &Target) {
        self.asked.clear();
        self.again.clear();
        self.incoming.clear();
        self.changed(None, core);
    }

    /// Asks the core to list `dirs`; one already being listed is listed once more when it is in.
    fn relist(&mut self, dirs: Vec<PathBuf>, core: &Target) {
        let fresh: Vec<PathBuf> = dirs
            .into_iter()
            .filter(|dir| {
                if self.asked.contains(dir) {
                    self.again.insert(dir.clone());
                    return false;
                }
                self.asked.insert(dir.clone());
                true
            })
            .collect();
        list(core, &fresh, None);
    }

    /// Takes in a `files.list` answer, asking for the next page of any folder it left short.
    pub fn listed(&mut self, answer: &Value, core: &Target) {
        for folder in answer.as_array().into_iter().flatten() {
            let Some(dir) = folder["dir"].as_str().map(PathBuf::from) else {
                continue;
            };
            if folder["error"].is_string() {
                self.asked.remove(&dir);
                self.again.remove(&dir);
                self.incoming.remove(&dir);
                continue;
            }
            let entries: Vec<Entry> = folder["entries"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|entry| {
                    Some(Entry {
                        name: entry["name"].as_str()?.to_owned(),
                        dir: entry["dir"].as_bool().unwrap_or(false),
                        ignored: entry["ignored"].as_bool().unwrap_or(false),
                    })
                })
                .collect();
            let total = folder["total"].as_u64().unwrap_or(0) as usize;
            let got = entries.len();
            let incoming = self.incoming.entry(dir.clone()).or_default();
            incoming.extend(entries);
            if got > 0 && incoming.len() < total {
                list_from(core, &dir, incoming.len());
                continue;
            }
            let whole = self.incoming.remove(&dir).unwrap_or_default();
            if self.listed.get(&dir) != Some(&whole) {
                self.listed.insert(dir.clone(), whole);
                self.rows.take();
            }
            self.asked.remove(&dir);
            if self.again.remove(&dir) {
                self.relist(vec![dir], core);
            }
        }
    }

    /// Lists again the folders shown that `paths` (relative to the project) could have changed,
    /// or every shown folder when the core did not say what moved.
    pub fn changed(&mut self, paths: Option<&[String]>, core: &Target) {
        let Some(root) = self.root.clone() else {
            return;
        };
        let shown: Vec<PathBuf> = std::iter::once(root.clone())
            .chain(self.expanded.iter().cloned())
            .collect();
        let stale: Vec<PathBuf> = match paths
            .filter(|paths| !paths.is_empty() && !paths.iter().any(|path| path == ".git"))
        {
            None => shown,
            Some(paths) => {
                let moved: Vec<PathBuf> = paths.iter().map(|path| root.join(path)).collect();
                shown
                    .into_iter()
                    .filter(|dir| {
                        moved.iter().any(|path| {
                            path.parent() == Some(dir.as_path())
                                || dir.starts_with(path)
                                || (path.file_name().is_some_and(|name| name == ".gitignore")
                                    && path.parent().is_some_and(|folder| dir.starts_with(folder)))
                        })
                    })
                    .collect()
            }
        };
        if !stale.is_empty() {
            self.relist(stale, core);
        }
    }

    pub fn toggle(&mut self, dir: &Path, core: &Target) {
        self.selected_dir = Some(dir.to_owned());
        self.rows.take();
        if !self.expanded.remove(dir) {
            self.expanded.insert(dir.to_owned());
            if !self.listed.contains_key(dir) {
                self.relist(vec![dir.to_owned()], core);
            }
        }
    }

    pub fn is_dir(&self, path: &Path) -> bool {
        let (Some(parent), Some(name)) = (path.parent(), path.file_name()) else {
            return false;
        };
        self.listed
            .get(parent)
            .into_iter()
            .flatten()
            .any(|entry| entry.dir && entry.name == name.to_string_lossy())
    }

    pub fn collapse_all(&mut self) {
        self.expanded.clear();
        self.rows.take();
    }

    /// Takes in what git says of the project's files.
    pub fn mark(&mut self, overview: &Value) {
        if let Some(root) = &self.root {
            self.marks = git_marks(root, overview);
        }
    }

    /// The rows to draw: the project's entries, and those of every open folder under its own.
    pub fn rows(&self) -> Rc<Vec<Row>> {
        self.rows
            .borrow_mut()
            .get_or_insert_with(|| {
                let mut rows = Vec::new();
                if let Some(root) = &self.root {
                    self.walk(root, 0, &mut rows);
                }
                Rc::new(rows)
            })
            .clone()
    }

    fn walk(&self, dir: &Path, depth: usize, rows: &mut Vec<Row>) {
        for entry in self.listed.get(dir).into_iter().flatten() {
            let path = dir.join(&entry.name);
            let open = entry.dir && self.expanded.contains(&path);
            rows.push(Row {
                path: path.clone(),
                depth,
                entry: entry.clone(),
                open,
            });
            if open {
                self.walk(&path, depth + 1, rows);
            }
        }
    }
}

/// How git sees a file, as one letter and the colour class it is drawn with.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Mark {
    Modified,
    Renamed,
    Added,
    Deleted,
    Untracked,
    Conflicted,
}

impl Mark {
    fn of(code: &str) -> Option<(char, Mark)> {
        Some(match code {
            "?" => ('U', Mark::Untracked),
            "U" => ('U', Mark::Conflicted),
            "A" => ('A', Mark::Added),
            "D" => ('D', Mark::Deleted),
            "R" => ('R', Mark::Renamed),
            "C" => ('C', Mark::Renamed),
            "T" => ('T', Mark::Modified),
            "M" => ('M', Mark::Modified),
            " " | "" => return None,
            _ => (code.chars().next()?, Mark::Modified),
        })
    }
}

/// Every changed file's letter and mark, and every folder holding changes with the most
/// telling mark below it, by absolute path.
fn git_marks(root: &Path, overview: &Value) -> HashMap<PathBuf, (Option<char>, Mark)> {
    let mut marks: HashMap<PathBuf, (Option<char>, Mark)> = HashMap::new();
    for file in overview["status"]["files"].as_array().into_iter().flatten() {
        let Some(path) = file["path"].as_str() else {
            continue;
        };
        let sides = [
            file["index"].as_str().unwrap_or(" "),
            file["worktree"].as_str().unwrap_or(" "),
        ];
        let Some((letter, mark)) = sides
            .into_iter()
            .filter_map(Mark::of)
            .max_by_key(|(_, mark)| *mark)
        else {
            continue;
        };
        let full = root.join(path.trim_end_matches('/'));
        let mut folder = full.parent();
        marks.insert(full.clone(), (Some(letter), mark));
        while let Some(dir) = folder.filter(|dir| dir.starts_with(root) && *dir != root) {
            let held = marks.entry(dir.to_owned()).or_insert((None, mark));
            if held.0.is_none() && mark > held.1 {
                held.1 = mark;
            }
            folder = dir.parent();
        }
    }
    marks
}

fn list(core: &Target, dirs: &[PathBuf], from: Option<usize>) {
    let dirs: Vec<String> = dirs
        .iter()
        .map(|dir| dir.to_string_lossy().into_owned())
        .collect();
    for batch in dirs.chunks(512) {
        let params = match from {
            Some(from) => json!({ "dirs": batch, "from": from }),
            None => json!({ "dirs": batch }),
        };
        core.answer("files.list", "files.list", params);
    }
}

fn list_from(core: &Target, dir: &Path, from: usize) {
    list(core, &[dir.to_owned()], Some(from));
}

/// The tab rail's Files page: the project's name and what can be made in it, then its tree.
pub fn page(bench: &Workbench, cx: &mut Context<Workbench>) -> impl IntoElement {
    let t = Tokens::get(cx).clone();
    let name = bench
        .files
        .root
        .as_ref()
        .and_then(|root| root.file_name())
        .map_or_else(
            || "files".to_owned(),
            |name| name.to_string_lossy().into_owned(),
        );
    let count = bench.files.rows().len();
    let action = |id: &'static str, glyph: Icon, label: &'static str| {
        IconButton::new(id, glyph, label).small()
    };
    div()
        .flex_1()
        .min_h_0()
        .flex()
        .flex_col()
        .child(
            div()
                .flex_none()
                .min_h(px(34.))
                .pt(px(6.))
                .pr(px(6.))
                .pb(px(6.))
                .pl(px(10.))
                .flex()
                .items_center()
                .justify_between()
                .gap(px(8.))
                .child(
                    ui::text(name.to_uppercase(), 10., FontWeight::SEMIBOLD)
                        .letter_spacing(0.08)
                        .text_color(t.text_tertiary),
                )
                .child(
                    div()
                        .flex()
                        .gap(px(2.))
                        .opacity(0.6)
                        .child(action("files-find", Icon::Search, "Find a file").on_click(
                            cx.listener(|bench, _, window, cx| {
                                bench.toggle_finder(window, cx);
                                cx.notify();
                            }),
                        ))
                        .child(action("files-new", Icon::FilePlus, "New file"))
                        .child(action("files-new-folder", Icon::FolderPlus, "New folder"))
                        .child(
                            action("files-collapse", Icon::CollapseAll, "Collapse folders")
                                .on_click(cx.listener(|bench, _, _, cx| {
                                    bench.files.collapse_all();
                                    cx.notify();
                                })),
                        ),
                ),
        )
        .child(
            uniform_list(
                "file-rows",
                count,
                cx.processor(|bench: &mut Workbench, range: Range<usize>, _window, cx| {
                    let front = bench.front_file();
                    let rows = bench.files.rows();
                    rows[range.start.min(rows.len())..range.end.min(rows.len())]
                        .iter()
                        .map(|row| row_view(row, &bench.files.marks, front.as_deref(), cx))
                        .collect::<Vec<_>>()
                }),
            )
            .flex_1()
            .px(px(6.))
            .pb(px(4.)),
        )
}

fn row_view(
    row: &Row,
    marks: &HashMap<PathBuf, (Option<char>, Mark)>,
    front: Option<&Path>,
    cx: &mut Context<Workbench>,
) -> AnyElement {
    let t = Tokens::get(cx).clone();
    let mark = marks.get(&row.path).copied();
    let colour = |mark: Mark| match mark {
        Mark::Modified => t.git_modified,
        Mark::Untracked => t.git_untracked,
        Mark::Added => t.git_added,
        Mark::Deleted => t.git_deleted,
        Mark::Renamed => t.git_renamed,
        Mark::Conflicted => t.danger,
    };
    let active = !row.entry.dir && front == Some(row.path.as_path());
    let ink = match mark {
        _ if active => t.text_primary,
        Some((_, mark)) => colour(mark),
        None if row.entry.ignored => t.text_disabled,
        None => t.text_secondary,
    };
    let pad = 4. + row.depth as f32 * 13.;
    let path = row.path.clone();
    let lead = if row.entry.dir {
        div()
            .flex()
            .items_center()
            .gap(px(5.))
            .child(
                ui::icon(Icon::Chevron, 11.)
                    .when(row.open, |chevron| {
                        chevron.with_transformation(Transformation::rotate(radians(FRAC_PI_2)))
                    })
                    .text_color(t.text_disabled),
            )
            .child(ui::icon(Icon::Folder, 17.).text_color(t.text_tertiary))
            .into_any_element()
    } else {
        let glyph = match crate::file_icons::icon(&row.entry.name, cx) {
            Some(icon) => gpui_kit::img(icon)
                .size(px(16.))
                .flex_none()
                .into_any_element(),
            None => ui::icon(Icon::File, 16.)
                .text_color(t.text_tertiary)
                .into_any_element(),
        };
        ui::mark_box(22., glyph).into_any_element()
    };
    let tail = match mark {
        Some((Some(letter), mark)) if !row.entry.dir => Some(
            ui::text(letter.to_string(), 10., FontWeight::BOLD)
                .ml_auto()
                .pl(px(8.))
                .opacity(0.8)
                .text_color(colour(mark))
                .into_any_element(),
        ),
        Some((_, mark)) if row.entry.dir => Some(
            ui::text("●", 7., FontWeight::NORMAL)
                .ml_auto()
                .pl(px(8.))
                .text_color(colour(mark))
                .into_any_element(),
        ),
        _ => None,
    };
    let drawn = BaseButton::new(SharedString::from(format!("file-{}", row.path.display())))
        .accessibility_label(SharedString::from(row.entry.name.clone()))
        .w_full()
        .h(px(ROW_HEIGHT))
        .pl(px(if row.entry.dir { pad } else { pad + 13. }))
        .pr(px(8.))
        .flex()
        .items_center()
        .justify_start()
        .gap(px(5.))
        .rounded(theme::RADIUS_2)
        .text_size(crate::theme::ui_text(12.))
        .font_weight(FontWeight::MEDIUM)
        .text_color(ink)
        .when(active, |row| row.bg(t.surface_chosen))
        .when(!active, |row| {
            row.hover(move |style| style.bg(t.surface_hover).text_color(t.text_primary))
        })
        .on_click(cx.listener(move |bench, event: &ClickEvent, _, cx| {
            if bench.files.expanded.contains(&path) || bench.files.is_dir(&path) {
                if event.click_count() == 1 {
                    bench.files.toggle(&path, &bench.target());
                }
            } else if event.click_count() == 2 {
                bench.open_file(&path);
            } else {
                bench.preview_file(&path);
            }
            cx.notify();
        }))
        .child(lead)
        .child(
            div()
                .min_w_0()
                .overflow_hidden()
                .text_ellipsis()
                .whitespace_nowrap()
                .child(row.entry.name.clone()),
        )
        .children(tail)
        .into_any_element();
    crate::drag::tree_row(drawn, &row.path, row.entry.dir, &cx.entity(), cx)
}

const ROW_HEIGHT: f32 = 23.;
