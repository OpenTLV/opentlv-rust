// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Marek Cingel

//! Formats composed from Core field encodings or a packed header.

use std::fmt;
use std::mem::MaybeUninit;
use std::os::raw::{c_int, c_void};
use std::ptr::{self, addr_of, addr_of_mut};
use std::sync::Arc;

use opentlv_sys as native;

use crate::error::{Error, Result};
use crate::field::{ConstructedBit, IdentifierEncoding, LengthEncoding, LengthPolicy, PackedField};
use crate::fixed_format::{ByteOrder, ElementOrder, LengthScope};
use crate::format::FormatRef;
use crate::tag::Tag;

/// Packed identifier tables hold 2^width entries; wider fields would need
/// multi-megabyte tables.
const PACKED_TAG_MAX_WIDTH: u32 = 16;

/// One identifier and one length encoding, composed into a definite Format by
/// [`ComposedFormat::fields`].
///
/// The default is TLV order, a count of the Value bytes, no tag-only
/// identifiers and every Value opaque.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FieldComposition {
    tag: IdentifierEncoding,
    length: LengthEncoding,
    element_order: ElementOrder,
    length_scope: LengthScope,
    tag_only: Vec<Tag>,
    constructed: Option<ConstructedBit>,
}

impl FieldComposition {
    /// Compose `tag` with `length`.
    pub fn new(tag: impl Into<IdentifierEncoding>, length: impl Into<LengthEncoding>) -> Self {
        FieldComposition {
            tag: tag.into(),
            length: length.into(),
            element_order: ElementOrder::Tlv,
            length_scope: LengthScope::Value,
            tag_only: Vec::new(),
            constructed: None,
        }
    }

    /// Select the wire order of the identifier and length fields.
    pub fn with_order(mut self, order: ElementOrder) -> Self {
        self.element_order = order;
        self
    }

    /// Select what the length field counts; `TagAndValue` also counts the
    /// identifier's wire bytes.
    pub fn with_length_scope(mut self, scope: LengthScope) -> Self {
        self.length_scope = scope;
        self
    }

    /// Identifiers that omit Length and Value and require an empty Value.
    /// They need TLV order and Value scope, and imply neither skipping nor
    /// termination.
    pub fn with_tag_only(mut self, tags: impl IntoIterator<Item = Tag>) -> Self {
        self.tag_only = tags.into_iter().collect();
        self
    }

    /// Classify nesting by an identifier bit; without it every Value is opaque.
    pub fn with_constructed(mut self, constructed: ConstructedBit) -> Self {
        self.constructed = Some(constructed);
        self
    }
}

/// A definite packed Tag/Length header, composed by [`ComposedFormat::packed`]
/// (`tlv_packed_layout_t`).
///
/// Both fields share one `header_size`-byte header integer, big-endian by
/// default; their bits must not overlap and unused bits are zeroed on encode.
/// Decoded Tags are the field value as `tag_size` big-endian bytes, by default
/// the fewest bytes holding the tag field.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PackedLayout {
    header_size: usize,
    tag: PackedField,
    length: PackedField,
    byte_order: ByteOrder,
    tag_size: Option<usize>,
    length_scope: LengthScope,
    constructed: Option<ConstructedBit>,
}

impl PackedLayout {
    /// A `header_size`-byte header holding a `tag` and a `length` field.
    pub fn new(header_size: usize, tag: PackedField, length: PackedField) -> Self {
        PackedLayout {
            header_size,
            tag,
            length,
            byte_order: ByteOrder::Big,
            tag_size: None,
            length_scope: LengthScope::Value,
            constructed: None,
        }
    }

    /// Select the byte order of the header integer.
    pub fn with_byte_order(mut self, order: ByteOrder) -> Self {
        self.byte_order = order;
        self
    }

    /// Select the canonical Tag width, at most 8 bytes.
    pub fn with_tag_size(mut self, tag_size: usize) -> Self {
        self.tag_size = Some(tag_size);
        self
    }

    /// Select what the length field counts; `TagAndValue` also counts the
    /// header bytes holding the tag.
    pub fn with_length_scope(mut self, scope: LengthScope) -> Self {
        self.length_scope = scope;
        self
    }

