// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Marek Cingel
//! Owned extension callbacks. Native C retains all language semantics.
use super::*;
use crate::{DefinitionRegistry, Tag};
use std::cell::RefCell;

type TagCallback = dyn Fn(&[u8]) -> crate::Result<i64> + Send + Sync;
/// Stable-ID semantic Tag adapter. Each optional callback is independently
/// advertised to C; raw Tag matching never needs either callback.
#[derive(Clone)]
pub struct QueryTagAdapter {
    /// Nonzero caller-assigned compatibility ID, retained in compiled images.
    pub id: u32,
    class: Option<Arc<TagCallback>>,
    number: Option<Arc<TagCallback>>,
}
impl std::fmt::Debug for QueryTagAdapter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("QueryTagAdapter")
            .field("id", &self.id)
            .field("class", &self.class.is_some())
            .field("number", &self.number.is_some())
            .finish()
    }
}
impl QueryTagAdapter {
    /// Create an adapter, then install at least one callback. Compilation rejects
    /// ID zero; callbacks must support concurrent independent executions.
    pub fn new(id: u32) -> Self {
        Self {
            id,
            class: None,
            number: None,
        }
    }
    /// Set semantic class lookup. Errors retain their original native status;
    /// callback panics are contained and map to InvalidValue.
    pub fn with_class<F>(mut self, callback: F) -> Self
    where
        F: Fn(&[u8]) -> crate::Result<i64> + Send + Sync + 'static,
    {
        self.class = Some(Arc::new(callback));
        self
    }
    /// Set semantic number lookup with the same ownership/error contract.
    pub fn with_number<F>(mut self, callback: F) -> Self
    where
        F: Fn(&[u8]) -> crate::Result<i64> + Send + Sync + 'static,
    {
        self.number = Some(Arc::new(callback));
        self
    }
}
pub(super) struct TagOwner {
    adapter: QueryTagAdapter,
    pub(super) native: native::tlv_query_tag_adapter_t,
}
impl TagOwner {
    pub(super) fn new(adapter: QueryTagAdapter) -> ProgramResult<Box<Self>> {
        if adapter.id == 0 || (adapter.class.is_none() && adapter.number.is_none()) {
            return Err(plain(native::TLV_ERR_INVALID_ARG).unwrap_err());
        }
        let mut owner = Box::new(Self {
            adapter,
            native: unsafe { zeroed() },
        });
        owner.native = native::tlv_query_tag_adapter_t {
            id: owner.adapter.id,
            context: ptr::addr_of!(owner.adapter).cast(),
            class_of: owner.adapter.class.as_ref().map(|_| tag_class as _),
            number_of: owner.adapter.number.as_ref().map(|_| tag_number as _),
        };
        Ok(owner)
    }
}
unsafe fn tag_lookup(
    context: *const c_void,
    tag: *const native::tlv_tag_t,
    output: *mut i64,
    class: bool,
) -> i32 {
    // SAFETY: context is retained in boxed program storage and C lends a Tag
    // and writable output for the synchronous call. No borrowed data escapes.
    let adapter = unsafe { &*context.cast::<QueryTagAdapter>() };
    let tag = unsafe { &*tag };
    let bytes = if tag.size == 0 {
        &[]
    } else {
        unsafe { slice::from_raw_parts(tag.data, tag.size) }
    };
    let callback = if class {
        &adapter.class
    } else {
        &adapter.number
    };
    let Some(callback) = callback else {
        return native::TLV_ERR_UNSUPPORTED;
    };
    match catch_unwind(AssertUnwindSafe(|| callback(bytes))) {
        Ok(Ok(value)) => {
            unsafe {
                *output = value;
            }
            native::TLV_OK
        }
        Ok(Err(error)) => error.code(),
        Err(_) => native::TLV_ERR_INVALID_VALUE,
    }
}
unsafe extern "C" fn tag_class(
    context: *const c_void,
    tag: *const native::tlv_tag_t,
    output: *mut i64,
) -> i32 {
    unsafe { tag_lookup(context, tag, output, true) }
}
unsafe extern "C" fn tag_number(
    context: *const c_void,
    tag: *const native::tlv_tag_t,
    output: *mut i64,
) -> i32 {
    unsafe { tag_lookup(context, tag, output, false) }
}

