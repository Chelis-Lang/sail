//! Read-time freshness of completed managed results, for daemon readers.
//!
//! Each check rereads exactly the files a Current run recorded (see
//! `read_check`); there is no retained projection, watcher or lease.

use super::InputSnapshot;
use crate::Result;
use std::collections::BTreeMap;
use std::path::Path;

/// Stateless read-time freshness checks. Kept as a type so the daemon's job
/// service owns one value, as before.
#[derive(Clone, Default)]
pub struct FreshnessIndex;

impl FreshnessIndex {
    /// Rechecks the latest Current record of `context` against the files now.
    /// `None` means there is no usable record, which readers report as Unknown.
    #[must_use]
    pub fn inspect(
        &self,
        cache: &Path,
        workspace: &Path,
        context: &str,
        environment: &BTreeMap<String, String>,
    ) -> Option<InputSnapshot> {
        self.establish(cache, workspace, context, environment).ok()
    }

    /// The same check as [`FreshnessIndex::inspect`], with its error.
    ///
    /// # Errors
    /// `SAIL3108` when the context has no readable record of a Current run.
    pub fn establish(
        &self,
        cache: &Path,
        workspace: &Path,
        context: &str,
        _environment: &BTreeMap<String, String>,
    ) -> Result<InputSnapshot> {
        // The environment is bound by the context identity itself.
        super::read_check::check(cache, workspace, context)
    }
}