    /// Classify nesting by a canonical Tag bit; without it every Value is opaque.
    pub fn with_constructed(mut self, constructed: ConstructedBit) -> Self {
        self.constructed = Some(constructed);
        self
    }
}

/// A Format composed through Core's generic composition mechanisms.
///
/// Build one with [`ComposedFormat::fields`] or [`ComposedFormat::packed`] and
/// pass `&format` wherever a format is accepted: [`Reader`](crate::Reader),
/// [`TreeReader`](crate::TreeReader), [`Writer`](crate::Writer),
/// [`TreeWriter`](crate::TreeWriter), [`Document`](crate::Document),
/// [`StructureSchema`](crate::StructureSchema) and, through
/// [`ProgramOptions::composed_format`](crate::ProgramOptions::composed_format)
/// and [`QuerySchema::with_composed_format`](crate::QuerySchema::with_composed_format),
/// Query. It owns the native descriptor, its configuration and every table Core
/// borrows. Clones share that owner and its native identity; share clones of
/// one owner when combining Query with a Reader or Document, because
/// independently composed equal configurations are different Formats to Core.
///
/// ```
/// use opentlv::{
///     ByteOrder, ComposedFormat, ElementOrder, FieldComposition, FixedIdentifier, FixedLength,
///     Reader, Tag, Writer,
/// };
///
/// let format = ComposedFormat::fields(
///     &FieldComposition::new(FixedIdentifier::new(1), FixedLength::new(2, ByteOrder::Big))
///         .with_order(ElementOrder::Ltv),
/// )
/// .unwrap();
/// let mut buf = [0u8; 8];
/// let mut writer = Writer::with_format(&mut buf, &format);
/// writer.write(&Tag::from_bytes(&[0x07]), &[0xAA]).unwrap();
/// assert_eq!(writer.written(), [0x00, 0x01, 0x07, 0xAA]);
/// let element = Reader::with_format(writer.written(), &format).next().unwrap().unwrap();
/// assert_eq!(element.value(), [0xAA]);
/// ```
///
/// Readers, Documents and decoded sources borrow the Format, so it must
/// outlive them; packed Tags, for example, borrow its identifier table:
///
/// ```compile_fail
/// use opentlv::{ComposedFormat, PackedField, PackedLayout};
///
/// let wire = [0x02, 0x00];
/// let decoded = {
///     let layout = PackedLayout::new(2, PackedField::new(9, 7), PackedField::new(0, 9));
///     let format = ComposedFormat::packed(&layout).unwrap();
///     opentlv::decode(&wire, &format).unwrap()
/// };
/// println!("{:?}", decoded.element);
/// ```
#[derive(Clone)]
pub struct ComposedFormat {
    owner: Arc<Owner>,
}

#[derive(Debug)]
enum Composition {
    Fields(FieldComposition),
    Packed(PackedLayout),
}

struct Owner {
    format: native::tlv_format_t,
    native: *mut Native,
    composition: Composition,
}

// SAFETY: the descriptor and its configuration are immutable after
// construction and reference only the owned allocation and static C callbacks.
unsafe impl Send for Owner {}
unsafe impl Sync for Owner {}

impl Drop for Owner {
    fn drop(&mut self) {
        // SAFETY: `native` came from Box::into_raw and is released only here,
        // after the last clone that could reach the descriptor.
        drop(unsafe { Box::from_raw(self.native) });
    }
}

/// Every descriptor context points at `config`, the first field, so the
/// constructed predicate wrapper finds the owner from any composition.
#[repr(C)]
struct Native {
    config: Config,
    constructed: native::tlv_constructed_bit_t,
    fields: Fields,
    /// Field source of tagged variable framing.
    variable: native::tlv_variable_format_t,
    tag_policy: native::tlv_identifier_policy_t,
    length_policy: native::tlv_length_policy_t,
    _tables: Tables,
}

/// Tables Core borrows. Pointers into them are taken before they move into
/// `Native`; a move does not reallocate their storage.
#[derive(Default)]
struct Tables {
    _forbidden: Vec<u8>,
    _tag_only: Vec<native::tlv_tag_t>,
    /// Tag-only identifier bytes or the packed identifier table.
    _tag_storage: Vec<u8>,
}