/// One explicit namespace owning an immutable generic Definition registry.
#[derive(Clone)]
pub struct QueryDefinitionScope {
    /// UTF-8 namespace, with no embedded NUL bytes.
    pub namespace: String,
    /// Shared definitions; duplicate labels preserve canonical ambiguity errors.
    pub definitions: Arc<DefinitionRegistry>,
}
type ResolverCallback = dyn Fn(&str, &str) -> crate::Result<Tag> + Send + Sync;
enum ResolverKind {
    Callback(Arc<ResolverCallback>),
    Definitions(DefinitionOwner),
    Emv,
}
/// Shared compile-only scoped resolver. C copies each returned Tag into its
/// image; programs do not retain resolver callbacks after compilation.
#[derive(Clone)]
pub struct QueryResolver(Arc<ResolverKind>);
impl std::fmt::Debug for QueryResolver {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("QueryResolver").finish_non_exhaustive()
    }
}
struct DefinitionOwner {
    _owners: Vec<QueryDefinitionScope>,
    _names: Vec<CString>,
    _registries: Box<[native::tlv_definition_registry_t]>,
    scopes: Box<[native::tlv_query_definition_scope_t]>,
}
// SAFETY: every native pointer refers to immutable retained heap storage.
unsafe impl Send for DefinitionOwner {}
unsafe impl Sync for DefinitionOwner {}
impl QueryResolver {
    /// Resolve native EMV base-dictionary symbols, including `emv:PAN` and
    /// unqualified `pan`. Lookup and ambiguity rules are provided by C.
    pub fn emv() -> Self {
        Self(Arc::new(ResolverKind::Emv))
    }
    /// Retain a scoped UTF-8 name callback. Empty namespace requests unqualified
    /// lookup. The callback must return the same Tag in both compilation passes.
    /// Original errors propagate; panics map to InvalidValue without unwinding C.
    pub fn new<F>(callback: F) -> Self
    where
        F: Fn(&str, &str) -> crate::Result<Tag> + Send + Sync + 'static,
    {
        Self(Arc::new(ResolverKind::Callback(Arc::new(callback))))
    }
    /// Compose the native Definition resolver over explicit immutable scopes.
    /// Unqualified names search all scopes; multiple matches remain InvalidArg.
    pub fn definitions(scopes: Vec<QueryDefinitionScope>) -> ProgramResult<Self> {
        let names = scopes
            .iter()
            .map(|scope| CString::new(scope.namespace.as_str()))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| plain(native::TLV_ERR_INVALID_ARG).unwrap_err())?;
        let registries = scopes
            .iter()
            .map(|scope| scope.definitions.raw())
            .collect::<Vec<_>>()
            .into_boxed_slice();
        let native_scopes = names
            .iter()
            .zip(registries.iter())
            .map(|(name, registry)| native::tlv_query_definition_scope_t {
                namespace_name: name.as_ptr(),
                definitions: registry,
            })
            .collect::<Vec<_>>()
            .into_boxed_slice();
        Ok(Self(Arc::new(ResolverKind::Definitions(DefinitionOwner {
            _owners: scopes,
            _names: names,
            _registries: registries,
            scopes: native_scopes,
        }))))
    }
}
pub(super) struct ResolverCall<'a> {
    pub(super) resolver: &'a QueryResolver,
    pub(super) tag: RefCell<Tag>,
}
pub(super) unsafe extern "C" fn resolve_callback(
    context: *const c_void,
    namespace: *const std::os::raw::c_char,
    namespace_size: usize,
    name: *const std::os::raw::c_char,
    name_size: usize,
    tag: *mut native::tlv_tag_t,
) -> i32 {
    let state = unsafe { &*context.cast::<ResolverCall<'_>>() };
    match &*state.resolver.0 {
        ResolverKind::Emv => unsafe {
            native::tlv_emv_query_resolve(
                ptr::null(),
                namespace,
                namespace_size,
                name,
                name_size,
                tag,
            )
        },
        ResolverKind::Definitions(owner) => {
            let resolver = native::tlv_query_definition_resolver_t {
                scopes: owner.scopes.as_ptr(),
                count: owner.scopes.len(),
            };
            // SAFETY: native scopes own all referenced registries/labels/Tags.
            unsafe {
                native::tlv_query_definition_resolve(
                    ptr::addr_of!(resolver).cast(),
                    namespace,
                    namespace_size,
                    name,
                    name_size,
                    tag,
                )
            }
        }
        ResolverKind::Callback(callback) => {
            let decode = |data, size| {
                if size == 0 {
                    Ok("")
                } else {
                    std::str::from_utf8(unsafe { slice::from_raw_parts(data as *const u8, size) })
                }
            };
            let (Ok(namespace), Ok(name)) =
                (decode(namespace, namespace_size), decode(name, name_size))
            else {
                return native::TLV_ERR_INVALID_ARG;
            };
            match catch_unwind(AssertUnwindSafe(|| callback(namespace, name))) {
                Ok(Ok(value)) => {
                    *state.tag.borrow_mut() = value;
                    unsafe {
                        *tag = state.tag.borrow().raw();
                    }
                    native::TLV_OK
                }
                Ok(Err(error)) => error.code(),
                Err(_) => native::TLV_ERR_INVALID_VALUE,
            }
        }
    }
}
