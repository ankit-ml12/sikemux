//! A board's columns and cards, and cards moved between them.

use super::*;

impl Jira {
    pub(super) fn board(
        &mut self,
        host: &mut Host,
        id: u64,
        site: &str,
        window: &mut Window,
        cx: &mut Context<PluginPane>,
    ) -> AnyElement {
        let t = Tokens::get(cx).clone();
        let people: Vec<&String> = self
            .listing
            .picked
            .iter()
            .filter(|who| *who != "me" && *who != "none")
            .collect();
        let params = json!({
            "id": id,
            "site": site_of(site),
            "people": people,
            "mine": self.listing.picked.contains("me"),
            "unassigned": self.listing.picked.contains("none"),
        });
        let page = match host.view("boardView", params, cx).read::<BoardPage>() {
            Ok(Some(page)) => {
                if !self
                    .listing
                    .last_board
                    .as_ref()
                    .is_some_and(|last| Rc::ptr_eq(last, &page))
                {
                    self.listing.moves.clear();
                    self.listing.last_board = Some(page.clone());
                }
                page
            }
            Ok(None) => match &self.listing.last_board {
                Some(last) => last.clone(),
                None => return crate::plugin_pane::loading(cx),
            },
            Err(failure) => return crate::plugin_pane::failure(&failure, cx),
        };
        let board = &page.board;
        let find = field(
            &mut self.card_find,
            &mut self.typing,
            "Find a card",
            false,
            (None, None),
            window,
            cx,
        );
        let wanted = words(&self.card_find, cx);
        let hidden = if self.listing.every_face {
            0
        } else {
            page.people.len().saturating_sub(FACES)
        };
        let faces: Vec<_> = page
            .people
            .iter()
            .enumerate()
            .filter(|(index, person)| {
                *index < page.people.len() - hidden
                    || self.listing.picked.contains(&person.account_id)
            })
            .map(|(index, person)| {
                let who = person.account_id.clone();
                let picked = self.listing.picked.contains(&who);
                div()
                    .id(("jira-face", index))
                    .rounded_full()
                    .border_2()
                    .border_color(if picked {
                        t.accent
                    } else {
                        Hsla::transparent_black()
                    })
                    .cursor_pointer()
                    .tooltip(ui::tooltip(format!("{} · {}", person.name, person.count)))
                    .child(host.face(&person.name, person.avatar_url.as_deref(), 22., cx))
                    .on_click(listen::<Jira, _>(cx, move |jira, _, _, _, cx| {
                        if !jira.listing.picked.remove(&who) {
                            jira.listing.picked.insert(who.clone());
                        }
                        cx.notify();
                    }))
            })
            .collect();
        let narrowed = !self.listing.picked.is_empty() || !wanted.is_empty();
        let filters = div()
            .flex()
            .flex_wrap()
            .items_center()
            .gap(px(8.))
            .child(
                div()
                    .w(px(200.))
                    .child(ui::Field::new(&find).icon(Icon::Search)),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(2.))
                    .children(faces)
                    .when(hidden > 0, |faces| {
                        faces.child(
                            Button::new("jira-more-faces", format!("+{hidden}"), Variant::Outline)
                                .tooltip(format!("Show {hidden} more"))
                                .on_click(listen::<Jira, _>(cx, |jira, _, _, _, cx| {
                                    jira.listing.every_face = true;
                                    cx.notify();
                                })),
                        )
                    }),
            )
            .child(
                Toggle::new(
                    "jira-only-mine",
                    "Only my issues",
                    self.listing.picked.contains("me"),
                )
                .on_click(listen::<Jira, _>(cx, |jira, _, _, _, cx| {
                    if !jira.listing.picked.remove("me") {
                        jira.listing.picked.insert("me".into());
                    }
                    cx.notify();
                })),
            )
            .when(page.unassigned > 0, |filters| {
                filters.child(
                    Toggle::new(
                        "jira-nobody",
                        format!("Unassigned {}", page.unassigned),
                        self.listing.picked.contains("none"),
                    )
                    .on_click(listen::<Jira, _>(cx, |jira, _, _, _, cx| {
                        if !jira.listing.picked.remove("none") {
                            jira.listing.picked.insert("none".into());
                        }
                        cx.notify();
                    })),
                )
            })
            .when(narrowed, |filters| {
                filters.child(
                    Button::new("jira-clear", "Clear", Variant::Outline).on_click(
                        listen::<Jira, _>(cx, |jira, _, _, window, cx| {
                            jira.listing.picked.clear();
                            if let Some(find) = &jira.card_find {
                                find.update(cx, |find, cx| find.set_value("", window, cx));
                            }
                            cx.notify();
                        }),
                    ),
                )
            });
        let sprint = board.sprint.as_ref().map(|sprint| {
            let mut said = sprint.name.clone();
            if let Some(left) = sprint_left(sprint.end.as_deref()) {
                said.push_str(&format!(" · {left}"));
            }
            ui::meta(said, cx)
        });
        let goal = board
            .sprint
            .as_ref()
            .and_then(|sprint| sprint.goal.clone())
            .map(|goal| ui::text(goal, 12., FontWeight::NORMAL).text_color(t.text_secondary));
        let head = div()
            .px(px(14.))
            .pt(px(12.))
            .pb(px(10.))
            .flex()
            .flex_col()
            .gap(px(8.))
            .border_b_1()
            .border_color(t.border)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(
                        ui::text(board.name.clone(), 15., FontWeight::SEMIBOLD)
                            .text_color(t.text_primary)
                            .min_w_0()
                            .truncate(),
                    )
                    .child(tag(&board.kind, cx))
                    .children(sprint.map(|sprint| sprint.min_w_0().truncate()))
                    .child(div().flex_1())
                    .child(
                        IconButton::new("jira-board-refresh", Icon::Refresh, "Refresh the board")
                            .small()
                            .on_click(listen::<Jira, _>(cx, |_, host, _, _, _| {
                                host.read_again(Some("boardView"))
                            })),
                    ),
            )
            .children(goal)
            .when(board.truncated, |head| {
                head.child(ui::meta(
                    "Showing the first 300 issues. Pick people or narrow the board in Jira to see the rest.",
                    cx,
                ))
            })
            .child(filters);
        if board.kind == "scrum" && board.sprint.is_none() {
            return div()
                .size_full()
                .flex()
                .flex_col()
                .child(head)
                .child(crate::plugin_pane::empty(
                    "No sprint is running on this board.",
                    cx,
                ))
                .into_any_element();
        }
        let shape = Shape {
            columns: &board.columns,
            wanted: &wanted,
            site,
        };
        let body = if page.lanes.is_empty() {
            self.columns(host, &shape, &board.columns, "all", cx)
                .into_any_element()
        } else {
            let mut lanes = div().flex().flex_col().gap(px(10.));
            for lane in &page.lanes {
                let id = lane.id.clone();
                let folded = self.listing.folded.contains(&id);
                let name = lane
                    .person
                    .as_ref()
                    .map_or("Unassigned".to_owned(), |person| person.name.clone());
                let items = match lane.count {
                    1 => "1 work item".to_owned(),
                    count => format!("{count} work items"),
                };
                let avatar = lane
                    .person
                    .as_ref()
                    .and_then(|person| person.avatar_url.as_deref());
                lanes = lanes.child(
                    div()
                        .id(SharedString::from(format!("jira-lane-{id}")))
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .cursor_pointer()
                        .child(chevron(!folded, cx))
                        .child(host.face(&name, avatar, 20., cx))
                        .child(
                            ui::text(name.clone(), 12.5, FontWeight::SEMIBOLD)
                                .text_color(t.text_primary),
                        )
                        .child(ui::meta(format!("({items})"), cx))
                        .on_click(listen::<Jira, _>(cx, move |jira, _, _, _, cx| {
                            if !jira.listing.folded.remove(&id) {
                                jira.listing.folded.insert(id.clone());
                            }
                            cx.notify();
                        })),
                );
                if !folded {
                    lanes = lanes.child(self.columns(host, &shape, &lane.columns, &lane.id, cx));
                }
            }
            lanes.into_any_element()
        };
        div()
            .size_full()
            .flex()
            .flex_col()
            .child(head)
            .child(
                div()
                    .id("jira-board")
                    .flex_1()
                    .min_h_0()
                    .overflow_scroll()
                    .p(px(12.))
                    .flex()
                    .items_start()
                    .child(div().flex_none().child(body)),
            )
            .into_any_element()
    }

    /// A lane's columns with their cards, each card where it was moved to here.
    fn columns(
        &self,
        host: &Host,
        shape: &Shape,
        columns: &[Column],
        lane: &str,
        cx: &mut Context<PluginPane>,
    ) -> Div {
        let t = Tokens::get(cx).clone();
        let mut row = div().flex().gap(px(10.));
        for (at, (column, cards)) in columns
            .iter()
            .zip(with_moves(columns, &self.listing.moves))
            .enumerate()
        {
            let cards: Vec<&IssueSummary> = cards
                .into_iter()
                .filter(|issue| {
                    shape.wanted.is_empty()
                        || issue.key.to_lowercase().contains(shape.wanted)
                        || issue.summary.to_lowercase().contains(shape.wanted)
                })
                .collect();
            let mut list = div()
                .w(px(250.))
                .flex_none()
                .p(px(6.))
                .flex()
                .flex_col()
                .gap(px(6.))
                .rounded(RADIUS_2)
                .bg(t.surface_sunken)
                .child(
                    div()
                        .px(px(4.))
                        .py(px(2.))
                        .flex()
                        .items_center()
                        .gap(px(6.))
                        .child(ui::caps(&column.name, cx))
                        .child(ui::meta(cards.len().to_string(), cx)),
                );
            for issue in cards {
                list = list.child(self.card(host, shape, issue, at, lane, cx));
            }
            row = row.child(list);
        }
        row
    }

    fn card(
        &self,
        host: &Host,
        shape: &Shape,
        issue: &IssueSummary,
        at: usize,
        lane: &str,
        cx: &mut Context<PluginPane>,
    ) -> AnyElement {
        let t = Tokens::get(cx).clone();
        let key = issue.key.clone();
        let chosen = self.open_key() == Some(issue.key.as_str());
        let moving = self.listing.moves.contains_key(&issue.key);
        let pane = cx.weak_entity();
        let targets: Vec<(String, Vec<String>)> = shape
            .columns
            .iter()
            .map(|column| (column.name.clone(), column.status_ids.clone()))
            .collect();
        let (menu_key, site) = (issue.key.clone(), shape.site.to_owned());
        let chips = (issue.parent.is_some() || issue.due.is_some()).then(|| {
            div()
                .flex()
                .flex_wrap()
                .gap(px(4.))
                .children(issue.parent.as_ref().map(|parent| epic_chip(parent, cx)))
                .children(
                    issue
                        .due
                        .as_ref()
                        .map(|due| due_chip(due, issue.status_category == "done", false, cx)),
                )
        });
        let who = issue
            .assignee
            .as_ref()
            .map_or("Unassigned", |person| person.name.as_str());
        let avatar = issue
            .assignee
            .as_ref()
            .and_then(|person| person.avatar_url.as_deref());
        div()
            .id(SharedString::from(format!(
                "jira-card-{lane}-{}",
                issue.key
            )))
            .p(px(8.))
            .flex()
            .flex_col()
            .gap(px(6.))
            .rounded(RADIUS_2)
            .border_1()
            .border_color(if chosen { t.border_selected } else { t.border })
            .bg(if chosen {
                t.surface_chosen
            } else {
                t.surface_raised
            })
            .when(moving, |card| card.opacity(0.6))
            .cursor_pointer()
            .hover(move |card| card.border_color(t.border_strong))
            .child(
                ui::text(issue.summary.clone(), 12.5, FontWeight::NORMAL)
                    .text_color(t.text_primary)
                    .whitespace_normal()
                    .text_ellipsis()
                    .line_clamp(3),
            )
            .children(chips)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .child(type_mark(issue.issue_type.as_deref(), cx))
                    .child(ui::mono(issue.key.clone(), 11., cx).text_color(t.text_secondary))
                    .children(
                        issue
                            .priority
                            .as_deref()
                            .map(|priority| priority_mark(priority, cx)),
                    )
                    .child(div().flex_1())
                    .child(host.face(who, avatar, 20., cx)),
            )
            .on_click(listen::<Jira, _>(cx, move |jira, _, _, _, cx| {
                let open = (jira.open_key() != Some(key.as_str())).then(|| key.clone());
                jira.open(open);
                cx.notify();
            }))
            .context_menu(ui::menu(move |popup, _, _| {
                let mut popup = popup;
                for (to, (name, status_ids)) in targets.iter().enumerate() {
                    let (pane, key, site) = (pane.clone(), menu_key.clone(), site.clone());
                    let (column, status_ids) = (name.clone(), status_ids.clone());
                    popup = popup.item(
                        PopupMenuItem::new(format!("Move to {name}"))
                            .icon(Icon::Move)
                            .disabled(to == at)
                            .on_click(move |_, _, cx| {
                                let moved = Moved {
                                    key: &key,
                                    to,
                                    column: &column,
                                    status_ids: &status_ids,
                                    site: &site,
                                };
                                move_card(&pane, moved, cx)
                            }),
                    );
                }
                popup
            }))
            .into_any_element()
    }
}

