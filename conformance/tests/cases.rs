//! Every case under cases/ holds, so the workspace's suite fails on a change
//! to the datalog crate that breaks one, without anybody writing the report.

use std::path::Path;

use datalog_conformance::{load, run_all};

#[test]
fn every_case_holds() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("cases");
    let features = load(&dir).unwrap_or_else(|e| panic!("{e}"));
    assert!(!features.is_empty(), "no cases under {}", dir.display());
    let outcomes = run_all(&features);
    let failed: Vec<String> = features
        .iter()
        .zip(&outcomes)
        .flat_map(|(f, os)| {
            f.cases
                .iter()
                .zip(os)
                .filter(|(_, o)| !o.passed)
                .map(move |(c, o)| format!("{}: {}: came back {:?}", f.file, c.name, o.got))
        })
        .collect();
    assert!(
        failed.is_empty(),
        "{} cases failed:\n{}",
        failed.len(),
        failed.join("\n")
    );
}
