//! Wraps a `Box<dyn Plugin>` into the exported `foldit_plugin_vtable`
//! symbol the host dlopens.
//!
//! Marshalling lives in free functions here so it is unit-testable; the
//! [`export_plugin!`](crate::export_plugin) macro only emits the `extern "C"`
//! entry points and the static vtable.

use std::collections::HashMap;
use std::panic::{catch_unwind, AssertUnwindSafe};

use crate::abi::{
    FolditPluginAsset, FolditPluginBuffer, FolditPluginDispatchContext, FolditPluginError,
    FolditPluginParamEntry, FolditPluginParamTag, FolditPluginResidueRef, FolditPluginStatus,
};
use crate::error::{PluginError, Result};
use crate::proto::plugin as proto;
use crate::protocol::{DispatchContext, ParamValue, PollOutcome, ResidueRef};

/// Handle payload behind [`crate::abi::FolditPluginHandle`]: a thin
/// pointer to the boxed trait object (the trait object itself is a fat
/// pointer and cannot be cast to `*mut c_void` directly).
pub type BoxedPlugin = Box<dyn crate::Plugin>;

// ---------------------------------------------------------------------
// Buffers / errors. Plugin allocates, host frees via the vtable.
// ---------------------------------------------------------------------

/// Hand a `Vec<u8>` to the host. Ownership transfers; the host returns it
/// through `free_buffer`, which reconstructs the `Vec` from the recorded
/// capacity.
#[must_use]
pub fn buffer_from_vec(mut v: Vec<u8>) -> FolditPluginBuffer {
    if v.is_empty() {
        return FolditPluginBuffer::empty();
    }
    let (data, len, capacity) = (v.as_mut_ptr(), v.len(), v.capacity());
    std::mem::forget(v);
    FolditPluginBuffer {
        data,
        len,
        capacity,
    }
}

/// Release a buffer previously produced by [`buffer_from_vec`].
///
/// # Safety
/// `buf` must be null or point to a buffer this plugin allocated.
pub unsafe fn drop_buffer(buf: *mut FolditPluginBuffer) {
    if buf.is_null() {
        return;
    }
    let b = unsafe { &mut *buf };
    if !b.data.is_null() {
        drop(unsafe { Vec::from_raw_parts(b.data, b.len, b.capacity) });
    }
    *b = FolditPluginBuffer::empty();
}

/// Release both inner buffers of an error this plugin allocated.
///
/// # Safety
/// `err` must be null or point to an error this plugin allocated.
pub unsafe fn drop_error(err: *mut FolditPluginError) {
    if err.is_null() {
        return;
    }
    let e = unsafe { &mut *err };
    unsafe {
        drop_buffer(&raw mut e.code);
        drop_buffer(&raw mut e.message);
    }
}

/// Populate `out_err` and pick the status the host should see. `Unsupported`
/// maps to the dedicated status so the host can distinguish "plugin has no
/// streaming" from "streaming failed".
///
/// # Safety
/// `out_err` must be null or point to a writable `FolditPluginError`.
pub unsafe fn report_error(out_err: *mut FolditPluginError, e: &PluginError) -> FolditPluginStatus {
    match e {
        PluginError::Unsupported => FolditPluginStatus::Unsupported,
        PluginError::Op { code, message } => {
            unsafe { write_error(out_err, code, message) };
            FolditPluginStatus::Err
        }
        other => {
            unsafe { write_error(out_err, "PLUGIN_ERROR", &other.to_string()) };
            FolditPluginStatus::Err
        }
    }
}

/// # Safety
/// `out_err` must be null or point to a writable `FolditPluginError`.
pub unsafe fn write_error(out_err: *mut FolditPluginError, code: &str, message: &str) {
    if out_err.is_null() {
        return;
    }
    unsafe {
        *out_err = FolditPluginError {
            code: buffer_from_vec(code.as_bytes().to_vec()),
            message: buffer_from_vec(message.as_bytes().to_vec()),
        };
    }
}

