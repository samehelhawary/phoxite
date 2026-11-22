//! `PhpUndefinedMagentoPluginMethod`: a method of a Magento plugin named
//! after a method its type doesn't have (`afterSaev()` on a class with
//! `save()`), so it never runs. See [`crate::frameworks::magento::plugins`].
//!
//! Only in a Magento project, for a class a `di.xml` registers as a plugin,
//! while the indexes are ready, and where every type the plugin is
//! registered on is a class the project has.

use super::{Inspection, InspectionContext, Problems};
use crate::base::Severity;
use crate::engine::EngineState;

/// `PhpUndefinedMagentoPluginMethod`.
pub struct UndefinedPluginMethod;

impl Inspection for UndefinedPluginMethod {
    fn id(&self) -> &'static str {
        "PhpUndefinedMagentoPluginMethod"
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn check(&self, cx: &InspectionContext<'_>, out: &mut Problems) {
        if cx.engine.status().state != EngineState::Ready || cx.engine.is_library(cx.path) {
            return;
        }
        for (start, end, message) in cx.engine.magento_plugin_problems(cx.summary()) {
            out.report(start, end, message);
        }
    }
}
