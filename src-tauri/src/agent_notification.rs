//! The app's notifications. Clicking one on macOS brings the app forward, and
//! opens the agent it is about.

use std::sync::Mutex;

use sikemux_process::sync::Locked;
use tauri::{AppHandle, Emitter, Manager, State};

/// The agent someone asked to see, kept until the page takes it, since a page
/// still loading misses the event that says one is waiting.
#[derive(Default)]
pub struct WaitingFocus(Mutex<Option<String>>);

/// Brings the window forward and has the page show `agent_id`.
pub(crate) fn focus_agent(app: &AppHandle, agent_id: String) {
    crate::harness::bring_to_front(app);
    *app.state::<WaitingFocus>().0.locked() = Some(agent_id);
    let _ = app.emit_to("main", "focus_agent", ());
}

/// The agent waiting to be shown, once.
#[tauri::command]
pub fn focus_claim(waiting: State<'_, WaitingFocus>) -> Option<String> {
    waiting.0.locked().take()
}

#[tauri::command]
pub fn post_notification(app: AppHandle, title: String, body: String, agent_id: Option<String>) {
    post(app, title, body, agent_id);
}

#[cfg(target_os = "macos")]
fn post(app: AppHandle, title: String, body: String, agent_id: Option<String>) {
    let shown = app.clone();
    let _ = app.run_on_main_thread(move || macos::post(&shown, &title, &body, agent_id.as_deref()));
}

#[cfg(not(target_os = "macos"))]
fn post(_app: AppHandle, title: String, body: String, _agent_id: Option<String>) {
    std::thread::spawn(move || {
        let _ = notify_rust::Notification::new()
            .summary(&title)
            .body(&body)
            .show();
    });
}

#[cfg(target_os = "macos")]
#[allow(deprecated)]
mod macos {
    use std::sync::{Once, OnceLock};

    use objc2::rc::Retained;
    use objc2::runtime::{AnyObject, Imp, NSObjectProtocol, ProtocolObject, Sel};
    use objc2::{
        define_class, msg_send, sel, ClassType, DefinedClass, MainThreadMarker, MainThreadOnly,
    };
    use objc2_foundation::{
        ns_string, NSBundle, NSDictionary, NSObject, NSString, NSUserNotification,
        NSUserNotificationCenter, NSUserNotificationCenterDelegate,
    };
    use tauri::AppHandle;

    define_class!(
        /// Hears every click on the app's notifications.
        // SAFETY: a plain NSObject subclass that adds no dealloc and is only used on the
        // main thread.
        #[unsafe(super(NSObject))]
        #[thread_kind = MainThreadOnly]
        #[ivars = AppHandle]
        struct ClickHandler;

        // SAFETY: every NSObject subclass already conforms to NSObjectProtocol.
        unsafe impl NSObjectProtocol for ClickHandler {}

        // SAFETY: both methods have the delegate protocol's exact signatures.
        unsafe impl NSUserNotificationCenterDelegate for ClickHandler {
            #[unsafe(method(userNotificationCenter:didActivateNotification:))]
            fn clicked(
                &self,
                center: &NSUserNotificationCenter,
                notification: &NSUserNotification,
            ) {
                center.removeDeliveredNotification(notification);
                let agent_id = notification
                    .userInfo()
                    .and_then(|info| info.objectForKey(agent_key()))
                    .and_then(|agent_id| agent_id.downcast::<NSString>().ok());
                match agent_id {
                    Some(agent_id) => super::focus_agent(self.ivars(), agent_id.to_string()),
                    None => crate::harness::bring_to_front(self.ivars()),
                }
            }

            #[unsafe(method(userNotificationCenter:shouldPresentNotification:))]
            fn shown_in_front(
                &self,
                _center: &NSUserNotificationCenter,
                _notification: &NSUserNotification,
            ) -> bool {
                true
            }
        }
    );

    fn agent_key() -> &'static NSString {
        ns_string!("agent")
    }

    pub(super) fn post(app: &AppHandle, title: &str, body: &str, agent_id: Option<&str>) {
        let Some(mtm) = MainThreadMarker::new() else {
            return;
        };
        static LISTENING: Once = Once::new();
        LISTENING.call_once(|| {
            if tauri::is_dev() {
                show_as_terminal();
            }
            let handler = mtm.alloc::<ClickHandler>().set_ivars(app.clone());
            // SAFETY: `handler` is freshly allocated with its ivars set, and NSObject's `init`
            // takes nothing and returns that same object.
            let handler: Retained<ClickHandler> = unsafe { msg_send![super(handler), init] };
            // SAFETY: the handler is never released, so it outlives the center's unretained
            // reference to it.
            unsafe {
                NSUserNotificationCenter::defaultUserNotificationCenter().setDelegate(Some(
                    ProtocolObject::from_ref(&*Retained::into_raw(handler)),
                ));
            }
        });

        let notification = NSUserNotification::new();
        notification.setTitle(Some(&NSString::from_str(title)));
        notification.setInformativeText(Some(&NSString::from_str(body)));
        if let Some(agent_id) = agent_id {
            let agent_id = NSString::from_str(agent_id);
            let info =
                NSDictionary::<NSString, AnyObject>::from_slices(&[agent_key()], &[&agent_id]);
            // SAFETY: the dictionary holds only strings, which the notification can store.
            unsafe { notification.setUserInfo(Some(&info)) };
        }
        NSUserNotificationCenter::defaultUserNotificationCenter()
            .deliverNotification(&notification);
    }

    type BundleIdentifier = unsafe extern "C-unwind" fn(&NSBundle, Sel) -> *const NSString;

    static BUNDLE_IDENTIFIER: OnceLock<Imp> = OnceLock::new();

    /// A dev build has no installed bundle for macOS to show its notifications as,
    /// so it shows them as Terminal's.
    fn show_as_terminal() {
        let Some(method) = NSBundle::class().instance_method(sel!(bundleIdentifier)) else {
            return;
        };
        BUNDLE_IDENTIFIER.get_or_init(|| method.implementation());
        let replacement: BundleIdentifier = terminal_for_main_bundle;
        // SAFETY: `terminal_for_main_bundle` has `bundleIdentifier`'s exact signature.
        unsafe {
            method.set_implementation(std::mem::transmute::<BundleIdentifier, Imp>(replacement))
        };
    }

    // SAFETY: only the Objective-C runtime calls this, as `bundleIdentifier` on a live bundle.
    unsafe extern "C-unwind" fn terminal_for_main_bundle(
        bundle: &NSBundle,
        selector: Sel,
    ) -> *const NSString {
        if std::ptr::eq(bundle, &*NSBundle::mainBundle()) {
            return ns_string!("com.apple.Terminal");
        }
        let Some(&inherited) = BUNDLE_IDENTIFIER.get() else {
            return std::ptr::null();
        };
        // SAFETY: `inherited` is the implementation this function stands in front of.
        let inherited = unsafe { std::mem::transmute::<Imp, BundleIdentifier>(inherited) };
        // SAFETY: passes the runtime's own arguments straight through.
        unsafe { inherited(bundle, selector) }
    }
}