impl Native {
    /// Moves the configuration to its final heap address, released by `Owner`.
    fn allocate(
        fields: Fields,
        constructed: Option<ConstructedBit>,
        tables: Tables,
    ) -> *mut Native {
        let policy = native::tlv_identifier_policy_t {
            forbidden_leading: ptr::null(),
            forbidden_leading_count: 0,
            reject_zero_first_payload: 0,
            require_minimal: 0,
        };
        Box::into_raw(Box::new(Native {
            // SAFETY: every Config variant is plain data; zero is a valid
            // placeholder until composition initializes one.
            config: unsafe { std::mem::zeroed() },
            constructed: constructed.map_or(
                native::tlv_constructed_bit_t {
                    byte_index: 0,
                    mask: 0,
                    value: 0,
                },
                |bit| bit.raw(),
            ),
            fields,
            // SAFETY: plain data, initialized before use.
            variable: unsafe { std::mem::zeroed() },
            tag_policy: policy,
            length_policy: LengthPolicy::default().raw(),
            _tables: tables,
        }))
    }
}

/// A table pointer for Core: null for an empty table.
fn table<T>(table: &[T]) -> (*const T, usize) {
    if table.is_empty() {
        (ptr::null(), 0)
    } else {
        (table.as_ptr(), table.len())
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
union Config {
    binary: native::tlv_binary_composition_t,
    tagged_binary: native::tlv_tagged_binary_composition_t,
    variable: native::tlv_variable_format_t,
    escaped: native::tlv_escaped_format_t,
    fields: native::tlv_field_composition_t,
    tagged_fields: native::tlv_tagged_fields_composition_t,
    packed: native::tlv_packed_layout_t,
}

/// Field adapter context for pairs without a dedicated Core composition.
struct Fields {
    tag: NativeTag,
    length: NativeLength,
}

#[derive(Clone, Copy)]
enum NativeTag {
    Fixed(native::tlv_fixed_identifier_t),
    Variable(native::tlv_variable_identifier_t),
}

#[derive(Clone, Copy)]
enum NativeLength {
    Fixed(native::tlv_fixed_length_t),
    Variable(native::tlv_variable_length_t),
    Escaped(native::tlv_escaped_length_t),
}

unsafe extern "C" fn is_constructed(
    context: *const c_void,
    tag: *const native::tlv_tag_t,
) -> c_int {
    // SAFETY: every installed descriptor context is `Native::config`, at
    // offset zero of the live `Native`.
    unsafe {
        let native = context.cast::<Native>();
        native::tlv_constructed_bit_predicate(addr_of!((*native).constructed).cast(), tag)
    }
}

unsafe extern "C" fn read_tag(
    context: *const c_void,
    data: *const u8,
    size: usize,
    tag: *mut native::tlv_tag_t,
    consumed: *mut usize,
) -> native::tlv_result_t {
    // SAFETY: the field composition context is the owner's live `Fields`;
    // Core supplies valid buffers.
    unsafe {
        match &(*context.cast::<Fields>()).tag {
            NativeTag::Fixed(config) => {
                native::tlv_fixed_identifier_read(config, data, size, tag, consumed)
            }
            NativeTag::Variable(config) => {
                native::tlv_variable_identifier_read(config, data, size, tag, consumed)
            }
        }
    }
}

unsafe extern "C" fn write_tag(
    context: *const c_void,
    tag: *const native::tlv_tag_t,
    data: *mut u8,
    capacity: usize,
    written: *mut usize,
) -> native::tlv_result_t {
    // SAFETY: as in read_tag.
    unsafe {
        match &(*context.cast::<Fields>()).tag {
            NativeTag::Fixed(config) => {
                native::tlv_fixed_identifier_write(config, tag, data, capacity, written)
            }
            NativeTag::Variable(config) => {
                native::tlv_variable_identifier_write(config, tag, data, capacity, written)
            }
        }
    }
}

unsafe extern "C" fn read_length(
    context: *const c_void,
    data: *const u8,
    size: usize,
    length: *mut native::tlv_size_t,
    consumed: *mut usize,
) -> native::tlv_result_t {
    // SAFETY: as in read_tag.
    unsafe {
        match &(*context.cast::<Fields>()).length {
            NativeLength::Fixed(config) => {
                native::tlv_fixed_length_read(config, data, size, length, consumed)
            }
            NativeLength::Variable(config) => {
                native::tlv_variable_length_read(config, data, size, length, consumed)
            }
            NativeLength::Escaped(config) => {
                native::tlv_escaped_length_read(config, data, size, length, consumed)
            }
        }
    }
}

unsafe extern "C" fn write_length(
    context: *const c_void,
    length: native::tlv_size_t,
    data: *mut u8,
    capacity: usize,
    written: *mut usize,
) -> native::tlv_result_t {
    // SAFETY: as in read_tag.
    unsafe {
        match &(*context.cast::<Fields>()).length {
            NativeLength::Fixed(config) => {
                native::tlv_fixed_length_write(config, length, data, capacity, written)
            }
            NativeLength::Variable(config) => {
                native::tlv_variable_length_write(config, length, data, capacity, written)
            }
            NativeLength::Escaped(config) => {
                native::tlv_escaped_length_write(config, length, data, capacity, written)
            }
        }
    }
}

/// Generic field composition validates configuration only when a field is
/// used. A sizing write checks configuration first, so only argument and
/// configuration errors reject the encodings here.
///
/// # Safety
///
/// `fields` must be the owner's live adapter context.
unsafe fn probe_fields(fields: *const Fields) -> native::tlv_result_t {
    let probe = 0u8;
    let tag = native::tlv_tag_t {
        data: &probe,
        size: 1,
    };
    let mut written = 0;
    let rejected = |code| code == native::TLV_ERR_INVALID_ARG || code == native::TLV_ERR_NULL_ARG;
    // SAFETY: sizing writes with a live context and no destination.
    let code = unsafe { write_tag(fields.cast(), &tag, ptr::null_mut(), 0, &mut written) };
    if rejected(code) {
        return code;
    }
    // SAFETY: as above.
    let code = unsafe { write_length(fields.cast(), 0, ptr::null_mut(), 0, &mut written) };
    if rejected(code) {
        code
    } else {
        native::TLV_OK
    }
}

impl ComposedFormat {
    /// Compose a definite Format from one identifier and one length encoding.
    ///
    /// The C composition follows from the encodings: a fixed identifier and a
    /// fixed length use `tlv_fixed_format_init()` or, with tag-only
    /// identifiers, `tlv_tagged_binary_format_init()`; a variable identifier
    /// and a variable length use `tlv_variable_format_init()` or
    /// `tlv_variable_fields_init()` with `tlv_tagged_fields_format_init()`; a
    /// fixed identifier with an escaped length uses `tlv_escaped_format_init()`.
    /// Other pairs compose the field primitives through
    /// `tlv_fields_format_init()` or `tlv_tagged_fields_format_init()`.
    ///
    /// EMV element framing, for example:
    ///
    /// ```
    /// use opentlv::{
    ///     ByteOrder, ComposedFormat, ConstructedBit, FieldComposition, IdentifierPolicy,
    ///     LengthPolicy, VariableIdentifier, VariableLength,
    /// };
    ///
    /// let emv = ComposedFormat::fields(
    ///     &FieldComposition::new(
    ///         VariableIdentifier::new(0x1F, 0x1F, 0x80, 0x7F, 2).with_policy(IdentifierPolicy {
    ///             forbidden_leading: vec![0x00],
    ///             reject_zero_first_payload: true,
    ///             ..IdentifierPolicy::default()
    ///         }),
    ///         VariableLength::new(0x80, 0x7F, ByteOrder::Big).with_policy(LengthPolicy {
    ///             max_long_octets: 2,
    ///             max_value: 65535,
    ///             ..LengthPolicy::default()
    ///         }),
    ///     )
    ///     .with_constructed(ConstructedBit::new(0, 0x20, 0x20)),
    /// )
    /// .unwrap();
    /// # let _ = emv;
    /// ```
    ///
    /// # Errors
    ///
    /// [`Error::InvalidArg`] for an invalid encoding, ordering, scope,
    /// tag-only table or constructed predicate, or another error Core reports
    /// for an unrepresentable tag-only identifier.
    pub fn fields(composition: &FieldComposition) -> Result<ComposedFormat> {
        let tag_policy = match &composition.tag {
            IdentifierEncoding::Variable(tag) => tag.policy.clone(),
            IdentifierEncoding::Fixed(_) => None,
        };
        let length_policy = match &composition.length {
            LengthEncoding::Variable(length) => length.policy,
            _ => None,
        };
        let forbidden = tag_policy
            .as_ref()
            .map_or_else(Vec::new, |policy| policy.forbidden_leading.clone());
        let tag_storage: Vec<u8> = composition
            .tag_only
            .iter()
            .flat_map(|tag| tag.as_bytes().iter().copied())
            .collect();
        let mut cursor = tag_storage.as_ptr();
        let tag_only: Vec<native::tlv_tag_t> = composition
            .tag_only
            .iter()
            .map(|tag| {
                let size = tag.as_bytes().len();
                let tag = native::tlv_tag_t { data: cursor, size };
                cursor = cursor.wrapping_add(size);
                tag
            })
            .collect();
        let (forbidden_leading, forbidden_leading_count) = table(&forbidden);
        let tag_only_table = table(&tag_only);
        let fields = Fields {
            tag: match &composition.tag {
                IdentifierEncoding::Fixed(tag) => NativeTag::Fixed(tag.raw()),
                IdentifierEncoding::Variable(tag) => NativeTag::Variable(tag.raw()),
            },
            length: match &composition.length {
                LengthEncoding::Fixed(length) => NativeLength::Fixed(length.raw()),
                LengthEncoding::Variable(length) => NativeLength::Variable(length.raw()),
                LengthEncoding::Escaped(length) => NativeLength::Escaped(length.raw()),
            },
        };
        let tables = Tables {
            _forbidden: forbidden,
            _tag_only: tag_only,
            _tag_storage: tag_storage,
        };
        let native = Native::allocate(fields, composition.constructed, tables);
        let owner = Owner {
            format: placeholder(),
            native,
            composition: Composition::Fields(composition.clone()),
        };
        // SAFETY: `native` is the fresh allocation `owner` releases; every
        // pointer stored below targets it and stays valid until that release.
        unsafe {
            if let Some(policy) = &tag_policy {
                (*native).tag_policy = native::tlv_identifier_policy_t {
                    forbidden_leading,
                    forbidden_leading_count,
                    reject_zero_first_payload: policy.reject_zero_first_payload.into(),
                    require_minimal: policy.require_minimal.into(),
                };
                if let NativeTag::Variable(tag) = &mut (*native).fields.tag {
                    tag.policy = addr_of!((*native).tag_policy);
                }
            }
            if let Some(policy) = length_policy {
                (*native).length_policy = policy.raw();
                if let NativeLength::Variable(length) = &mut (*native).fields.length {
                    length.policy = addr_of!((*native).length_policy);
                }
            }
            let format = compose_fields(
                native,
                composition.element_order.into(),
                composition.length_scope.into(),
                composition.constructed.is_some(),
                tag_only_table,
            )?;
            finish(owner, format, composition.constructed.is_some())
        }
    }

    /// Compose a definite packed Tag/Length header (`tlv_packed_layout_t`).
    ///
    /// The binding owns the canonical identifier table Core borrows, so tag
    /// fields are limited to 16 bits. Configuration is checked with
    /// `tlv_packed_layout_validate()`. LLDP TLV framing, a 7-bit type above a
    /// 9-bit length:
    ///
    /// ```
    /// use opentlv::{ComposedFormat, Element, FormatRef, PackedField, PackedLayout, Tag};
    ///
    /// let lldp =
    ///     ComposedFormat::packed(&PackedLayout::new(2, PackedField::new(9, 7), PackedField::new(0, 9)))
    ///         .unwrap();
    /// let element = Element::new(Tag::from_bytes(&[0x05]), b"host");
    /// let encoded = FormatRef::from(&lldp).encode(&element).unwrap();
    /// assert_eq!(encoded, [0x0A, 0x04, b'h', b'o', b's', b't']);
    /// ```
    ///
    /// # Errors
    ///
    /// [`Error::Unsupported`] for a tag field wider than 16 bits,
    /// [`Error::InvalidArg`] for invalid widths, overlap, scope or constructed
    /// predicate, and [`Error::OutOfMemory`] if the table cannot be allocated.
    pub fn packed(layout: &PackedLayout) -> Result<ComposedFormat> {
        let tag_size = layout
            .tag_size
            .unwrap_or_else(|| (layout.tag.bit_width as usize).div_ceil(8).max(1));
        // Without a table Core would report the missing storage, not the width.
        if layout.tag.bit_width == 0 || tag_size == 0 || tag_size > 8 {
            return Err(Error::InvalidArg);
        }
        if layout.tag.bit_width > PACKED_TAG_MAX_WIDTH {
            return Err(Error::Unsupported);
        }
        let entries = 1usize << layout.tag.bit_width;
        let mut identifiers = Vec::new();
        identifiers
            .try_reserve_exact(entries * tag_size)
            .map_err(|_| Error::OutOfMemory)?;
        // Entry i is i in tag_size big-endian bytes.
        for entry in 0..entries as u64 {
            identifiers.extend_from_slice(&entry.to_be_bytes()[8 - tag_size..]);
        }
        let (tag_storage, tag_storage_size) = (identifiers.as_ptr(), identifiers.len());
        let fields = Fields {
            tag: NativeTag::Fixed(native::tlv_fixed_identifier_t { size: 0 }),
            length: NativeLength::Fixed(native::tlv_fixed_length_t {
                size: 0,
                byte_order: native::TLV_BYTE_ORDER_BIG_ENDIAN,
            }),
        };
        let tables = Tables {
            _tag_storage: identifiers,
            ..Tables::default()
        };
        let native = Native::allocate(fields, layout.constructed, tables);
        let owner = Owner {
            format: placeholder(),
            native,
            composition: Composition::Packed(layout.clone()),
        };
        // SAFETY: as in `fields`; the layout borrows the owned table.
        unsafe {
            let packed = addr_of_mut!((*native).config.packed);
            packed.write(native::tlv_packed_layout_t {
                header_size: layout.header_size,
                tag: layout.tag.raw(layout.header_size, layout.byte_order),
                length: layout.length.raw(layout.header_size, layout.byte_order),
                length_scope: layout.length_scope.into(),
                tag_storage,
                tag_size,
                tag_storage_size,
            });
            Error::check(native::tlv_packed_layout_validate(packed))?;
            let mut format = MaybeUninit::uninit();
            Error::check(native::tlv_format_init(
                format.as_mut_ptr(),
                packed.cast(),
                Some(native::tlv_packed_decode),
                Some(native::tlv_packed_measure),
                Some(native::tlv_packed_encode),
            ))?;
            finish(owner, format.assume_init(), layout.constructed.is_some())
        }
    }

    /// Borrow this Format's descriptor and its operations.
    pub fn as_format(&self) -> FormatRef<'_> {
        self.into()
    }

    pub(crate) fn raw(&self) -> *const native::tlv_format_t {
        &self.owner.format
    }
}

impl fmt::Debug for ComposedFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.owner.composition {
            Composition::Fields(composition) => f
                .debug_tuple("ComposedFormat::fields")
                .field(composition)
                .finish(),
            Composition::Packed(layout) => f
                .debug_tuple("ComposedFormat::packed")
                .field(layout)
                .finish(),
        }
    }
}

