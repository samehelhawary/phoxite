//! Per-inspection settings of an inspection profile.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Changes to one inspection; absent fields keep the inspection's defaults.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct InspectionOverride {
    pub enabled: Option<bool>,
    pub severity: Option<InspectionLevel>,
    /// The inspection's own options; absent ones keep their defaults.
    pub options: Option<BTreeMap<String, bool>>,
}

/// Highlighting levels.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum InspectionLevel {
    Error,
    Warning,
    WeakWarning,
    ServerProblem,
    Typo,
    /// Reported but not highlighted in the editor.
    None,
}
