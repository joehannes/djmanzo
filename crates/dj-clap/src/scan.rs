//! Finding plugins on disk.
//!
//! CLAP says where plugins live, and unlike every format before it the answer
//! is the same on every machine. A host that scans those directories finds
//! everything a user has installed without being told anything.
//!
//! # Why the search is shallow-ish and why that matters
//!
//! Vendors nest: `~/.clap/u-he/Diva.clap` is normal, and so is a flat
//! `~/.clap/Something.clap`. But a plugin *bundle* on macOS is itself a
//! directory ending in `.clap` with a whole tree inside it, and descending into
//! one would find nothing and cost a great deal. So the walk stops the moment a
//! name ends in `.clap`, and otherwise goes a few levels down — enough for a
//! vendor folder, not enough to walk somebody's home directory.

use std::path::{Path, PathBuf};

/// How deep to look below a search root.
///
/// Two is `~/.clap/vendor/Thing.clap`. Three would find a vendor who nested by
/// product line as well; nobody does, and every extra level is a directory
/// tree somebody has to wait for.
const MAX_DEPTH: usize = 2;

/// A plugin bundle found on disk.
///
/// Only the path: what is *inside* it takes loading the library to find out,
/// and scanning must not load code. A directory named `Thing.clap` that is not
/// a plugin at all is a perfectly ordinary thing to find, and the honest
/// moment to discover it is when somebody asks to load it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Found {
    pub path: PathBuf,
    /// The file name without its extension, for a list before anything is
    /// loaded.
    pub name: String,
}

/// Where CLAP plugins live, per the specification.
///
/// System paths first, then the user's — but see [`scan`]: the *result* is
/// sorted by name, and a duplicate keeps the first found, so this order is what
/// decides which copy of a plugin installed twice gets used.
#[must_use]
pub fn search_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();

    #[cfg(target_os = "macos")]
    {
        paths.push(PathBuf::from("/Library/Audio/Plug-Ins/CLAP"));
        if let Some(home) = home() {
            paths.push(home.join("Library/Audio/Plug-Ins/CLAP"));
        }
    }

    #[cfg(target_os = "windows")]
    {
        if let Ok(common) = std::env::var("COMMONPROGRAMFILES") {
            paths.push(PathBuf::from(common).join("CLAP"));
        }
        if let Ok(local) = std::env::var("LOCALAPPDATA") {
            paths.push(PathBuf::from(local).join("Programs/Common/CLAP"));
        }
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        paths.push(PathBuf::from("/usr/lib/clap"));
        paths.push(PathBuf::from("/usr/local/lib/clap"));
        if let Some(home) = home() {
            paths.push(home.join(".clap"));
        }
    }

    // `CLAP_PATH` overrides nothing and adds everything: it is the escape
    // hatch for a plugin installed somewhere unusual, and it goes last so a
    // user's own entry wins a name collision with a system one.
    if let Some(extra) = std::env::var_os("CLAP_PATH") {
        paths.extend(std::env::split_paths(&extra));
    }
    paths
}

#[cfg(unix)]
fn home() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}

/// Every plugin bundle under the standard search paths.
///
/// Sorted by name, and a name found twice keeps the first — so a plugin
/// installed both system-wide and for one user resolves to one entry rather
/// than appearing twice in a list.
#[must_use]
pub fn scan() -> Vec<Found> {
    scan_paths(&search_paths())
}

/// [`scan`], against paths given rather than discovered. The testable half.
#[must_use]
pub fn scan_paths(roots: &[PathBuf]) -> Vec<Found> {
    let mut found = Vec::new();
    for root in roots {
        collect(root, 0, &mut found);
    }
    // By name, then by path. Sorting by name is what a list to pick from wants,
    // and it is also what makes the dedup below correct: `dedup_by` only
    // removes *adjacent* duplicates, so on a path-sorted list two copies of one
    // plugin in different directories would both survive.
    found.sort_by(|a, b| a.name.cmp(&b.name).then_with(|| a.path.cmp(&b.path)));
    found.dedup_by(|a, b| a.name == b.name);
    found
}

fn collect(dir: &Path, depth: usize, out: &mut Vec<Found>) {
    if depth > MAX_DEPTH {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        // A search path that does not exist is the normal case, not a fault:
        // most machines have no `/usr/lib/clap`.
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if is_bundle(&path) {
            if let Some(name) = plugin_name(&path) {
                out.push(Found { path, name });
            }
            // Never descend into a bundle. On macOS it is a directory tree with
            // nothing a scan wants in it.
            //
            // The `continue` is also load-bearing beyond style: `path` was
            // moved into the `Found` above, so removing it does not merely
            // change behaviour, it stops compiling. A guarantee the borrow
            // checker holds is worth more than one a test holds.
            continue;
        }
        if path.is_dir() {
            collect(&path, depth + 1, out);
        }
    }
}

