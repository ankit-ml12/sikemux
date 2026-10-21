//! Tells the person, while Sikemux is in the background, when an agent needs them or
//! finished: a macOS notification that brings the agent forward when clicked, and the
//! Dock icon's bounce and badge.

use gpui_kit::Context;
use sikemux_wire::protocol::{AgentGroup, DeviceView};
use tokio::sync::oneshot;

use crate::workbench::Workbench;

/// What the test row and the first launch post, to show what a notice looks like.
const TEST: (&str, &str) = (
    "Sikemux notifications are on",
    "This is how an agent will tell you it needs you or has finished.",
);

/// Posts a notification and, when it is clicked, brings Sikemux forward on the agent it is about.
pub fn post(title: String, body: String, agent: Option<String>, cx: &mut Context<Workbench>) {
    let (clicked, click) = oneshot::channel();
    std::thread::Builder::new()
        .name("notification".into())
        .spawn(move || {
            let _ = clicked.send(post_and_wait_for_click(&title, &body));
        })
        .ok();
    cx.spawn(async move |bench, cx| {
        if click.await != Ok(true) {
            return;
        }
        let _ = bench.update(cx, |bench, cx| {
            cx.activate(true);
            if let Some(agent) = agent {
                bench.open_agent(&agent);
            }
        });
    })
    .detach();
}

#[cfg(target_os = "macos")]
fn post_and_wait_for_click(title: &str, body: &str) -> bool {
    use mac_notification_sys::{Notification, NotificationResponse};
    // A build run from the terminal has no installed bundle for macOS to show the notification as.
    let bundle = match crate::core::IDENTIFIER.ends_with(".dev") {
        true => "com.apple.Terminal",
        false => crate::core::IDENTIFIER,
    };
    let _ = mac_notification_sys::set_application(bundle);
    matches!(
        Notification::new()
            .title(title)
            .message(body)
            .sound("Tink")
            .wait_for_click(true)
            .send(),
        Ok(NotificationResponse::Click)
    )
}

#[cfg(not(target_os = "macos"))]
fn post_and_wait_for_click(_: &str, _: &str) -> bool {
    false
}

/// Posts what the core said about an agent, while the person is away from Sikemux and
/// asked to hear.
pub fn notice(
    bench: &Workbench,
    title: String,
    body: String,
    agent: String,
    cx: &mut Context<Workbench>,
) {
    if !bench.preferences().agent_notifications || cx.active_window().is_some() {
        return;
    }
    if let Some(window) = cx.windows().first().copied() {
        let _ = window.update(cx, |_, window, _| window.request_attention());
    }
    post(title, body, Some(agent), cx);
}

pub fn test(cx: &mut Context<Workbench>) {
    post(TEST.0.into(), TEST.1.into(), None, cx);
}

/// macOS asks whether an app may notify only the first time it posts, so the first launch
/// with notifications on posts a greeting to put that question up front.
pub fn introduce(bench: &Workbench, cx: &mut Context<Workbench>) {
    let preferences = bench.preferences();
    if !preferences.agent_notifications || preferences.notifications_introduced {
        return;
    }
    crate::settings::set(bench, "notificationsIntroduced", true);
    test(cx);
}

/// How many agents wait on the person: those asking for something, and those that finished
/// a turn the person has not seen.
fn waiting(view: &DeviceView) -> usize {
    let waits =
        |group: Option<AgentGroup>| matches!(group, Some(AgentGroup::Blocked | AgentGroup::Done));
    view.workspace
        .agents
        .iter()
        .filter(|agent| {
            let chat = view
                .chats
                .iter()
                .find(|chat| chat.agent_id == agent.id)
                .map(|chat| chat.group);
            let terminal = view
                .sessions
                .iter()
                .find(|session| session.agent_id.as_deref() == Some(agent.id.as_str()))
                .and_then(|session| session.group);
            waits(chat.or(terminal))
        })
        .count()
}

/// Keeps the Dock icon's badge at the number of agents waiting on the person.
pub fn badge(bench: &mut Workbench) {
    let count = bench.view.as_ref().map_or(0, waiting);
    if count != std::mem::replace(&mut bench.badge, count) {
        show_badge(count);
    }
}

fn show_badge(count: usize) {
    #[cfg(target_os = "macos")]
    {
        use objc2::MainThreadMarker;
        use objc2_app_kit::NSApplication;
        use objc2_foundation::NSString;
        let Some(main) = MainThreadMarker::new() else {
            return;
        };
        let label = (count > 0).then(|| NSString::from_str(&count.to_string()));
        NSApplication::sharedApplication(main)
            .dockTile()
            .setBadgeLabel(label.as_deref());
    }
}
