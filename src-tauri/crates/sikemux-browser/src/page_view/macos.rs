use std::cell::Cell;

use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2::{define_class, msg_send, DefinedClass, MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{NSEvent, NSEventModifierFlags, NSEventPhase, NSView};
use objc2_foundation::{
    NSNumber, NSObject, NSObjectNSKeyValueCoding, NSObjectProtocol, NSPoint, NSRect, NSSize,
    NSString,
};
use objc2_web_kit::{
    WKNavigationAction, WKNavigationActionPolicy, WKNavigationDelegate, WKNavigationType,
    WKScriptMessage, WKScriptMessageHandler, WKUserContentController, WKUserScript,
    WKUserScriptInjectionTime, WKWebView, WKWebViewConfiguration, WKWebsiteDataStore,
};

use super::{PageEvent, BOOTSTRAP};

pub struct PageIvars {
    events: Box<dyn Fn(PageEvent)>,
}

define_class!(
    // SAFETY: a plain NSObject subclass that adds no dealloc and is only used on the
    // main thread.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[ivars = PageIvars]
    struct PageMessages;

    // SAFETY: every NSObject subclass already conforms to NSObjectProtocol.
    unsafe impl NSObjectProtocol for PageMessages {}

    // SAFETY: the method has the signature WebKit declares, and WebKit calls it on the
    // main thread with the page's message.
    unsafe impl WKScriptMessageHandler for PageMessages {
        #[unsafe(method(userContentController:didReceiveScriptMessage:))]
        fn received(&self, _controller: &WKUserContentController, message: &WKScriptMessage) {
            // SAFETY: main thread, and WebKit keeps `message` alive for the call.
            let body = unsafe { message.body() };
            let Some(text) = body.downcast_ref::<NSString>().map(|text| text.to_string()) else {
                return;
            };
            if let Some(event) = PageEvent::read(&text) {
                (self.ivars().events)(event);
            }
        }
    }

    // SAFETY: the method has the signature WebKit declares, and WebKit calls it on the
    // main thread with a live webview and action.
    unsafe impl WKNavigationDelegate for PageMessages {
        #[unsafe(method(webView:decidePolicyForNavigationAction:decisionHandler:))]
        unsafe fn decide(
            &self,
            _webview: &WKWebView,
            action: &WKNavigationAction,
            handler: &block2::DynBlock<dyn Fn(WKNavigationActionPolicy)>,
        ) {
            // SAFETY: main thread, and WebKit keeps `action` alive for the callback.
            let (kind, url) = unsafe {
                (
                    action.navigationType(),
                    action
                        .request()
                        .URL()
                        .and_then(|url| url.absoluteString())
                        .map(|url| url.to_string())
                        .unwrap_or_default(),
                )
            };
            let first_load = kind == WKNavigationType::Other && url == "about:blank";
            if kind == WKNavigationType::LinkActivated && url.starts_with("http") {
                (self.ivars().events)(PageEvent::Link(url));
            }
            handler.call((if first_load {
                WKNavigationActionPolicy::Allow
            } else {
                WKNavigationActionPolicy::Cancel
            },));
        }
    }
);

define_class!(
    /// A page drawn at its full height, so it never scrolls up or down itself: those
    /// scrolls go straight on to the app, and only sideways ones and pinches stay with WebKit.
    // SAFETY: WKWebView subclassed with one plain ivar and no dealloc, used on the main thread.
    #[unsafe(super(WKWebView))]
    #[thread_kind = MainThreadOnly]
    #[ivars = Cell<bool>]
    struct PageWebView;

    impl PageWebView {
        // SAFETY: matches NSResponder's `scrollWheel:`: an event in, nothing out.
        #[unsafe(method(scrollWheel:))]
        fn scroll_wheel(&self, event: &NSEvent) {
            let (phase, momentum) = (event.phase(), event.momentumPhase());
            if phase.contains(NSEventPhase::Began) || (phase.is_empty() && momentum.is_empty()) {
                self.ivars().set(is_vertical(event));
            }
            // SAFETY: main thread, where AppKit delivers scrolls; the next responder holds the page.
            let next = (self.ivars().get())
                .then(|| unsafe { self.nextResponder() })
                .flatten();
            match next {
                Some(next) => next.scrollWheel(event),
                // SAFETY: hands the event on to WKWebView's own handler, on the main thread.
                None => unsafe { msg_send![super(self), scrollWheel: event] },
            }
        }
    }
);

/// Whether a scroll that starts with `event` goes up or down rather than sideways or to zoom.
fn is_vertical(event: &NSEvent) -> bool {
    !event
        .modifierFlags()
        .contains(NSEventModifierFlags::Control)
        && event.scrollingDeltaY().abs() >= event.scrollingDeltaX().abs()
}

/// The page's view, which the app places, and the handler its events go through.
pub struct PageView {
    view: Retained<PageWebView>,
    _messages: Retained<PageMessages>,
}

impl PageView {
    /// Shows `html`, which the app has already given its theme, and tells `events` what
    /// the page's script reports.
    pub fn new(mtm: MainThreadMarker, html: &str, events: impl Fn(PageEvent) + 'static) -> Self {
        let messages = mtm.alloc::<PageMessages>().set_ivars(PageIvars {
            events: Box::new(events),
        });
        // SAFETY: NSObject's `init` takes nothing and returns the object it was sent to.
        let messages: Retained<PageMessages> = unsafe { msg_send![super(messages), init] };
        // SAFETY: main thread; the configuration and its parts are fresh and only ours,
        // and every value set is one WebKit reads with that type.
        let view = unsafe {
            let configuration = WKWebViewConfiguration::new(mtm);
            configuration.setWebsiteDataStore(&WKWebsiteDataStore::nonPersistentDataStore(mtm));
            let controller = configuration.userContentController();
            let script = WKUserScript::initWithSource_injectionTime_forMainFrameOnly(
                WKUserScript::alloc(mtm),
                &NSString::from_str(BOOTSTRAP),
                WKUserScriptInjectionTime::AtDocumentStart,
                true,
            );
            controller.addUserScript(&script);
            controller.addScriptMessageHandler_name(
                ProtocolObject::from_ref(&*messages),
                &NSString::from_str("sikemux"),
            );
            let frame = NSRect::new(NSPoint::ZERO, NSSize::new(1.0, 1.0));
            let view: Retained<PageWebView> = msg_send![
                super(PageWebView::alloc(mtm).set_ivars(Cell::new(false))),
                initWithFrame: frame,
                configuration: &*configuration
            ];
            view.unregisterDraggedTypes();
            view.setNavigationDelegate(Some(ProtocolObject::from_ref(&*messages)));
            let no = NSNumber::numberWithBool(false);
            view.setValue_forKey(Some(&no), &NSString::from_str("drawsBackground"));
            view.loadHTMLString_baseURL(&NSString::from_str(html), None);
            view
        };
        Self {
            view,
            _messages: messages,
        }
    }

    pub fn view(&self) -> Retained<NSView> {
        Retained::into_super(Retained::into_super(self.view.clone()))
    }

    /// Runs `script` in the page, as when the window's theme changes under it.
    pub fn run_script(&self, script: &str) {
        // SAFETY: main thread, as the view is only made there, and no answer is wanted.
        unsafe {
            self.view
                .evaluateJavaScript_completionHandler(&NSString::from_str(script), None)
        };
    }
}

impl Drop for PageView {
    fn drop(&mut self) {
        // SAFETY: main thread; the controller holds its handler strongly, so it lets go here.
        unsafe {
            self.view
                .configuration()
                .userContentController()
                .removeScriptMessageHandlerForName(&NSString::from_str("sikemux"));
        }
    }
}