fn is_bundle(path: &Path) -> bool {
    path.extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("clap"))
}

fn plugin_name(path: &Path) -> Option<String> {
    Some(path.file_stem()?.to_string_lossy().into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    /// A throwaway directory tree.
    struct Tree(PathBuf);

    impl Tree {
        fn new(name: &str) -> Tree {
            let path =
                std::env::temp_dir().join(format!("djmanzo-clap-{name}-{}", std::process::id()));
            let _ = fs::remove_dir_all(&path);
            fs::create_dir_all(&path).unwrap();
            Tree(path)
        }

        fn dir(&self, relative: &str) -> PathBuf {
            let path = self.0.join(relative);
            fs::create_dir_all(&path).unwrap();
            path
        }

        fn file(&self, relative: &str) {
            let path = self.0.join(relative);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).unwrap();
            }
            fs::write(path, b"not really a plugin").unwrap();
        }
    }

    impl Drop for Tree {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn names(found: &[Found]) -> Vec<&str> {
        found.iter().map(|f| f.name.as_str()).collect()
    }

    #[test]
    fn a_missing_search_path_is_not_a_fault() {
        let found = scan_paths(&[PathBuf::from("/no/such/place/at/all")]);
        assert!(found.is_empty());
    }

    #[test]
    fn it_finds_a_bundle_at_the_top_and_in_a_vendor_folder() {
        let tree = Tree::new("nested");
        tree.file("Flat.clap");
        tree.file("u-he/Diva.clap");
        let found = scan_paths(std::slice::from_ref(&tree.0));
        assert_eq!(names(&found), vec!["Diva", "Flat"]);
    }

    /// **A bundle is a leaf.** On macOS a `.clap` is a directory with a whole
    /// tree inside it, and descending would find nothing at real cost.
    #[test]
    fn it_does_not_descend_into_a_bundle() {
        let tree = Tree::new("bundle");
        tree.dir("Thing.clap/Contents/MacOS");
        tree.file("Thing.clap/Contents/MacOS/Thing");
        tree.file("Thing.clap/Contents/Inner.clap");
        let found = scan_paths(std::slice::from_ref(&tree.0));
        assert_eq!(names(&found), vec!["Thing"], "it walked into the bundle");
    }

    /// Somebody's home directory is not a plugin folder, and a scan that walked
    /// one would take minutes.
    #[test]
    fn it_stops_before_it_has_walked_the_whole_disk() {
        let tree = Tree::new("deep");
        tree.file("a/b/c/d/Buried.clap");
        assert!(scan_paths(std::slice::from_ref(&tree.0)).is_empty());
    }

    /// A plugin installed both system-wide and for one user is one plugin.
    #[test]
    fn a_plugin_found_twice_appears_once() {
        let tree = Tree::new("dup");
        tree.file("system/Thing.clap");
        tree.file("user/Thing.clap");
        let found = scan_paths(&[tree.0.join("system"), tree.0.join("user")]);
        assert_eq!(names(&found), vec!["Thing"]);
    }

    #[test]
    fn the_extension_is_matched_whatever_its_case() {
        let tree = Tree::new("case");
        tree.file("Shouty.CLAP");
        assert_eq!(
            names(&scan_paths(std::slice::from_ref(&tree.0))),
            vec!["Shouty"]
        );
    }

    #[test]
    fn things_that_are_not_plugins_are_left_alone() {
        let tree = Tree::new("other");
        tree.file("readme.txt");
        tree.file("libthing.so");
        assert!(scan_paths(std::slice::from_ref(&tree.0)).is_empty());
    }

    /// The search paths are where the specification says they are. Not a
    /// tautology: getting one wrong means a host that silently finds nothing
    /// on a machine full of plugins.
    #[test]
    fn the_standard_paths_are_searched() {
        let paths = search_paths();
        assert!(!paths.is_empty());
        #[cfg(all(unix, not(target_os = "macos")))]
        assert!(paths.iter().any(|p| p == Path::new("/usr/lib/clap")));
        #[cfg(target_os = "macos")]
        assert!(
            paths
                .iter()
                .any(|p| p == Path::new("/Library/Audio/Plug-Ins/CLAP"))
        );
    }
}
