use std::sync::Arc;

use webkit2gtk::WebView;

use crate::TabEvents;

pub(crate) fn attach(_: &WebView, _: &str, _: &str, _: Arc<dyn TabEvents>) {}