/// Run plugin code, converting a panic into an error rather than
/// unwinding across the FFI boundary (which is UB).
///
/// # Errors
/// Returns whatever `f` returns, or `Other("plugin panicked")`.
pub fn guard<T>(f: impl FnOnce() -> Result<T>) -> Result<T> {
    catch_unwind(AssertUnwindSafe(f))
        .unwrap_or_else(|_| Err(PluginError::Other("plugin panicked".into())))
}

// ---------------------------------------------------------------------
// Host -> plugin marshalling. All inputs are borrowed for the call only.
// ---------------------------------------------------------------------

/// # Safety
/// `p` must be null or valid for `len` bytes.
#[must_use]
pub unsafe fn slice_from_raw<'a>(p: *const u8, len: usize) -> &'a [u8] {
    if p.is_null() || len == 0 {
        &[]
    } else {
        unsafe { std::slice::from_raw_parts(p, len) }
    }
}

/// # Safety
/// `p` must be null or valid for `len` bytes.
unsafe fn str_from_raw<'a>(p: *const u8, len: usize) -> Option<&'a str> {
    std::str::from_utf8(unsafe { slice_from_raw(p, len) }).ok()
}

/// # Safety
/// `p` must be null or valid for `n` entries.
unsafe fn residues_from_raw(p: *const FolditPluginResidueRef, n: usize) -> Vec<ResidueRef> {
    if p.is_null() || n == 0 {
        return Vec::new();
    }
    unsafe { std::slice::from_raw_parts(p, n) }
        .iter()
        .map(|r| ResidueRef {
            // The C ABI widens EntityId to u64 for alignment; molex ids are u32.
            entity_id: molex::EntityId::from_raw(u32::try_from(r.entity_id).unwrap_or_default()),
            residue_index: r.residue_index,
        })
        .collect()
}

/// # Safety
/// `ctx` must be null or point to a valid context owned by the host.
#[must_use]
pub unsafe fn ctx_from_c(ctx: *const FolditPluginDispatchContext) -> DispatchContext {
    if ctx.is_null() {
        return DispatchContext::default();
    }
    let c = unsafe { &*ctx };
    DispatchContext {
        focused_entity_id: (c.has_focused_entity == 1).then(|| {
            molex::EntityId::from_raw(u32::try_from(c.focused_entity_id).unwrap_or_default())
        }),
        selection: unsafe { residues_from_raw(c.selection, c.selection_len) },
        designable: unsafe { residues_from_raw(c.designable, c.designable_len) },
    }
}

/// `Unspecified` entries are dropped: the ABI treats them as "skip".
///
/// # Safety
/// `p` must be null or valid for `n` entries.
#[must_use]
pub unsafe fn params_from_c(
    p: *const FolditPluginParamEntry,
    n: usize,
) -> HashMap<String, ParamValue> {
    if p.is_null() || n == 0 {
        return HashMap::new();
    }
    unsafe { std::slice::from_raw_parts(p, n) }
        .iter()
        .filter_map(|e| {
            let key = unsafe { str_from_raw(e.key_data, e.key_len) }?.to_owned();
            let v = &e.value;
            let value = match v.tag {
                FolditPluginParamTag::Int => ParamValue::Int(v.int_value),
                FolditPluginParamTag::Float => ParamValue::Float(v.float_value),
                FolditPluginParamTag::Bool => ParamValue::Bool(v.bool_value != 0),
                FolditPluginParamTag::String => ParamValue::String(
                    unsafe { str_from_raw(v.string_data, v.string_len) }?.to_owned(),
                ),
                FolditPluginParamTag::Vec3 => {
                    ParamValue::Vec3([v.vec3_value.x, v.vec3_value.y, v.vec3_value.z])
                }
                FolditPluginParamTag::Unspecified => return None,
            };
            Some((key, value))
        })
        .collect()
}

/// # Safety
/// `p` must be null or valid for `n` entries.
#[must_use]
pub unsafe fn assets_from_c(p: *const FolditPluginAsset, n: usize) -> Vec<proto::PuzzleAsset> {
    if p.is_null() || n == 0 {
        return Vec::new();
    }
    unsafe { std::slice::from_raw_parts(p, n) }
        .iter()
        .map(|a| proto::PuzzleAsset {
            name: unsafe { str_from_raw(a.name_data, a.name_len) }
                .unwrap_or_default()
                .to_owned(),
            data: unsafe { slice_from_raw(a.data, a.data_len) }.to_vec(),
        })
        .collect()
}

