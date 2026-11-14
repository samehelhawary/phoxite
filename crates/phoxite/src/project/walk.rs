//! Walking a project's files, skipping what `.gitignore` and the excluded
//! globs leave out.

use std::path::Path;
use std::sync::Arc;

use globset::{Glob, GlobSet, GlobSetBuilder};

/// Excluded-file globs, relative to a base folder. A pattern without `/`
/// matches at any depth (`node_modules`); a leading `/` anchors it at the base.
#[derive(Clone, Debug)]
pub struct ExcludeMatcher {
    set: GlobSet,
}

impl ExcludeMatcher {
    pub fn new(patterns: &[String]) -> Self {
        let mut builder = GlobSetBuilder::new();
        for raw in patterns {
            let pattern = raw.trim().trim_end_matches('/');
            if pattern.is_empty() {
                continue;
            }
            let anchored = pattern.starts_with('/');
            let pattern = pattern.trim_start_matches("./").trim_start_matches('/');
            if pattern.is_empty() {
                continue;
            }
            let mut variants = vec![pattern.to_owned()];
            if !anchored && !pattern.contains('/') {
                variants.push(format!("**/{pattern}"));
            }
            for v in variants {
                match Glob::new(&v) {
                    Ok(glob) => {
                        builder.add(glob);
                    }
                    Err(e) => tracing::warn!("invalid exclude pattern {raw:?}: {e}"),
                }
            }
        }
        let set = builder.build().unwrap_or_else(|e| {
            tracing::warn!("invalid exclude patterns: {e}");
            GlobSet::empty()
        });
        Self { set }
    }

    /// Whether `rel` itself matches (ancestors are not consulted).
    pub fn is_match(&self, rel: &Path) -> bool {
        !self.set.is_empty() && self.set.is_match(rel)
    }

    /// Whether `rel` (relative to the base) or one of its folders is excluded.
    pub fn is_excluded(&self, rel: &Path) -> bool {
        if self.set.is_empty() {
            return false;
        }
        rel.ancestors().filter(|p| !p.as_os_str().is_empty()).any(|p| self.set.is_match(p))
    }
}

fn inside_git_repo(dir: &Path) -> bool {
    dir.ancestors().any(|d| d.join(".git").exists() || d.join(".jj").exists())
}

/// Calls `visit` (from several threads) with every file below `root` that
/// neither `.gitignore` nor `excludes` leaves out. `.git` is skipped.
pub fn walk_files(root: &Path, excludes: &[String], visit: impl Fn(&Path) + Send + Sync) {
    let excludes = Arc::new(ExcludeMatcher::new(excludes));
    let base = root.to_path_buf();
    let mut builder = ignore::WalkBuilder::new(root);
    builder.hidden(false).require_git(inside_git_repo(root)).follow_links(false);
    builder.filter_entry(move |entry| {
        if entry.depth() > 0 && entry.file_name() == ".git" {
            return false;
        }
        match entry.path().strip_prefix(&base) {
            Ok(rel) if !rel.as_os_str().is_empty() => !excludes.is_match(rel),
            _ => true,
        }
    });
    let visit = &visit;
    builder.build_parallel().run(|| {
        Box::new(move |result| {
            match result {
                Ok(entry) => {
                    let is_file = match entry.file_type() {
                        Some(ft) if ft.is_file() => true,
                        Some(ft) if ft.is_symlink() => entry.path().is_file(),
                        _ => false,
                    };
                    if is_file {
                        visit(entry.path());
                    }
                }
                Err(e) => tracing::debug!("walk: {e}"),
            }
            ignore::WalkState::Continue
        })
    });
}
