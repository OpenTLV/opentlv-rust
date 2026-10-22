// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Marek Cingel

//! Selects and verifies the Core dependency before any Core build file runs.
//!
//! Shared by `build.rs` and `tests/core_dependency.rs`; uses only `std`.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

pub const REPOSITORY: &str = "OpenTLV/opentlv";

/// The approved Core dependency recorded in `core/core-artifact.json`.
#[derive(Clone, Debug, PartialEq)]
pub struct Lock {
    pub version: String,
    pub commit: String,
    pub release_tag: Option<String>,
    pub allow_verified_source: bool,
}

/// Identity of a Core Git checkout.
#[derive(Clone, Debug, PartialEq)]
pub struct Identity {
    pub commit: String,
    pub version: String,
    pub dirty: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Selection {
    /// `local-source`, `pinned-git`, `verified-source` or `verified-binary`.
    pub mode: String,
    /// True only for authenticated release artifacts.
    pub verified: bool,
    pub identity: Identity,
    /// Core source tree: built in source modes, headers and test data otherwise.
    pub source_dir: PathBuf,
    /// Directory containing the authenticated static library (`verified-binary`).
    pub lib_dir: Option<PathBuf>,
    /// Public headers of the authenticated library (`verified-binary`).
    pub include_dir: Option<PathBuf>,
}

#[derive(Clone, Debug, PartialEq)]
enum Value {
    Null,
    Bool(bool),
    Number(u64),
    Text(String),
}

/// Parses a flat JSON object of strings, integers, booleans and nulls.
fn parse_flat_json(text: &str) -> Result<BTreeMap<String, Value>, String> {
    let mut chars = text.chars().peekable();
    let mut fields = BTreeMap::new();
    let skip = |chars: &mut std::iter::Peekable<std::str::Chars>| {
        while chars.peek().is_some_and(|c| c.is_ascii_whitespace()) {
            chars.next();
        }
    };
    let string = |chars: &mut std::iter::Peekable<std::str::Chars>| {
        let mut value = String::new();
        loop {
            match chars.next() {
                Some('"') => return Ok(value),
                Some('\\') => match chars.next() {
                    Some(c @ ('"' | '\\' | '/')) => value.push(c),
                    _ => return Err("unsupported JSON escape".to_string()),
                },
                Some(c) if !c.is_control() => value.push(c),
                _ => return Err("unterminated JSON string".to_string()),
            }
        }
    };
    skip(&mut chars);
    if chars.next() != Some('{') {
        return Err("expected a JSON object".to_string());
    }
    skip(&mut chars);
    if chars.peek() == Some(&'}') {
        chars.next();
    } else {
        loop {
            skip(&mut chars);
            if chars.next() != Some('"') {
                return Err("expected a JSON key".to_string());
            }
            let key = string(&mut chars)?;
            skip(&mut chars);
            if chars.next() != Some(':') {
                return Err("expected ':'".to_string());
            }
            skip(&mut chars);
            let value = match chars.peek() {
                Some('"') => {
                    chars.next();
                    Value::Text(string(&mut chars)?)
                }
                Some(c) if c.is_ascii_alphanumeric() => {
                    let mut word = String::new();
                    while chars.peek().is_some_and(|c| c.is_ascii_alphanumeric()) {
                        word.push(chars.next().unwrap());
                    }
                    match word.as_str() {
                        "null" => Value::Null,
                        "true" => Value::Bool(true),
                        "false" => Value::Bool(false),
                        _ if word.bytes().all(|b| b.is_ascii_digit()) => {
                            Value::Number(word.parse().map_err(|_| "invalid JSON number")?)
                        }
                        _ => return Err(format!("unsupported JSON value `{word}`")),
                    }
                }
                _ => return Err("unsupported JSON value".to_string()),
            };
            if fields.insert(key.clone(), value).is_some() {
                return Err(format!("duplicate JSON key `{key}`"));
            }
            skip(&mut chars);
            match chars.next() {
                Some(',') => continue,
                Some('}') => break,
                _ => return Err("expected ',' or '}'".to_string()),
            }
        }
    }
    skip(&mut chars);
    if chars.next().is_some() {
        return Err("trailing data after JSON object".to_string());
    }
    Ok(fields)
}

fn is_commit(value: &str) -> bool {
    value.len() == 40
        && value
            .bytes()
            .all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

fn is_number(value: &str) -> bool {
    !value.is_empty() && value.bytes().all(|b| b.is_ascii_digit())
}

/// `MAJOR.MINOR.PATCH`, optionally followed by `-prerelease`.
fn is_version(value: &str, allow_prerelease: bool) -> bool {
    let (core, prerelease) = match value.split_once('-') {
        Some((core, prerelease)) => (core, Some(prerelease)),
        None => (value, None),
    };
    let parts: Vec<&str> = core.split('.').collect();
    parts.len() == 3
        && parts.iter().all(|part| is_number(part))
        && match prerelease {
            None => true,
            Some(text) => {
                allow_prerelease
                    && !text.is_empty()
                    && text
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'-')
            }
        }
}

pub fn parse_lock(text: &str) -> Result<Lock, String> {
    let fields = parse_flat_json(text).map_err(|e| format!("invalid Core artifact lock: {e}"))?;
    let expected = [
        "schema",
        "repository",
        "version",
        "commit",
        "release_tag",
        "allow_verified_source",
    ];
    if fields.len() != expected.len() || !expected.iter().all(|k| fields.contains_key(*k)) {
        return Err(format!(
            "Core artifact lock must contain exactly {expected:?}"
        ));
    }
    if fields["schema"] != Value::Number(1)
        || fields["repository"] != Value::Text(REPOSITORY.to_string())
    {
        return Err("unsupported Core artifact lock".to_string());
    }
    let commit = match &fields["commit"] {
        Value::Text(commit) if is_commit(commit) => commit.clone(),
        _ => return Err("expected an exact Core commit".to_string()),
    };
    let version = match &fields["version"] {
        Value::Text(version) if is_version(version, true) => version.clone(),
        _ => return Err("expected an exact Core version".to_string()),
    };
    let allow_verified_source = match fields["allow_verified_source"] {
        Value::Bool(value) => value,
        _ => return Err("explicit Verified Source fallback policy required".to_string()),
    };
    let release_tag = match &fields["release_tag"] {
        Value::Null => None,
        Value::Text(tag) if *tag == format!("v{version}") => Some(tag.clone()),
        _ => return Err("Core release tag must be null or `v<version>`".to_string()),
    };
    Ok(Lock {
        version,
        commit,
        release_tag,
        allow_verified_source,
    })
}

pub fn read_lock(path: &Path) -> Result<Lock, String> {
    let text =
        fs::read_to_string(path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    parse_lock(&text)
}

/// True for a set environment flag other than empty, `0` or `false`.
pub fn is_set(value: Option<String>) -> bool {
    value.is_some_and(|v| !matches!(v.to_ascii_lowercase().as_str(), "" | "0" | "false"))
}

/// True when a `CI` or `GITHUB_ACTIONS` value marks a CI build.
pub fn is_ci(lookup: impl Fn(&str) -> Option<String>) -> bool {
    ["CI", "GITHUB_ACTIONS"]
        .iter()
        .any(|key| is_set(lookup(key)))
}

fn git(dir: &Path, args: &[&str]) -> Result<String, String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .map_err(|e| format!("failed to run git (is Git installed?): {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "git {} failed in {}: {}",
            args.join(" "),
            dir.display(),
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn same_path(left: &Path, right: &Path) -> bool {
    match (left.canonicalize(), right.canonicalize()) {
        (Ok(left), Ok(right)) => left == right,
        _ => false,
    }
}

/// Reads the commit, version tag and dirty state of a Core Git checkout.
///
/// The version follows Core's own rule: the nearest version tag, preferring a
/// stable tag when several tags share its commit.
pub fn git_identity(source: &Path) -> Result<Identity, String> {
    if !source.join(".git").exists() {
        return Err(format!(
            "Core source {} must be an explicit Git checkout",
            source.display()
        ));
    }
    let top = git(source, &["rev-parse", "--show-toplevel"])?;
    if !same_path(Path::new(&top), source) {
        return Err(format!(
            "Core source {} must be the Git repository root",
            source.display()
        ));
    }
    let commit = git(source, &["rev-parse", "HEAD"])?;
    if !is_commit(&commit) {
        return Err("Core checkout has no exact commit".to_string());
    }
    let dirty = !git(source, &["status", "--porcelain", "--untracked-files=all"])?.is_empty();
    let tag = git(
        source,
        &[
            "describe",
            "--tags",
            "--abbrev=0",
            "--match",
            "v[0-9]*",
            "--match",
            "[0-9]*",
            "HEAD",
        ],
    )
    .map_err(|_| "Core checkout has no supported version tag".to_string())?;
    let tag_commit = git(source, &["rev-list", "-n", "1", &tag])?;
    let tags = git(
        source,
        &[
            "tag",
            "--points-at",
            &tag_commit,
            "--list",
            "v[0-9]*",
            "[0-9]*",
            "--sort=-version:refname",
        ],
    )?;
    let tags: Vec<&str> = tags
        .lines()
        .map(|t| t.strip_prefix('v').unwrap_or(t))
        .collect();
    let version = tags
        .iter()
        .find(|t| is_version(t, false))
        .or_else(|| tags.iter().find(|t| is_version(t, true)))
        .ok_or("Core checkout has no supported version tag")?
        .to_string();
    Ok(Identity {
        commit,
        version,
        dirty,
    })
}

/// Selects an explicit local Core checkout for development.
pub fn select_local(source: &Path, ci: bool, require_verified: bool) -> Result<Selection, String> {
    if ci || require_verified {
        return Err(
            "OPENTLV_CORE_SOURCE is development-only and forbidden in CI and verified builds"
                .to_string(),
        );
    }
    if !source.is_absolute() {
        return Err(format!(
            "OPENTLV_CORE_SOURCE must be an absolute path, got `{}`",
            source.display()
        ));
    }
    Ok(Selection {
        mode: "local-source".to_string(),
        verified: false,
        identity: git_identity(source)?,
        source_dir: source.to_path_buf(),
        lib_dir: None,
        include_dir: None,
    })
}

/// Clones the locked commit into `work/source` once and verifies it.
pub fn select_pinned(lock: &Lock, work: &Path, url: &str) -> Result<Selection, String> {
    let source = work.join("source");
    // Written only after a complete checkout, so an interrupted clone is
    // fetched again instead of being mistaken for the locked revision.
    let complete = work.join("source.complete");
    if !complete.exists() {
        let _ = fs::remove_dir_all(&source);
        fs::create_dir_all(work).map_err(|e| format!("cannot create {}: {e}", work.display()))?;
        let source_arg = source.to_str().ok_or("non-UTF-8 Core work path")?;
        git(
            work,
            &[
                "clone",
                "--quiet",
                "--no-checkout",
                "--filter=blob:none",
                url,
                source_arg,
            ],
        )?;
        git(
            &source,
            &[
                "-c",
                "advice.detachedHead=false",
                "checkout",
                "--quiet",
                "--detach",
                &lock.commit,
            ],
        )?;
        fs::write(&complete, &lock.commit)
            .map_err(|e| format!("cannot record the Core checkout: {e}"))?;
    }
    let identity = git_identity(&source)?;
    let stale = format!("; remove {} to fetch it again", source.display());
    if identity.commit != lock.commit {
        return Err(format!("Core checkout differs from locked commit{stale}"));
    }
    if identity.version != lock.version {
        return Err(format!(
            "Core checkout version {} differs from locked version {}{stale}",
            identity.version, lock.version
        ));
    }
    if identity.dirty {
        return Err(format!("pinned Core checkout is modified{stale}"));
    }
    Ok(Selection {
        mode: "pinned-git".to_string(),
        verified: false,
        identity,
        source_dir: source,
        lib_dir: None,
        include_dir: None,
    })
}

/// Reads the `key=value` result written by `core/verified_core.py`.
pub fn parse_verified(text: &str, lock: &Lock) -> Result<Selection, String> {
    let mut fields = BTreeMap::new();
    for line in text.lines() {
        let (key, value) = line
            .split_once('=')
            .ok_or("malformed verified Core result")?;
        if fields.insert(key, value).is_some() {
            return Err(format!("duplicate verified Core field `{key}`"));
        }
    }
    let field = |key: &str| {
        fields
            .get(key)
            .copied()
            .ok_or(format!("verified Core result lacks `{key}`"))
    };
    let mode = field("mode")?;
    let binary = match mode {
        "verified-binary" => true,
        "verified-source" => false,
        _ => return Err(format!("unexpected verified Core mode `{mode}`")),
    };
    if field("commit")? != lock.commit || field("version")? != lock.version {
        return Err("verified Core identity differs from the lock".to_string());
    }
    let path = |key: &str| field(key).map(PathBuf::from);
    Ok(Selection {
        mode: mode.to_string(),
        verified: true,
        identity: Identity {
            commit: lock.commit.clone(),
            version: lock.version.clone(),
            dirty: false,
        },
        source_dir: path("source_dir")?,
        lib_dir: if binary { Some(path("lib_dir")?) } else { None },
        include_dir: if binary {
            Some(path("include_dir")?)
        } else {
            None
        },
    })
}

fn json_string(value: &str) -> String {
    let mut out = String::from("\"");
    for c in value.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            c if c.is_control() => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// Machine-readable record of the selected Core.
pub fn evidence(selection: &Selection, lock: &Lock) -> String {
    let path = |p: &Option<PathBuf>| match p {
        Some(p) => json_string(&p.display().to_string()),
        None => "null".to_string(),
    };
    let tag = match &lock.release_tag {
        Some(tag) => json_string(tag),
        None => "null".to_string(),
    };
    format!(
        "{{\n  \"mode\": {},\n  \"verified\": {},\n  \"repository\": {},\n  \"version\": {},\n  \
         \"commit\": {},\n  \"dirty\": {},\n  \"locked_version\": {},\n  \"locked_commit\": {},\n  \
         \"release_tag\": {},\n  \"source_dir\": {},\n  \"lib_dir\": {},\n  \"include_dir\": {}\n}}\n",
        json_string(&selection.mode),
        selection.verified,
        json_string(REPOSITORY),
        json_string(&selection.identity.version),
        json_string(&selection.identity.commit),
        selection.identity.dirty,
        json_string(&lock.version),
        json_string(&lock.commit),
        tag,
        json_string(&selection.source_dir.display().to_string()),
        path(&selection.lib_dir),
        path(&selection.include_dir),
    )
}

fn header_lines(path: &Path) -> Result<Vec<String>, String> {
    Ok(fs::read_to_string(path)
        .map_err(|e| format!("cannot read {}: {e}", path.display()))?
        .lines()
        .map(|line| line.trim_end().to_string())
        .collect())
}

/// Checks that public `tlv/version.h` and `tlv/config.h` describe the
/// selected Core and the requested components.
pub fn validate_headers(
    version_h: &Path,
    config_h: &Path,
    identity: &Identity,
    components: &[(&str, bool)],
) -> Result<(), String> {
    let versions = header_lines(version_h)?;
    let expected = format!("#define OPENTLV_VERSION_STRING \"{}\"", identity.version);
    if !versions.contains(&expected) {
        return Err(format!(
            "Core public version differs from selected version {}",
            identity.version
        ));
    }
    let commits: Vec<&String> = versions
        .iter()
        .filter(|line| line.starts_with("#define OPENTLV_GIT_COMMIT_HASH "))
        .collect();
    let public = match commits.as_slice() {
        [line] => line
            .strip_prefix("#define OPENTLV_GIT_COMMIT_HASH \"")
            .and_then(|rest| rest.strip_suffix('"'))
            .unwrap_or_default(),
        _ => "",
    };
    if public.len() < 7
        || !public
            .bytes()
            .all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
        || !identity.commit.starts_with(public)
    {
        return Err(format!(
            "Core public commit `{public}` differs from selected commit {}",
            identity.commit
        ));
    }
    let config = header_lines(config_h)?;
    for (name, enabled) in components {
        let prefix = format!("#define {name} ");
        let matches: Vec<&String> = config.iter().filter(|l| l.starts_with(&prefix)).collect();
        let expected = format!("#define {name} {}", u8::from(*enabled));
        if matches.len() != 1 || *matches[0] != expected {
            return Err(format!("Core must define `{expected}` exactly once"));
        }
    }
    Ok(())
}
