// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Marek Cingel

//! Shared failure coordinates, copied from the canonical C diagnostic.
use opentlv_sys as native;

/// Coordinate space; the producing API identifies the source object and origin.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LocationDomain {
    /// No coordinate space is known.
    #[default]
    Unknown,
    /// Input wire bytes.
    Input,
    /// Would-be output wire bytes.
    Output,
    /// Query expression text bytes.
    Expression,
    /// Definition text bytes.
    Definition,
    /// Value-local bytes with no known enclosing origin.
    Value,
}

/// Meaning of the coordinates, independently of the result code.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LocationKind {
    /// Coordinates are unavailable.
    #[default]
    Unknown,
    /// Known position, including byte zero or EOF.
    Point,
    /// Half-open evidence range.
    Span,
    /// End of the enclosing scope.
    ScopeEnd,
    /// Position where missing ordered content would be inserted.
    Insertion,
}

/// Owned primary evidence location. Numeric bounds are meaningful only for a known kind.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Location {
    /// Coordinate space.
    pub domain: LocationDomain,
    /// Anchor kind.
    pub kind: LocationKind,
    /// Inclusive start.
    pub begin: usize,
    /// Exclusive end, equal to begin for a point or boundary.
    pub end: usize,
}
impl Location {
    /// Known starting position; `Some(0)` differs from unknown.
    pub fn offset(self) -> Option<usize> {
        (self.kind != LocationKind::Unknown).then_some(self.begin)
    }
    pub(crate) fn from_raw(raw: native::tlv_location_t) -> Self {
        Self {
            domain: match raw.domain {
                1 => LocationDomain::Input,
                2 => LocationDomain::Output,
                3 => LocationDomain::Expression,
                4 => LocationDomain::Definition,
                5 => LocationDomain::Value,
                _ => LocationDomain::Unknown,
            },
            kind: match raw.kind {
                1 => LocationKind::Point,
                2 => LocationKind::Span,
                3 => LocationKind::ScopeEnd,
                4 => LocationKind::Insertion,
                _ => LocationKind::Unknown,
            },
            begin: raw.begin,
            end: raw.end,
        }
    }
}

impl std::fmt::Display for Location {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.kind == LocationKind::Unknown {
            return f.write_str("unknown location");
        }
        write!(
            f,
            "{} {} {}..{}",
            self.domain.name(),
            self.kind.name(),
            self.begin,
            self.end
        )
    }
}

impl LocationDomain {
    /// Canonical C coordinate-domain name.
    pub fn name(self) -> &'static str {
        // SAFETY: C returns static ASCII text.
        unsafe { std::ffi::CStr::from_ptr(native::tlv_location_domain_string(self as i32)) }
            .to_str()
            .expect("ASCII location label")
    }
}
impl LocationKind {
    /// Canonical C location-anchor name.
    pub fn name(self) -> &'static str {
        // SAFETY: C returns static ASCII text.
        unsafe { std::ffi::CStr::from_ptr(native::tlv_location_kind_string(self as i32)) }
            .to_str()
            .expect("ASCII location label")
    }
}