fn placeholder() -> native::tlv_format_t {
    native::tlv_format_t {
        context: ptr::null(),
        decode: None,
        measure: None,
        encode: None,
        is_constructed: None,
    }
}

/// Selects the dedicated Core composition for an encoding pair, or composes
/// the field primitives through the generic field composition.
///
/// # Safety
///
/// `native` must be the live allocation of the format's owner and `tag_only`
/// its owned table.
unsafe fn compose_fields(
    native: *mut Native,
    order: native::tlv_element_order_t,
    scope: native::tlv_length_scope_t,
    has_constructed: bool,
    (tag_only, count): (*const native::tlv_tag_t, usize),
) -> Result<native::tlv_format_t> {
    let mut format = MaybeUninit::uninit();
    // SAFETY: all configuration and tables are owned by `native`; Core
    // borrows them for the descriptor's lifetime.
    unsafe {
        let constructed = if has_constructed {
            addr_of!((*native).constructed)
        } else {
            ptr::null()
        };
        let config = addr_of_mut!((*native).config);
        let code = match ((*native).fields.tag, (*native).fields.length) {
            (NativeTag::Fixed(tag), NativeLength::Fixed(length)) => {
                let binary = native::tlv_binary_composition_t {
                    identifier: tag,
                    length,
                    element_order: order,
                    length_scope: scope,
                };
                if count == 0 {
                    (*config).binary = binary;
                    native::tlv_fixed_format_init(format.as_mut_ptr(), addr_of!((*config).binary))
                } else {
                    (*config).tagged_binary = native::tlv_tagged_binary_composition_t {
                        fields: binary,
                        tag_only,
                        count,
                    };
                    native::tlv_tagged_binary_format_init(
                        format.as_mut_ptr(),
                        addr_of!((*config).tagged_binary),
                    )
                }
            }
            (NativeTag::Variable(tag), NativeLength::Variable(length)) => {
                (*native).variable = native::tlv_variable_format_t {
                    identifier: tag,
                    length,
                    element_order: order,
                    length_scope: scope,
                    constructed,
                };
                if count == 0 {
                    (*config).variable = (*native).variable;
                    native::tlv_variable_format_init(
                        format.as_mut_ptr(),
                        addr_of!((*config).variable),
                    )
                } else {
                    let fields = addr_of_mut!((*config).tagged_fields.fields);
                    match native::tlv_variable_fields_init(fields, addr_of!((*native).variable)) {
                        native::TLV_OK => {
                            (*config).tagged_fields.tag_only = tag_only;
                            (*config).tagged_fields.count = count;
                            native::tlv_tagged_fields_format_init(
                                format.as_mut_ptr(),
                                addr_of!((*config).tagged_fields),
                            )
                        }
                        code => code,
                    }
                }
            }
            (NativeTag::Fixed(tag), NativeLength::Escaped(length)) => {
                (*config).escaped = native::tlv_escaped_format_t {
                    identifier: tag,
                    length,
                    element_order: order,
                    length_scope: scope,
                    tag_only,
                    count,
                };
                native::tlv_escaped_format_init(format.as_mut_ptr(), addr_of!((*config).escaped))
            }
            _ => {
                let adapter = addr_of!((*native).fields);
                match probe_fields(adapter) {
                    native::TLV_OK => {
                        let fields = native::tlv_field_composition_t {
                            context: adapter.cast(),
                            read_tag: Some(read_tag),
                            read_length: Some(read_length),
                            resolve: None,
                            write_tag: Some(write_tag),
                            write_length: Some(write_length),
                            element_order: order,
                            length_scope: scope,
                        };
                        if count == 0 {
                            (*config).fields = fields;
                            native::tlv_fields_format_init(
                                format.as_mut_ptr(),
                                addr_of!((*config).fields),
                            )
                        } else {
                            (*config).tagged_fields = native::tlv_tagged_fields_composition_t {
                                fields,
                                tag_only,
                                count,
                            };
                            native::tlv_tagged_fields_format_init(
                                format.as_mut_ptr(),
                                addr_of!((*config).tagged_fields),
                            )
                        }
                    }
                    code => code,
                }
            }
        };
        Error::check(code)?;
        Ok(format.assume_init())
    }
}

/// Installs the constructed predicate a composition lacks and publishes the owner.
///
/// # Safety
///
/// `format` must be a descriptor initialized over `owner`'s allocation.
unsafe fn finish(
    mut owner: Owner,
    mut format: native::tlv_format_t,
    has_constructed: bool,
) -> Result<ComposedFormat> {
    if has_constructed {
        // SAFETY: the allocation is live until `owner` drops.
        let bit = unsafe { (*owner.native).constructed };
        if bit.mask == 0 || bit.value & !bit.mask != 0 {
            return Err(Error::InvalidArg);
        }
        if format.is_constructed.is_none() {
            format.is_constructed = Some(is_constructed);
        }
    }
    owner.format = format;
    Ok(ComposedFormat {
        owner: Arc::new(owner),
    })
}
