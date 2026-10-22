// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Marek Cingel

//! Wire formats shared by the reader and the writer.

use std::fmt;
use std::ptr;
use std::str::FromStr;

use opentlv_sys as native;

use crate::error::Error;

/// The wire format a [`Reader`](crate::Reader), [`Writer`](crate::Writer) or
/// [`StructureSchema`](crate::StructureSchema) uses.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Format {
    /// BER-TLV.
    #[default]
    Ber,
    /// Definite EMV Contact Book 3 BER-TLV element framing.
    Emv,
    /// Canonical Encoding Rules.
    Cer,
    /// Distinguished Encoding Rules.
    Der,
    /// LLDP packed Type/Length framing; no LLDPDU semantic validation.
    #[cfg(feature = "lldp")]
    Lldp,
    /// Contiguous NFC Type 2 TLV stream; termination is controlled by the caller.
    #[cfg(feature = "nfc")]
    NfcType2,
}

impl Format {
    /// Every supported format.
    pub const ALL: [Format; 4 + cfg!(feature = "lldp") as usize + cfg!(feature = "nfc") as usize] = [
        Format::Ber,
        Format::Emv,
        Format::Cer,
        Format::Der,
        #[cfg(feature = "lldp")]
        Format::Lldp,
        #[cfg(feature = "nfc")]
        Format::NfcType2,
    ];

    /// Returns the lowercase name of the format, accepted by [`FromStr`].
    pub fn name(self) -> &'static str {
        match self {
            Format::Ber => "ber",
            Format::Emv => "emv",
            Format::Cer => "cer",
            Format::Der => "der",
            #[cfg(feature = "lldp")]
            Format::Lldp => "lldp",
            #[cfg(feature = "nfc")]
            Format::NfcType2 => "nfc-type2",
        }
    }

    pub(crate) fn raw(self) -> *const native::tlv_format_t {
        // SAFETY: only the address of an immutable static is taken; the
        // static lives for the whole program and is never written. Newer
        // compilers treat this as safe, but the MSRV (1.70) needs `unsafe`.
        #[allow(unused_unsafe)]
        unsafe {
            match self {
                Format::Ber => ptr::addr_of!(native::tlv_format_ber),
                Format::Emv => ptr::addr_of!(native::tlv_format_emv),
                Format::Cer => ptr::addr_of!(native::tlv_format_cer),
                Format::Der => ptr::addr_of!(native::tlv_format_der),
                #[cfg(feature = "lldp")]
                Format::Lldp => ptr::addr_of!(native::tlv_format_lldp),
                #[cfg(feature = "nfc")]
                Format::NfcType2 => ptr::addr_of!(native::tlv_format_nfc_type2),
            }
        }
    }
}

impl fmt::Display for Format {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

impl FromStr for Format {
    type Err = Error;

    /// Parses a format name as returned by [`Format::name`], ignoring case.
    ///
    /// Fails with [`Error::InvalidArg`] for an unknown name.
    fn from_str(s: &str) -> Result<Format, Error> {
        Format::ALL
            .into_iter()
            .find(|format| format.name().eq_ignore_ascii_case(s))
            .ok_or(Error::InvalidArg)
    }
}