/// What a board's columns and cards share: every column, the words a card is found by,
/// and the site.
struct Shape<'a> {
    columns: &'a [Column],
    wanted: &'a str,
    site: &'a str,
}

/// A card moved to another column.
struct Moved<'a> {
    key: &'a str,
    to: usize,
    column: &'a str,
    status_ids: &'a [String],
    site: &'a str,
}

/// How long is left in a sprint, in the words a standup uses.
fn sprint_left(end: Option<&str>) -> Option<String> {
    let end = chrono::DateTime::parse_from_rfc3339(end?).ok()?;
    let left = end.timestamp_millis() - chrono::Utc::now().timestamp_millis();
    if left < 0 {
        return Some("ended".into());
    }
    Some(match left / 86_400_000 {
        0 => "ends today".into(),
        1 => "1 day left".into(),
        days => format!("{days} days left"),
    })
}

/// Each column's cards after the moves still on their way to Jira.
fn with_moves<'a>(
    columns: &'a [Column],
    moves: &HashMap<String, usize>,
) -> Vec<Vec<&'a IssueSummary>> {
    columns
        .iter()
        .enumerate()
        .map(|(index, column)| {
            let staying = column
                .issues
                .iter()
                .filter(|issue| !moves.contains_key(&issue.key));
            let arriving = columns
                .iter()
                .flat_map(|other| other.issues.iter())
                .filter(|issue| moves.get(&issue.key) == Some(&index));
            staying.chain(arriving).collect()
        })
        .collect()
}

/// Moves a card to another column through whichever transition leads to one of its
/// statuses, showing it there until the board comes again.
fn move_card(pane: &WeakEntity<PluginPane>, moved: Moved, cx: &mut App) {
    let key = moved.key.to_owned();
    let params = json!({
        "key": moved.key,
        "statusIds": moved.status_ids,
        "column": moved.column,
        "site": site_of(moved.site),
    });
    let to = moved.to;
    let _ = pane.update(cx, |pane, cx| {
        let Some((jira, host)) = pane.parts::<Jira>() else {
            return;
        };
        jira.listing.moves.insert(key.clone(), to);
        let call = host.call("moveIssue", params);
        after::<Jira, _>(
            call,
            move |jira, host, done, cx| {
                if let Err(failure) = done {
                    jira.listing.moves.remove(&key);
                    host.said(&failure.titled(&format!("Move {key}")), cx);
                }
            },
            cx,
        );
        cx.notify();
    });
}
