//! Which releases updates come from, and whether anonymous usage is shared.

use gpui_kit::{div, AnyElement, Context, Entity, ParentElement, SharedString};
use sikemux_wire::protocol::UpdateChannel;

use crate::icons::Icon;
use crate::settings::rows;
use crate::ui;
use crate::workbench::Workbench;

pub const SECTIONS: &[&str] = &["Updates", "Privacy"];

pub fn render(
    bench: &Workbench,
    lit: &dyn Fn(&str) -> bool,
    this: &Entity<Workbench>,
    cx: &mut Context<Workbench>,
) -> Vec<AnyElement> {
    let preferences = bench.preferences();
    let channel = {
        let this = this.clone();
        let chosen = match preferences.update_channel {
            UpdateChannel::Stable => 0,
            _ => 1,
        };
        ui::Segmented::new(
            "update-channel",
            vec![
                (Icon::Shield, SharedString::from("Stable")),
                (Icon::Flask, SharedString::from("Nightly")),
            ],
            chosen,
        )
        .on_select(move |chosen, _, cx| {
            let channel = if *chosen == 0 { "stable" } else { "nightly" };
            this.update(cx, |bench, _| {
                crate::settings::set(bench, "updateChannel", channel)
            })
        })
    };
    let updates = div()
        .child(rows::row(
            "Channel",
            Some("Stable follows the latest signed release; nightly the newest build, prerelease or stable."),
            channel,
            cx,
        ))
        .child(rows::note("Checking for updates arrives with the updater.", cx));
    let privacy = rows::row(
        "Share anonymous usage",
        Some("Once a day, sends a random install ID and a one-way hash of your account with the app version, update channel and system version. Never your files, commands or projects."),
        rows::switch("share-usage", "shareUsageData", preferences.share_usage_data, this),
        cx,
    );
    vec![
        rows::section("Updates", None, None, lit("Updates"), updates, cx),
        rows::section("Privacy", None, None, lit("Privacy"), privacy, cx),
    ]
}
