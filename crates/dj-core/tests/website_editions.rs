//! The website sells what `dj_core::edition` says an edition is.
//!
//! The shop (`website/`) cannot read Rust, so it reads
//! `website/shared/editions.json`, and this test holds that file to the parts
//! list: a module added, renamed or moved between editions here fails until
//! the file is written again, so the site cannot go on selling last week's
//! editions. To write it:
//!
//! ```text
//! DJMANZO_WRITE_EDITIONS=1 cargo test -p dj-core --test website_editions
//! ```

use dj_core::edition::{Edition, Module};
use std::fmt::Write as _;
use std::path::PathBuf;

fn quoted(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
}

/// The parts list as the website reads it.
fn exported() -> String {
    let mut out = String::from("{\n");
    out.push_str(
        "  \"_\": \"Written from dj_core::edition by crates/dj-core/tests/website_editions.rs; do not edit by hand.\",\n",
    );
    out.push_str("  \"editions\": [\n");
    for (i, e) in Edition::ALL.iter().enumerate() {
        let modules: Vec<String> = e.modules().iter().map(|m| quoted(m.slug())).collect();
        let _ = writeln!(
            out,
            "    {{ \"slug\": {}, \"product\": {}, \"modules\": [{}] }}{}",
            quoted(e.slug()),
            quoted(e.product_name()),
            modules.join(", "),
            if i + 1 < Edition::ALL.len() { "," } else { "" }
        );
    }
    out.push_str("  ],\n  \"modules\": [\n");
    for (i, m) in Module::ALL.iter().enumerate() {
        let _ = writeln!(
            out,
            "    {{ \"slug\": {}, \"title\": {}, \"cheapest\": {} }}{}",
            quoted(m.slug()),
            quoted(m.title()),
            quoted(Edition::cheapest_with(*m).slug()),
            if i + 1 < Module::ALL.len() { "," } else { "" }
        );
    }
    out.push_str("  ]\n}\n");
    out
}

#[test]
fn the_website_sells_the_editions_this_code_defines() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../website/shared/editions.json");
    let want = exported();
    if std::env::var_os("DJMANZO_WRITE_EDITIONS").is_some() {
        std::fs::write(&path, &want).expect("website/shared/editions.json is writable");
    }
    let have = std::fs::read_to_string(&path).unwrap_or_default();
    assert!(
        have == want,
        "website/shared/editions.json is not dj_core::edition's parts list; write it again with \
         DJMANZO_WRITE_EDITIONS=1 cargo test -p dj-core --test website_editions"
    );
}
