// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Marek Cingel

//! Core selection must reject unpinned, modified or mismatched dependencies
//! before any Core build file runs. Exercises the module `build.rs` uses.

#[allow(dead_code)]
#[path = "../core/select.rs"]
mod select;

use select::{Identity, Lock};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

const LOCK: &str = include_str!("../core/core-artifact.json");

/// A fresh directory removed when dropped.
struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "opentlv-core-{name}-{}-{stamp}",
            std::process::id()
        ));
        fs::create_dir(&path).unwrap();
        Scratch(path)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn git(dir: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args([
            "-c",
            "user.name=Dependency Test",
            "-c",
            "user.email=test@example.invalid",
            "-c",
            "commit.gpgsign=false",
            "-c",
            "tag.gpgsign=false",
            "-c",
            "core.autocrlf=false",
        ])
        .args(args)
        .output()
        .expect("tests require Git");
    assert!(
        output.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

/// A Core-like repository with one commit and an annotated version tag.
fn repository(dir: &Path, tag: &str) -> String {
    git(dir, &["init", "--quiet"]);
    fs::write(dir.join("CMakeLists.txt"), "# fixture\n").unwrap();
    git(dir, &["add", "CMakeLists.txt"]);
    git(dir, &["commit", "--quiet", "-m", "fixture"]);
    git(dir, &["tag", "-a", tag, "-m", "release"]);
    git(dir, &["rev-parse", "HEAD"])
}

fn lock_text(field: &str, value: &str) -> String {
    let mut fields = vec![
        ("schema", "1".to_string()),
        ("repository", "\"OpenTLV/opentlv\"".to_string()),
        ("version", "\"0.7.1\"".to_string()),
        ("commit", format!("\"{}\"", "a".repeat(40))),
        ("release_tag", "null".to_string()),
        ("allow_verified_source", "true".to_string()),
    ];
    for entry in &mut fields {
        if entry.0 == field {
            entry.1 = value.to_string();
        }
    }
    let body: Vec<String> = fields
        .iter()
        .map(|(k, v)| format!("\"{k}\": {v}"))
        .collect();
    format!("{{{}}}", body.join(", "))
}

fn lock_for(commit: &str) -> Lock {
    Lock {
        version: "0.7.1".to_string(),
        commit: commit.to_string(),
        release_tag: None,
        allow_verified_source: true,
    }
}

#[test]
fn committed_lock_pins_an_exact_core_commit() {
    let lock = select::parse_lock(LOCK).unwrap();
    assert_eq!(lock.commit, "345c77acb29266036df5f27d04ce315bb61853cc");
    assert_eq!(lock.version, "0.8.0");
    assert_eq!(lock.release_tag.as_deref(), Some("v0.8.0"));
    assert!(lock.allow_verified_source);
}

#[test]
fn lock_accepts_matching_release_tag() {
    let lock = select::parse_lock(&lock_text("release_tag", "\"v0.7.1\"")).unwrap();
    assert_eq!(lock.release_tag.as_deref(), Some("v0.7.1"));
}

#[test]
fn lock_rejects_floating_or_malformed_values() {
    let short = format!("\"{}\"", "a".repeat(39));
    let upper = format!("\"{}\"", "A".repeat(40));
    for (field, value) in [
        ("commit", "\"main\""),
        ("commit", short.as_str()),
        ("commit", upper.as_str()),
        ("version", "null"),
        ("version", "\"0.7\""),
        ("version", "\"latest\""),
        ("release_tag", "\"v9.0.0\""),
        ("release_tag", "\"0.7.1\""),
        ("allow_verified_source", "\"yes\""),
        ("repository", "\"someone/opentlv\""),
        ("schema", "2"),
    ] {
        assert!(
            select::parse_lock(&lock_text(field, value)).is_err(),
            "{field}={value} was accepted"
        );
    }
}

#[test]
fn lock_rejects_malformed_json() {
    let valid = lock_text("", "");
    for text in [
        valid.replace(", \"allow_verified_source\": true", ""),
        valid.replace('}', ", \"extra\": 1}"),
        valid.replace('}', ", \"schema\": 1}"),
        format!("{valid} trailing"),
        valid.replace("\"schema\": 1", "\"schema\": {\"nested\": 1}"),
        String::new(),
    ] {
        assert!(select::parse_lock(&text).is_err(), "{text} was accepted");
    }
}

#[test]
fn ci_detection() {
    let env = |pairs: &'static [(&'static str, &'static str)]| {
        move |key: &str| {
            pairs
                .iter()
                .find(|(k, _)| *k == key)
                .map(|(_, v)| v.to_string())
        }
    };
    assert!(select::is_ci(env(&[("CI", "true")])));
    assert!(select::is_ci(env(&[("GITHUB_ACTIONS", "true")])));
    assert!(select::is_ci(env(&[("CI", "1")])));
    assert!(!select::is_ci(env(&[
        ("CI", "false"),
        ("GITHUB_ACTIONS", "0")
    ])));
    assert!(!select::is_ci(env(&[("CI", "")])));
    assert!(!select::is_ci(env(&[])));
}

#[test]
fn local_override_is_development_only() {
    let dir = Scratch::new("local-ci");
    let commit = repository(&dir.0, "v0.7.1");
    for (ci, verified) in [(true, false), (false, true)] {
        let error = select::select_local(&dir.0, ci, verified).unwrap_err();
        assert!(error.contains("development-only"), "{error}");
    }
    let selection = select::select_local(&dir.0, false, false).unwrap();
    assert_eq!(selection.mode, "local-source");
    assert!(!selection.verified);
    assert_eq!(selection.identity.commit, commit);
    assert_eq!(selection.source_dir, dir.0);
}

#[test]
fn local_override_requires_absolute_git_root() {
    let dir = Scratch::new("local-root");
    assert!(select::select_local(Path::new("../opentlv"), false, false)
        .unwrap_err()
        .contains("absolute"));
    assert!(select::git_identity(&dir.0)
        .unwrap_err()
        .contains("explicit Git checkout"));
    repository(&dir.0, "v0.7.1");
    let nested = dir.0.join("nested");
    fs::create_dir_all(nested.join(".git")).unwrap();
    assert!(select::git_identity(&nested)
        .unwrap_err()
        .contains("repository root"));
}

#[test]
fn git_identity_follows_core_version_rules() {
    let dir = Scratch::new("identity");
    let commit = repository(&dir.0, "v0.7.1");
    assert_eq!(
        select::git_identity(&dir.0).unwrap(),
        Identity {
            commit: commit.clone(),
            version: "0.7.1".to_string(),
            dirty: false
        }
    );
    // A stable tag wins over a pre-release tag on the same commit.
    git(&dir.0, &["tag", "-a", "v0.8.0-rc.1", "-m", "candidate"]);
    assert_eq!(select::git_identity(&dir.0).unwrap().version, "0.7.1");
    // Later commits keep the nearest tag; historical tags have no `v`.
    fs::write(dir.0.join("later.txt"), "later\n").unwrap();
    git(&dir.0, &["add", "later.txt"]);
    git(&dir.0, &["commit", "--quiet", "-m", "later"]);
    git(&dir.0, &["tag", "0.7.2"]);
    assert_eq!(select::git_identity(&dir.0).unwrap().version, "0.7.2");
    fs::write(dir.0.join("CMakeLists.txt"), "# changed\n").unwrap();
    assert!(select::git_identity(&dir.0).unwrap().dirty);
    fs::write(dir.0.join("CMakeLists.txt"), "# fixture\n").unwrap();
    fs::write(dir.0.join("untracked.txt"), "new\n").unwrap();
    assert!(select::git_identity(&dir.0).unwrap().dirty);
}

#[test]
fn git_identity_requires_version_tag() {
    let dir = Scratch::new("untagged");
    repository(&dir.0, "not-a-version");
    assert!(select::git_identity(&dir.0)
        .unwrap_err()
        .contains("version tag"));
}

#[test]
fn pinned_clone_is_verified_and_reused() {
    let origin = Scratch::new("origin");
    let commit = repository(&origin.0, "v0.7.1");
    let url = origin.0.to_str().unwrap();
    let work = Scratch::new("work");
    let lock = lock_for(&commit);

    let selection = select::select_pinned(&lock, &work.0, url).unwrap();
    assert_eq!(selection.mode, "pinned-git");
    assert!(!selection.verified);
    assert_eq!(selection.identity.commit, commit);
    assert!(work.0.join("source.complete").is_file());

    // Reused without fetching; a modified checkout is rejected, not repaired.
    fs::remove_dir_all(&origin.0).unwrap();
    assert!(select::select_pinned(&lock, &work.0, url).is_ok());
    fs::write(selection.source_dir.join("CMakeLists.txt"), "# changed\n").unwrap();
    assert!(select::select_pinned(&lock, &work.0, url)
        .unwrap_err()
        .contains("modified"));
}

#[test]
fn interrupted_clone_is_fetched_again() {
    let origin = Scratch::new("origin-retry");
    let commit = repository(&origin.0, "v0.7.1");
    let work = Scratch::new("work-retry");
    fs::create_dir_all(work.0.join("source/partial")).unwrap();
    let selection =
        select::select_pinned(&lock_for(&commit), &work.0, origin.0.to_str().unwrap()).unwrap();
    assert!(!selection.source_dir.join("partial").exists());
}

#[test]
fn pinned_clone_rejects_identity_mismatch() {
    let origin = Scratch::new("origin-mismatch");
    let commit = repository(&origin.0, "v0.7.1");
    let url = origin.0.to_str().unwrap();

    let work = Scratch::new("work-version");
    let mut lock = lock_for(&commit);
    lock.version = "0.7.0".to_string();
    assert!(select::select_pinned(&lock, &work.0, url)
        .unwrap_err()
        .contains("version"));

    let work = Scratch::new("work-commit");
    assert!(select::select_pinned(&lock_for(&"b".repeat(40)), &work.0, url).is_err());
    assert!(!work.0.join("source.complete").exists());
}

#[test]
fn verified_result_must_match_lock() {
    let commit = "c".repeat(40);
    let lock = lock_for(&commit);
    let source =
        format!("mode=verified-source\nversion=0.7.1\ncommit={commit}\nsource_dir=/core\n");
    let selection = select::parse_verified(&source, &lock).unwrap();
    assert!(selection.verified);
    assert_eq!(selection.lib_dir, None);

    let binary = format!(
        "mode=verified-binary\nversion=0.7.1\ncommit={commit}\nsource_dir=/core\n\
         lib_dir=/pkg/lib\ninclude_dir=/pkg/include\n"
    );
    let selection = select::parse_verified(&binary, &lock).unwrap();
    assert_eq!(selection.lib_dir, Some(PathBuf::from("/pkg/lib")));

    for text in [
        source.replace(&commit, &"d".repeat(40)),
        source.replace("0.7.1", "0.7.2"),
        source.replace("verified-source", "pinned-git"),
        binary.replace("lib_dir=/pkg/lib\n", ""),
        format!("{source}mode=verified-source\n"),
        format!("{source}garbage\n"),
    ] {
        assert!(select::parse_verified(&text, &lock).is_err(), "{text}");
    }
}

#[test]
fn public_headers_must_match_selection() {
    let dir = Scratch::new("headers");
    let identity = Identity {
        commit: "476f1fbecde6b056419e43c9a6cea4d1ca8df1cf".to_string(),
        version: "0.7.1".to_string(),
        dirty: false,
    };
    let version_h = dir.0.join("version.h");
    let config_h = dir.0.join("config.h");
    let components = [("OPENTLV_READER", true), ("OPENTLV_LLDP", false)];
    let check = |version: &str, config: &str| {
        fs::write(&version_h, version).unwrap();
        fs::write(&config_h, config).unwrap();
        select::validate_headers(&version_h, &config_h, &identity, &components)
    };
    let version = "#define OPENTLV_VERSION_STRING \"0.7.1\"\n\
                   #define OPENTLV_GIT_COMMIT_HASH \"476f1fbe\"\n";
    let config = "#define OPENTLV_READER 1\n#define OPENTLV_LLDP 0\n";
    check(version, config).unwrap();
    for (version_text, config_text) in [
        (version.replace("0.7.1", "0.7.2"), config.to_string()),
        (version.replace("476f1fbe", "476f1fbf"), config.to_string()),
        (version.replace("476f1fbe", "476f1f"), config.to_string()),
        (version.replace("476f1fbe", "unknown"), config.to_string()),
        (version.to_string(), config.replace("READER 1", "READER 0")),
        (version.to_string(), config.replace("LLDP 0", "LLDP 1")),
        (
            version.to_string(),
            format!("{config}#define OPENTLV_READER 1\n"),
        ),
        (
            version.to_string(),
            "#define OPENTLV_READER 1\n".to_string(),
        ),
    ] {
        assert!(
            check(&version_text, &config_text).is_err(),
            "{version_text}{config_text}"
        );
    }
}

#[test]
fn evidence_records_selection_and_lock() {
    let lock = select::parse_lock(LOCK).unwrap();
    let selection = select::Selection {
        mode: "local-source".to_string(),
        verified: false,
        identity: Identity {
            commit: "e".repeat(40),
            version: "0.8.0".to_string(),
            dirty: true,
        },
        source_dir: PathBuf::from("C:\\core \"dev\""),
        lib_dir: None,
        include_dir: None,
    };
    let text = select::evidence(&selection, &lock);
    for expected in [
        "\"mode\": \"local-source\"",
        "\"verified\": false",
        "\"dirty\": true",
        "\"version\": \"0.8.0\"",
        "\"locked_commit\": \"345c77acb29266036df5f27d04ce315bb61853cc\"",
        "\"release_tag\": \"v0.8.0\"",
        "\"source_dir\": \"C:\\\\core \\\"dev\\\"\"",
        "\"lib_dir\": null",
    ] {
        assert!(text.contains(expected), "{expected} missing from {text}");
    }
}