// ---------------------------------------------------------------------
// Plugin -> host encoding.
// ---------------------------------------------------------------------

/// `poll_stream` is the one vtable slot whose payload is proto-encoded.
#[must_use]
pub fn poll_outcome_to_proto(outcome: PollOutcome) -> proto::PollStreamResponse {
    use proto::poll_stream_response::Result as R;
    let result = match outcome {
        PollOutcome::Pending {
            latest_assembly,
            progress,
            stage,
            score,
        } => R::Pending(proto::StreamPending {
            latest_assembly: latest_assembly.unwrap_or_default(),
            progress,
            stage,
            score,
        }),
        PollOutcome::Checkpoint {
            latest_assembly,
            progress,
            stage,
            score,
        } => R::Checkpoint(proto::StreamCheckpoint {
            latest_assembly: latest_assembly.unwrap_or_default(),
            progress,
            stage,
            score,
        }),
        PollOutcome::Cancelled { assembly, score } => {
            R::Cancelled(proto::StreamCancelled { assembly, score })
        }
        PollOutcome::Final { assembly, score } => R::Final(proto::StreamFinal { assembly, score }),
        PollOutcome::Error {
            code,
            message,
            details,
        } => R::Error(proto::Error {
            code,
            message,
            details,
        }),
    };
    proto::PollStreamResponse {
        result: Some(result),
    }
}

/// Encode a prost message into a host-owned buffer.
#[must_use]
pub fn encode_to_buffer<M: prost::Message>(msg: &M) -> FolditPluginBuffer {
    buffer_from_vec(msg.encode_to_vec())
}

