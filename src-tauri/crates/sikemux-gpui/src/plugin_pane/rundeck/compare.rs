//! The grid that sets the branches of a folder's deploy jobs side by side.

use super::*;

/// The same jobs across the folders right below this one, side by side.
pub(super) fn compare_grid(
    sections: &[(&Section, Vec<&JobRow>)],
    loose: u32,
    cx: &mut Context<PluginPane>,
) -> AnyElement {
    let t = Tokens::get(cx).clone();
    let mut names: Vec<&str> = sections
        .iter()
        .flat_map(|(_, jobs)| jobs.iter().map(|job| job.name.as_str()))
        .collect();
    names.sort_unstable();
    names.dedup();
    let column = |section: &Section| {
        div().flex_1().min_w_0().px(px(8.)).child(
            ui::caps(section.segment.as_deref().unwrap_or_default(), cx)
                .when(section.prod, |caps| caps.text_color(t.warn)),
        )
    };
    let head = div()
        .flex()
        .px(px(12.))
        .py(px(8.))
        .child(div().w(px(160.)).flex_none().child(ui::caps("Job", cx)))
        .children(sections.iter().map(|(section, _)| column(section)));
    let rows = names.into_iter().map(|name| {
        div()
            .flex()
            .px(px(12.))
            .py(px(6.))
            .border_t_1()
            .border_color(t.line_soft)
            .child(
                ui::text(name.to_owned(), 12.5, FontWeight::MEDIUM)
                    .text_color(t.text_primary)
                    .w(px(160.))
                    .flex_none()
                    .truncate(),
            )
            .children(sections.iter().map(|(_, jobs)| {
                let cell = div()
                    .flex_1()
                    .min_w_0()
                    .px(px(8.))
                    .flex()
                    .flex_col()
                    .gap(px(2.));
                match jobs.iter().find(|job| job.name == name) {
                    None => cell,
                    Some(job) => cell
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(6.))
                                .child(status_dot(
                                    job.latest.as_ref().map(|run| run.status_kind),
                                    cx,
                                ))
                                .child(branch_chip(job.branch.as_deref(), cx)),
                        )
                        .children(job.latest.as_ref().map(|run| {
                            let when = run
                                .ended_at
                                .as_deref()
                                .or(run.started_at.as_deref())
                                .map(crate::ui::time::ago_written)
                                .unwrap_or_default();
                            ui::meta(
                                format!("{} · {when}", run.user.clone().unwrap_or_default()),
                                cx,
                            )
                        }))
                        .children(job.live_branch.as_ref().map(|live| {
                            ui::meta(format!("live is {live}"), cx).text_color(t.warn)
                        })),
                }
            }))
    });
    div()
        .id("rundeck-compare")
        .flex_1()
        .min_h_0()
        .overflow_y_scroll()
        .child(head)
        .children(rows)
        .when(loose > 0, |grid| {
            grid.child(div().px(px(12.)).py(px(10.)).child(ui::meta(
                format!(
                    "{loose} {} directly in this folder; switch to List to see {}.",
                    if loose == 1 { "job sits" } else { "jobs sit" },
                    if loose == 1 { "it" } else { "them" }
                ),
                cx,
            )))
        })
        .into_any_element()
}
