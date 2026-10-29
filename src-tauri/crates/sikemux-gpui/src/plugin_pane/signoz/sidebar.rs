//! The sections and pinned services down the side, with the environment picked.

use super::*;

impl Signoz {
    pub(super) fn sidebar(
        &mut self,
        host: &mut Host,
        status: &Value,
        settings: &Settings,
        services: Option<Rc<Services>>,
        cx: &mut Context<PluginPane>,
    ) -> AnyElement {
        let t = Tokens::get(cx).clone();
        let environments = services
            .as_ref()
            .map(|services| services.environments.clone())
            .unwrap_or_default();
        let mut picks = vec![ui::Pick::new("", "All environments")];
        picks.extend(
            environments
                .iter()
                .map(|name| ui::Pick::new(name.clone(), name.clone())),
        );
        let shown = settings
            .environment
            .clone()
            .unwrap_or_else(|| "All environments".into());
        let environment = ui::select(
            "signoz-environment",
            shown,
            picks,
            Some(settings.environment.clone().unwrap_or_default().into()),
            false,
            listen::<Signoz, SharedString>(cx, |_, host, picked, _, cx| {
                let picked = picked.to_string();
                host.update_settings(
                    |settings: &mut Settings| {
                        settings.environment = (!picked.is_empty()).then_some(picked);
                    },
                    cx,
                );
            }),
            cx,
        );
        let mut sections = div().flex().flex_col().gap(px(1.));
        for (section, label, icon) in SECTIONS {
            sections = sections.child(
                ui::Row::new(SharedString::from(format!("signoz-section-{label}")), label)
                    .chosen(
                        self.section == section
                            && self.service.is_none()
                            && self.dashboard.is_none(),
                    )
                    .mark(22., ui::icon(icon, 14.).text_color(t.text_tertiary))
                    .on_click(listen::<Signoz, _>(cx, move |signoz, _, _, _, cx| {
                        signoz.show_section(section);
                        cx.notify();
                    })),
            );
        }
        let failing: HashSet<String> = services
            .as_ref()
            .map(|services| {
                services
                    .rows
                    .iter()
                    .filter(|row| row.error_rate >= FAILING_RATE)
                    .map(|row| row.service.clone())
                    .collect()
            })
            .unwrap_or_default();
        let pins = (!settings.pins.is_empty()).then(|| {
            let mut pinned = div().flex().flex_col().gap(px(1.)).child(
                div()
                    .px(px(8.))
                    .pt(px(4.))
                    .pb(px(2.))
                    .child(ui::caps("Pinned", cx)),
            );
            for pin in &settings.pins {
                let Some((kind, id)) = pin.split_once(':') else {
                    continue;
                };
                let id = id.to_owned();
                let row = match kind {
                    "service" => {
                        let ink = if failing.contains(&id) {
                            t.danger
                        } else {
                            t.live
                        };
                        let chosen = self.section == Section::Services
                            && self.service.as_deref() == Some(id.as_str());
                        let service = id.clone();
                        ui::Row::new(SharedString::from(format!("signoz-pin-{pin}")), id.clone())
                            .chosen(chosen)
                            .mark(22., div().size(px(6.)).rounded_full().bg(ink))
                            .on_click(listen::<Signoz, _>(cx, move |signoz, _, _, _, cx| {
                                signoz.show_service(service.clone(), Tab::Overview);
                                cx.notify();
                            }))
                    }
                    _ => {
                        let chosen = self.section == Section::Dashboards
                            && self.dashboard.as_deref() == Some(id.as_str());
                        let title = self.dashboard_title(host, &id, cx);
                        ui::Row::new(SharedString::from(format!("signoz-pin-{pin}")), title)
                            .chosen(chosen)
                            .mark(
                                22.,
                                ui::icon(Icon::Dashboard, 14.).text_color(t.text_tertiary),
                            )
                            .on_click(listen::<Signoz, _>(cx, move |signoz, _, _, _, cx| {
                                signoz.section = Section::Dashboards;
                                signoz.dashboard = Some(id.clone());
                                signoz.trace = None;
                                cx.notify();
                            }))
                    }
                };
                pinned = pinned.child(row);
            }
            pinned
        });
        let url = status["url"].as_str().unwrap_or_default().to_owned();
        let host_name = url
            .trim_start_matches("https://")
            .trim_start_matches("http://")
            .to_owned();
        let account = match status["email"].as_str() {
            Some(email) if !email.is_empty() => email.to_owned(),
            _ => "API key".into(),
        };
        div()
            .size_full()
            .flex()
            .flex_col()
            .child(
                div()
                    .px(px(10.))
                    .pt(px(10.))
                    .pb(px(8.))
                    .flex()
                    .flex_col()
                    .gap(px(8.))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(8.))
                            .child(ui::picture(Icon::Signoz, 16., signoz_mark()))
                            .child(
                                ui::text("SigNoz", 13., FontWeight::SEMIBOLD)
                                    .text_color(t.text_primary),
                            ),
                    )
                    .child(environment),
            )
            .child(
                div()
                    .id("signoz-side")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .px(px(6.))
                    .flex()
                    .flex_col()
                    .gap(px(10.))
                    .child(sections)
                    .children(pins),
            )
            .child(
                div()
                    .px(px(10.))
                    .py(px(8.))
                    .flex()
                    .flex_col()
                    .gap(px(6.))
                    .border_t_1()
                    .border_color(t.border)
                    .child(
                        ui::text(host_name, 12., FontWeight::MEDIUM)
                            .text_color(t.text_primary)
                            .truncate(),
                    )
                    .child(ui::meta(account, cx).truncate())
                    .child(
                        div()
                            .flex()
                            .flex_wrap()
                            .gap(px(6.))
                            .child(
                                Button::new("signoz-open", "Open", Variant::Outline)
                                    .icon(Icon::Globe)
                                    .on_click(move |_, _, cx| cx.open_url(&url)),
                            )
                            .child(
                                Button::new("signoz-sign-out", "Sign out", Variant::Outline)
                                    .icon(Icon::LogOut)
                                    .on_click(listen::<Signoz, _>(cx, |_, host, _, _, cx| {
                                        let call = host.call("signOut", Value::Null);
                                        after::<Signoz, _>(
                                            call,
                                            |_, host, done, cx| {
                                                if let Err(failure) = done {
                                                    host.said(
                                                        &failure.titled("Sign out of SigNoz"),
                                                        cx,
                                                    );
                                                }
                                            },
                                            cx,
                                        );
                                    })),
                            ),
                    ),
            )
            .into_any_element()
    }

    pub(super) fn dashboard_title(
        &self,
        host: &mut Host,
        id: &str,
        cx: &mut Context<PluginPane>,
    ) -> String {
        host.view("dashboardsView", Value::Null, cx)
            .read::<Vec<DashboardSummary>>()
            .ok()
            .flatten()
            .and_then(|all| {
                all.iter()
                    .find(|dashboard| dashboard.id == id)
                    .map(|dashboard| dashboard.title.clone())
            })
            .unwrap_or_else(|| "Dashboard".into())
    }
}