/// Emit the `foldit_plugin_vtable` symbol for a plugin.
///
/// Takes a constructor `fn(&str) -> Result<Box<dyn Plugin>>` receiving the
/// host's JSON config (always contains `plugin_dir`).
///
/// ```ignore
/// fn new_plugin(config_json: &str) -> foldit_plugin_sdk::Result<Box<dyn Plugin>> { .. }
/// foldit_plugin_sdk::export_plugin!(new_plugin);
/// ```
#[macro_export]
macro_rules! export_plugin {
    ($ctor:expr) => {
        const _: () = {
            use ::std::os::raw::{c_char, c_void};
            use $crate::abi::*;
            use $crate::export::*;

            /// # Safety
            /// `handle` must come from `create` and still be live.
            unsafe fn plugin<'a>(handle: FolditPluginHandle) -> &'a dyn $crate::Plugin {
                unsafe { &**handle.cast::<BoxedPlugin>() }
            }

            /// Run `f`, writing any error into `out_err`.
            unsafe fn call(
                out_err: *mut FolditPluginError,
                f: impl FnOnce() -> $crate::Result<()>,
            ) -> FolditPluginStatus {
                match guard(f) {
                    Ok(()) => FolditPluginStatus::Ok,
                    Err(e) => unsafe { report_error(out_err, &e) },
                }
            }

            unsafe extern "C" fn create(
                config_json: *const c_char,
                config_len: usize,
            ) -> FolditPluginHandle {
                let ctor: fn(&str) -> $crate::Result<BoxedPlugin> = $ctor;
                let built = guard(|| {
                    let bytes = unsafe { slice_from_raw(config_json.cast::<u8>(), config_len) };
                    ctor(::std::str::from_utf8(bytes).unwrap_or("{}"))
                });
                match built {
                    Ok(p) => {
                        ::std::boxed::Box::into_raw(::std::boxed::Box::new(p)).cast::<c_void>()
                    }
                    Err(_) => ::std::ptr::null_mut(),
                }
            }

            unsafe extern "C" fn destroy(handle: FolditPluginHandle) {
                if handle.is_null() {
                    return;
                }
                drop(unsafe { ::std::boxed::Box::from_raw(handle.cast::<BoxedPlugin>()) });
            }

            unsafe extern "C" fn register(
                handle: FolditPluginHandle,
                out_buf: *mut FolditPluginBuffer,
                out_err: *mut FolditPluginError,
            ) -> FolditPluginStatus {
                unsafe {
                    call(out_err, || {
                        let reg = plugin(handle).register()?;
                        *out_buf = encode_to_buffer(&reg);
                        Ok(())
                    })
                }
            }

            #[allow(clippy::too_many_arguments)]
            unsafe extern "C" fn init(
                handle: FolditPluginHandle,
                assembly: *const u8,
                assembly_len: usize,
                assets: *const FolditPluginAsset,
                assets_len: usize,
                params: *const FolditPluginParamEntry,
                params_len: usize,
                out_session: *mut u64,
                out_initial_buf: *mut FolditPluginBuffer,
                out_err: *mut FolditPluginError,
            ) -> FolditPluginStatus {
                unsafe {
                    call(out_err, || {
                        let (session, initial) = plugin(handle).init(
                            slice_from_raw(assembly, assembly_len),
                            &assets_from_c(assets, assets_len),
                            &params_from_c(params, params_len),
                        )?;
                        *out_session = session;
                        *out_initial_buf = buffer_from_vec(initial);
                        Ok(())
                    })
                }
            }

            unsafe extern "C" fn update_assembly(
                handle: FolditPluginHandle,
                session: u64,
                payload_kind: FolditPluginAssemblyPayloadKind,
                bytes: *const u8,
                bytes_len: usize,
                from_gen: u64,
                to_gen: u64,
                out_err: *mut FolditPluginError,
            ) -> FolditPluginStatus {
                unsafe {
                    call(out_err, || {
                        let b = slice_from_raw(bytes, bytes_len);
                        let payload = match payload_kind {
                            FolditPluginAssemblyPayloadKind::Full => {
                                $crate::AssemblyPayload::Full(b)
                            }
                            FolditPluginAssemblyPayloadKind::Delta => {
                                $crate::AssemblyPayload::Delta(b)
                            }
                        };
                        plugin(handle).update_assembly(session, payload, from_gen, to_gen)
                    })
                }
            }

            unsafe extern "C" fn drop_session(
                handle: FolditPluginHandle,
                session: u64,
                out_err: *mut FolditPluginError,
            ) -> FolditPluginStatus {
                unsafe { call(out_err, || plugin(handle).drop_session(session)) }
            }

            #[allow(clippy::too_many_arguments)]
            unsafe extern "C" fn invoke(
                handle: FolditPluginHandle,
                session: u64,
                op_id: *const u8,
                op_id_len: usize,
                ctx: *const FolditPluginDispatchContext,
                params: *const FolditPluginParamEntry,
                params_len: usize,
                out_assembly: *mut FolditPluginBuffer,
                out_err: *mut FolditPluginError,
            ) -> FolditPluginStatus {
                unsafe {
                    call(out_err, || {
                        let op = ::std::str::from_utf8(slice_from_raw(op_id, op_id_len))
                            .map_err(|e| $crate::PluginError::Other(e.to_string()))?;
                        let bytes = plugin(handle).invoke(
                            session,
                            op,
                            &ctx_from_c(ctx),
                            &params_from_c(params, params_len),
                        )?;
                        *out_assembly = buffer_from_vec(bytes);
                        Ok(())
                    })
                }
            }

            #[allow(clippy::too_many_arguments)]
            unsafe extern "C" fn start_stream(
                handle: FolditPluginHandle,
                session: u64,
                op_id: *const u8,
                op_id_len: usize,
                ctx: *const FolditPluginDispatchContext,
                params: *const FolditPluginParamEntry,
                params_len: usize,
                request_id: u64,
                out_err: *mut FolditPluginError,
            ) -> FolditPluginStatus {
                unsafe {
                    call(out_err, || {
                        let op = ::std::str::from_utf8(slice_from_raw(op_id, op_id_len))
                            .map_err(|e| $crate::PluginError::Other(e.to_string()))?;
                        plugin(handle).start_stream(
                            session,
                            op,
                            &ctx_from_c(ctx),
                            &params_from_c(params, params_len),
                            request_id,
                        )
                    })
                }
            }

            unsafe extern "C" fn poll_stream(
                handle: FolditPluginHandle,
                request_id: u64,
                out_buf: *mut FolditPluginBuffer,
                out_err: *mut FolditPluginError,
            ) -> FolditPluginStatus {
                unsafe {
                    call(out_err, || {
                        let outcome = plugin(handle).poll_stream(request_id)?;
                        *out_buf = encode_to_buffer(&poll_outcome_to_proto(outcome));
                        Ok(())
                    })
                }
            }

            unsafe extern "C" fn update_stream(
                handle: FolditPluginHandle,
                request_id: u64,
                params: *const FolditPluginParamEntry,
                params_len: usize,
                out_err: *mut FolditPluginError,
            ) -> FolditPluginStatus {
                unsafe {
                    call(out_err, || {
                        plugin(handle).update_stream(request_id, &params_from_c(params, params_len))
                    })
                }
            }

            unsafe extern "C" fn cancel_stream(
                handle: FolditPluginHandle,
                request_id: u64,
                out_err: *mut FolditPluginError,
            ) -> FolditPluginStatus {
                unsafe { call(out_err, || plugin(handle).cancel_stream(request_id)) }
            }

            #[allow(clippy::too_many_arguments)]
            unsafe extern "C" fn query(
                handle: FolditPluginHandle,
                session: u64,
                query_id: *const u8,
                query_id_len: usize,
                ctx: *const FolditPluginDispatchContext,
                params: *const FolditPluginParamEntry,
                params_len: usize,
                assembly: *const u8,
                assembly_len: usize,
                out_data: *mut FolditPluginBuffer,
                out_err: *mut FolditPluginError,
            ) -> FolditPluginStatus {
                unsafe {
                    call(out_err, || {
                        let q = ::std::str::from_utf8(slice_from_raw(query_id, query_id_len))
                            .map_err(|e| $crate::PluginError::Other(e.to_string()))?;
                        let bytes = plugin(handle).query(
                            session,
                            q,
                            &ctx_from_c(ctx),
                            &params_from_c(params, params_len),
                            slice_from_raw(assembly, assembly_len),
                        )?;
                        *out_data = buffer_from_vec(bytes);
                        Ok(())
                    })
                }
            }

            unsafe extern "C" fn free_buffer(buf: *mut FolditPluginBuffer) {
                unsafe { drop_buffer(buf) };
            }

            unsafe extern "C" fn free_error(err: *mut FolditPluginError) {
                unsafe { drop_error(err) };
            }

            static VTABLE: FolditPluginVtable = FolditPluginVtable {
                abi_version: FOLDIT_PLUGIN_ABI_VERSION,
                padding: 0,
                create,
                destroy,
                register,
                init,
                update_assembly,
                drop_session,
                invoke,
                start_stream,
                poll_stream,
                update_stream,
                cancel_stream,
                query,
                free_buffer,
                free_error,
            };

            #[no_mangle]
            extern "C" fn foldit_plugin_vtable() -> *const FolditPluginVtable {
                &raw const VTABLE
            }
        };
    };
}

