// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Marek Cingel

//! Callback adaptation only; C owns traversal and continuation.
use crate::{Element, Error, ReaderDiagnostic, Result};
use opentlv_sys as native;
use std::{
    any::Any,
    ffi::c_void,
    marker::PhantomData,
    mem::MaybeUninit,
    panic::{catch_unwind, resume_unwind, AssertUnwindSafe},
};

/// Action returned by a Visitor callback.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i32)]
pub enum Visit {
    /// Continue canonical traversal.
    Continue = 0,
    /// Stop successfully with the current element consumed/published.
    Stop = 1,
    /// Return Error::Visitor.
    Error = 2,
}

struct Context<'a, F> {
    callback: F,
    panic: Option<Box<dyn Any + Send>>,
    error: Option<Error>,
    lifetime: PhantomData<&'a [u8]>,
}

unsafe extern "C" fn tree_callback<'a, F: FnMut(Element<'a>, usize, usize) -> Visit>(
    element: *const native::tlv_element_t,
    depth: usize,
    offset: usize,
    context: *mut c_void,
) -> i32 {
    // SAFETY: run supplies this exact live Context for a synchronous C call.
    let context = unsafe { &mut *context.cast::<Context<'a, F>>() };
    let outcome = catch_unwind(AssertUnwindSafe(|| {
        // SAFETY: C publishes complete content backed by run's 'a storage contract.
        let element = unsafe { Element::from_raw(&*element) }?;
        Ok((context.callback)(element, depth, offset))
    }));
    match outcome {
        Ok(Ok(action)) => action as i32,
        Ok(Err(error)) => {
            context.error = Some(error);
            Visit::Error as i32
        }
        Err(panic) => {
            context.panic = Some(panic);
            Visit::Error as i32
        }
    }
}

unsafe extern "C" fn sequential_callback<'a, F: FnMut(Element<'a>, usize, usize) -> Visit>(
    element: *const native::tlv_element_t,
    context: *mut c_void,
) -> i32 {
    // SAFETY: forwards the same synchronous callback and context contract.
    unsafe { tree_callback::<F>(element, 0, 0, context) }
}

/// Exactly one cursor must be non-null, exclusively borrowed and backed by 'a input/Format.
pub(crate) unsafe fn run<'a, F: FnMut(Element<'a>, usize, usize) -> Visit>(
    reader: *mut native::tlv_reader_t,
    tree: *mut native::tlv_tree_reader_t,
    callback: F,
    matcher: *mut native::tlv_query_matcher_t,
) -> (Result<()>, Option<ReaderDiagnostic>) {
    // SAFETY: caller retains input, Format and frames for the traversal.
    unsafe {
        traverse(callback, |opaque, diagnostic| {
            if !matcher.is_null() {
                native::tlv_query_visit(tree, matcher, Some(tree_callback::<F>), opaque, diagnostic)
            } else if tree.is_null() {
                native::tlv_reader_visit_diag(
                    reader,
                    Some(sequential_callback::<F>),
                    opaque,
                    diagnostic,
                )
            } else {
                native::tlv_tree_reader_visit(tree, Some(tree_callback::<F>), opaque, diagnostic)
            }
        })
    }
}

/// Visit the matches of `query` in complete `'a` input without a caller Reader.
pub(crate) unsafe fn query_buffer<'a, F: FnMut(Element<'a>, usize, usize) -> Visit>(
    data: &'a [u8],
    format: *const native::tlv_format_t,
    query: &native::tlv_query_t,
    max_depth: usize,
    max_elements: usize,
    callback: F,
) -> (Result<()>, Option<ReaderDiagnostic>) {
    // SAFETY: caller retains the Format for 'a; C owns its traversal frames.
    unsafe {
        traverse(callback, |opaque, diagnostic| {
            native::tlv_query_visit_buffer(
                data.as_ptr(),
                data.len(),
                format,
                query,
                max_depth,
                max_elements,
                Some(tree_callback::<F>),
                opaque,
                diagnostic,
            )
        })
    }
}

/// `invoke` runs one C traversal with the callback context and a diagnostic.
unsafe fn traverse<'a, F: FnMut(Element<'a>, usize, usize) -> Visit>(
    callback: F,
    invoke: impl FnOnce(*mut c_void, *mut native::tlv_reader_diagnostic_t) -> i32,
) -> (Result<()>, Option<ReaderDiagnostic>) {
    let mut context = Context {
        callback,
        panic: None,
        error: None,
        lifetime: PhantomData,
    };
    let mut diagnostic = MaybeUninit::uninit();
    // SAFETY: local state remains live during the synchronous C traversal.
    let (code, diagnostic) = unsafe {
        native::tlv_reader_diagnostic_init(diagnostic.as_mut_ptr());
        let opaque = (&mut context as *mut Context<'a, F>).cast();
        let code = invoke(opaque, diagnostic.as_mut_ptr());
        (code, diagnostic.assume_init())
    };
    // Never unwind through C; resume only after the C adapter has returned.
    if let Some(panic) = context.panic {
        resume_unwind(panic);
    }
    let result = context.error.map_or_else(|| Error::check(code), Err);
    // SAFETY: diagnostics still refer to live input/Format and are copied here.
    let detail = result
        .is_err()
        .then(|| unsafe { ReaderDiagnostic::from_raw(&diagnostic) });
    (result, detail)
}