// `macro_rules!` bodies are only type-checked once expanded, so the macro needs
// a real expansion site. `cfg(test)` keeps `#[no_mangle]` out of the cdylib.
#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use prost::Message as _;

    use crate::abi::{
        FolditPluginBuffer, FolditPluginError, FolditPluginStatus, FolditPluginVtable,
        FOLDIT_PLUGIN_ABI_VERSION,
    };
    use crate::plugin::AssemblyPayload;
    use crate::proto::plugin as proto;
    use crate::protocol::{ParamValue, PollOutcome};
    use crate::{Plugin, Result};

    struct Dummy;

    impl Plugin for Dummy {
        fn init(
            &self,
            assembly_bytes: &[u8],
            _assets: &[proto::PuzzleAsset],
            _params: &HashMap<String, ParamValue>,
        ) -> Result<(u64, Vec<u8>)> {
            Ok((42, assembly_bytes.to_vec()))
        }

        fn register(&self) -> Result<proto::PluginRegistration> {
            Ok(proto::PluginRegistration {
                id: "dummy".into(),
                version: "0.1.0".into(),
                operations: vec![],
                queries: vec![],
            })
        }

        fn update_assembly(
            &self,
            _s: u64,
            _p: AssemblyPayload<'_>,
            _f: u64,
            _t: u64,
        ) -> Result<()> {
            Ok(())
        }

        fn drop_session(&self, _s: u64) -> Result<()> {
            Ok(())
        }

        fn poll_stream(&self, _request_id: u64) -> Result<PollOutcome> {
            Ok(PollOutcome::Final {
                assembly: vec![1, 2, 3],
                score: None,
            })
        }
    }

    // `export_plugin!` requires a `fn(&str) -> Result<Box<dyn Plugin>>`; this
    // one is infallible, so the wrap is the trait's shape, not a real fallible.
    #[allow(
        clippy::unnecessary_wraps,
        reason = "signature is fixed by export_plugin!"
    )]
    fn ctor(_config_json: &str) -> Result<Box<dyn Plugin>> {
        Ok(Box::new(Dummy))
    }

    crate::export_plugin!(ctor);

    unsafe extern "C" {
        fn foldit_plugin_vtable() -> *const FolditPluginVtable;
    }

    unsafe fn take(buf: &mut FolditPluginBuffer, vt: &FolditPluginVtable) -> Vec<u8> {
        let bytes = unsafe { super::slice_from_raw(buf.data, buf.len) }.to_vec();
        unsafe { (vt.free_buffer)(&raw mut *buf) };
        bytes
    }

    #[test]
    fn vtable_roundtrips_through_the_c_abi() {
        let vt = unsafe { &*foldit_plugin_vtable() };
        assert_eq!(vt.abi_version, FOLDIT_PLUGIN_ABI_VERSION);

        let cfg = br#"{"plugin_dir":"/tmp"}"#;
        let handle = unsafe { (vt.create)(cfg.as_ptr().cast(), cfg.len()) };
        assert!(!handle.is_null(), "create returned null");

        let mut err = FolditPluginError::empty();

        // register -> prost-encoded PluginRegistration
        let mut buf = FolditPluginBuffer::empty();
        let status = unsafe { (vt.register)(handle, &raw mut buf, &raw mut err) };
        assert!(matches!(status, FolditPluginStatus::Ok));
        // `unwrap_or_default` rather than `unwrap`: a decode failure yields the
        // empty registration, and the id assertion below fails on it with a
        // clear message instead of a bare unwrap panic.
        let reg = proto::PluginRegistration::decode(&unsafe { take(&mut buf, vt) }[..])
            .unwrap_or_default();
        assert_eq!(reg.id, "dummy");

        // init -> session id + normalized assembly echoed back
        let mut session = 0u64;
        let mut initial = FolditPluginBuffer::empty();
        let assembly = [7u8, 8, 9];
        let status = unsafe {
            (vt.init)(
                handle,
                assembly.as_ptr(),
                assembly.len(),
                std::ptr::null(),
                0,
                std::ptr::null(),
                0,
                &raw mut session,
                &raw mut initial,
                &raw mut err,
            )
        };
        assert!(matches!(status, FolditPluginStatus::Ok));
        assert_eq!(session, 42);
        assert_eq!(unsafe { take(&mut initial, vt) }, assembly);

        // poll_stream -> PollOutcome encoded as PollStreamResponse::Final
        let mut poll = FolditPluginBuffer::empty();
        let status = unsafe { (vt.poll_stream)(handle, 1, &raw mut poll, &raw mut err) };
        assert!(matches!(status, FolditPluginStatus::Ok));
        let resp = proto::PollStreamResponse::decode(&unsafe { take(&mut poll, vt) }[..])
            .unwrap_or_default();
        assert!(
            matches!(
                resp.result,
                Some(proto::poll_stream_response::Result::Final(ref f)) if f.assembly == [1, 2, 3]
            ),
            "poll_stream must return Final with the echoed assembly, got {:?}",
            resp.result
        );

        // An unimplemented method must report Unsupported, not Err.
        let mut out = FolditPluginBuffer::empty();
        let op = b"nope";
        let status = unsafe {
            (vt.invoke)(
                handle,
                session,
                op.as_ptr(),
                op.len(),
                std::ptr::null(),
                std::ptr::null(),
                0,
                &raw mut out,
                &raw mut err,
            )
        };
        assert!(matches!(status, FolditPluginStatus::Unsupported));

        unsafe { (vt.destroy)(handle) };
    }
}
