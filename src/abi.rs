//! C ABI for native plugins.
//!
//! Native plugins are shared libraries (`lib{id}.{dylib,so,dll}`) that
//! export the symbol `foldit_plugin_vtable` returning a pointer to a
//! [`FolditPluginVtable`]. The orchestrator (host) loads the dylib via
//! `libloading`, reads the vtable, and dispatches into the plugin
//! through the function pointers; no IPC, no proto serialization on
//! hot paths.
//!
//! ## ABI version
//!
//! [`FolditPluginVtable::abi_version`] is checked by the host on load.
//! Bump it whenever the vtable layout changes; old plugins fail to
//! load with a clear error.
//!
//! ## Memory ownership
//!
//! - **Plugin to host buffers** (`FolditPluginBuffer` from `register`, `score`,
//!   `invoke`, `query`, `poll_stream`): plugin allocates, host frees via
//!   [`FolditPluginVtable::free_buffer`].
//! - **Plugin to host errors** (`FolditPluginError` filled when a method returns
//!   `FOLDIT_PLUGIN_ERR`): plugin allocates the inner `code` / `message`
//!   buffers, host frees the whole struct via
//!   [`FolditPluginVtable::free_error`].
//! - **Host to plugin buffers** (assembly bytes, params bytes, session context):
//!   plugin must NOT retain pointers past the call return. Copy if needed.
//!
//! ## Threading
//!
//! Calls into a single plugin instance are serialized by the host (the
//! orchestrator owns each `Box<dyn Plugin>` exclusively). Plugin
//! authors don't need to make their internals thread-safe across
//! method calls, but each call may run on a different OS thread, so
//! per-instance state must not assume a single thread.

#![allow(non_camel_case_types)]

use std::os::raw::{c_char, c_void};

/// Current ABI version. Bump on any layout change.
pub const FOLDIT_PLUGIN_ABI_VERSION: u32 = 7;

/// Payload tag for [`FolditPluginVtable::update_assembly`].
///
/// `Full` carries fresh assembly bytes; discard prior state, decode and
/// install. `Delta` carries delta bytes; decode via molex's
/// `molex_delta_to_edits` and apply incrementally; preserves derived
/// plugin state across mutations. Plugins that don't track incremental
/// state may treat `Delta` the same as `Full` by reconstituting the
/// assembly from the decoded edits.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FolditPluginAssemblyPayloadKind {
    /// Payload is a fresh assembly snapshot.
    Full = 0,
    /// Payload is a delta edit list.
    Delta = 1,
}

/// Status code returned by every fallible vtable method.
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FolditPluginStatus {
    /// Success. Out-parameters are valid.
    Ok = 0,
    /// Plugin returned an op-level error. `out_err` is populated; host
    /// frees it via `free_error`.
    Err = 1,
    /// Plugin doesn't implement this method. Out-parameters are
    /// untouched. (E.g. a plugin without streaming returns this from
    /// `start_stream`.)
    Unsupported = 2,
}

/// Plugin-allocated byte buffer. Host frees via
/// [`FolditPluginVtable::free_buffer`].
#[repr(C)]
#[derive(Debug)]
pub struct FolditPluginBuffer {
    /// Pointer to the bytes. Null + `len = 0` for an empty buffer.
    pub data: *mut u8,
    /// Byte length of the valid region pointed to by `data`.
    pub len: usize,
    /// Capacity (typically `Vec` capacity); used by the plugin's
    /// `free_buffer` to reconstruct the original allocation.
    pub capacity: usize,
}

impl FolditPluginBuffer {
    /// An empty buffer with no allocation. Safe to pass to
    /// `free_buffer` (which should be a no-op on null `data`).
    #[must_use]
    pub const fn empty() -> Self {
        Self {
            data: std::ptr::null_mut(),
            len: 0,
            capacity: 0,
        }
    }
}

/// Plugin-allocated error payload. Host frees via
/// [`FolditPluginVtable::free_error`].
#[repr(C)]
#[derive(Debug)]
pub struct FolditPluginError {
    /// UTF-8 machine-readable error code (e.g. `"INVALID_INPUT"`).
    pub code: FolditPluginBuffer,
    /// UTF-8 human-readable message.
    pub message: FolditPluginBuffer,
}

impl FolditPluginError {
    /// An empty error with null `code` and `message` buffers. Safe to
    /// pass to `free_error`.
    #[must_use]
    pub const fn empty() -> Self {
        Self {
            code: FolditPluginBuffer::empty(),
            message: FolditPluginBuffer::empty(),
        }
    }
}

/// Mirror of `proto::Vec3`.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct FolditPluginVec3 {
    /// X component.
    pub x: f32,
    /// Y component.
    pub y: f32,
    /// Z component.
    pub z: f32,
}

/// Mirror of `proto::ResidueRef`.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct FolditPluginResidueRef {
    /// Entity the residue belongs to.
    pub entity_id: u64,
    /// 0-indexed residue within `entity_id`.
    pub residue_index: u32,
    /// Padding to align `entity_id` slots in arrays.
    pub padding: u32,
}

/// Mirror of `proto::DispatchContext`. Borrowed view; host owns the
/// `selection` array for the duration of the call.
#[repr(C)]
#[derive(Debug)]
pub struct FolditPluginDispatchContext {
    /// 0 = no focused entity, 1 = `focused_entity_id` is valid.
    pub has_focused_entity: u8,
    /// Padding so `focused_entity_id` is 8-aligned.
    pub padding: [u8; 7],
    /// Focused entity id when `has_focused_entity == 1`; otherwise
    /// undefined.
    pub focused_entity_id: u64,
    /// Pointer to a host-owned array of `selection_len` residue refs.
    /// May be null when `selection_len == 0`.
    pub selection: *const FolditPluginResidueRef,
    /// Number of entries in `selection`.
    pub selection_len: usize,
    /// Pointer to a host-owned array of `designable_len` residue refs: the
    /// residues the plugin may redesign (the puzzle's design mask). May be
    /// null when `designable_len == 0`.
    pub designable: *const FolditPluginResidueRef,
    /// Number of entries in `designable`.
    pub designable_len: usize,
}

/// Tag for [`FolditPluginParamValue`]. Mirrors `proto::ParamType`.
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FolditPluginParamTag {
    /// Default / unset; treated as "skip this param" by the plugin.
    Unspecified = 0,
    /// `int_value` is valid.
    Int = 1,
    /// `float_value` is valid.
    Float = 2,
    /// `bool_value` is valid.
    Bool = 3,
    /// UTF-8 string (also used for ENUM-typed params).
    String = 4,
    /// `vec3_value` is valid.
    Vec3 = 5,
}

/// Mirror of `proto::ParamValue`. Tagged struct with all variant
/// fields inlined (avoids `repr(C) union` portability concerns and
/// keeps cbindgen happy).
#[repr(C)]
#[derive(Debug)]
pub struct FolditPluginParamValue {
    /// Discriminator selecting which payload field is valid.
    pub tag: FolditPluginParamTag,
    /// Padding for 8-alignment.
    pub padding: u32,
    /// Valid when `tag == Int`.
    pub int_value: i32,
    /// Valid when `tag == Float`.
    pub float_value: f32,
    /// Valid when `tag == Bool` (0 / 1).
    pub bool_value: u8,
    /// Padding to keep the following pointer 8-aligned.
    pub padding2: [u8; 7],
    /// UTF-8 string body when `tag == String`. Borrowed; not
    /// null-terminated.
    pub string_data: *const u8,
    /// Byte length of `string_data`.
    pub string_len: usize,
    /// Valid when `tag == Vec3`.
    pub vec3_value: FolditPluginVec3,
}

/// One entry in a parameter map. Borrowed view; the host owns the
/// underlying memory for the duration of the call.
#[repr(C)]
#[derive(Debug)]
pub struct FolditPluginParamEntry {
    /// UTF-8 key; not null-terminated.
    pub key_data: *const u8,
    /// Byte length of `key_data`.
    pub key_len: usize,
    /// The parameter value.
    pub value: FolditPluginParamValue,
}

/// One puzzle asset delivered at Init: a name plus its raw bytes.
///
/// Borrowed view; the host owns the underlying memory for the duration
/// of the `init` call. The name carries the original filename (extension
/// included) so the plugin can sniff the asset's format.
#[repr(C)]
#[derive(Debug)]
pub struct FolditPluginAsset {
    /// UTF-8 asset name (original filename); not null-terminated.
    pub name_data: *const u8,
    /// Byte length of `name_data`.
    pub name_len: usize,
    /// Pointer to the asset bytes.
    pub data: *const u8,
    /// Byte length of `data`.
    pub data_len: usize,
}

/// Plugin opaque handle. The plugin allocates this in `create` and
/// frees it in `destroy`. The host treats it as opaque and threads it
/// through every other call.
pub type FolditPluginHandle = *mut c_void;

/// Function-pointer table exported by every native plugin dylib.
///
/// The plugin exports a single C symbol (`foldit_plugin_vtable`) that
/// returns `*const FolditPluginVtable`. The host calls this once at
/// load time, validates `abi_version`, and stores the pointer.
#[repr(C)]
pub struct FolditPluginVtable {
    /// MUST equal [`FOLDIT_PLUGIN_ABI_VERSION`] - host rejects the
    /// dylib otherwise.
    pub abi_version: u32,
    /// Padding to 8-align the following function pointers.
    pub padding: u32,

    // Lifecycle
    /// Construct a plugin instance from a UTF-8 JSON-encoded config
    /// dict. Returns null on failure.
    pub create:
        unsafe extern "C" fn(config_json: *const c_char, config_len: usize) -> FolditPluginHandle,

    /// Free the plugin instance. Called once; safe to assume no
    /// in-flight calls when invoked.
    pub destroy: unsafe extern "C" fn(handle: FolditPluginHandle),

    // Required protocol endpoints
    /// Returns serialized `proto::PluginRegistration` (registration is
    /// nested + only called once per session, so paying the proto cost
    /// here is fine).
    pub register: unsafe extern "C" fn(
        handle: FolditPluginHandle,
        out_buf: *mut FolditPluginBuffer,
        out_err: *mut FolditPluginError,
    ) -> FolditPluginStatus,

    /// Open a session with the initial assembly bytes. Writes the
    /// assigned session id to `*out_session` on success. `assets`
    /// carries the puzzle assets (e.g. a density map, ligand params) as
    /// borrowed name+bytes views valid only for this call. Also writes
    /// assembly bytes of the assembly the plugin settled on after any
    /// post-Init normalization (e.g. Rosetta builds a full-atom pose
    /// from the input, which may add missing atoms, hydrogens, or
    /// terminal O, changing the atom count) into `*out_initial_buf`.
    /// Plugins with no normalization step write an empty buffer; host
    /// then keeps its input assembly. Host owns the buffer afterward
    /// (released via the same `free_buffer` path as `register`/`score`).
    pub init: unsafe extern "C" fn(
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
    ) -> FolditPluginStatus,

    /// Push an Assembly update to a session. The payload is either a
    /// full assembly snapshot (`payload_kind = Full`) or a delta edit
    /// list (`payload_kind = Delta`). `from_gen` / `to_gen` are the
    /// host's broadcast generation counters; a plugin whose local gen
    /// doesn't match `from_gen` should arm a `STALE_GEN` error to
    /// return on its next dispatch so the host re-syncs.
    pub update_assembly: unsafe extern "C" fn(
        handle: FolditPluginHandle,
        session: u64,
        payload_kind: FolditPluginAssemblyPayloadKind,
        bytes: *const u8,
        bytes_len: usize,
        from_gen: u64,
        to_gen: u64,
        out_err: *mut FolditPluginError,
    ) -> FolditPluginStatus,

    /// Tear down a session and release its per-session state.
    pub drop_session: unsafe extern "C" fn(
        handle: FolditPluginHandle,
        session: u64,
        out_err: *mut FolditPluginError,
    ) -> FolditPluginStatus,

    // Optional protocol endpoints
    /// Run a one-shot op. Writes resulting assembly bytes (typically
    /// delta bytes) to `*out_assembly` on success.
    pub invoke: unsafe extern "C" fn(
        handle: FolditPluginHandle,
        session: u64,
        op_id: *const u8,
        op_id_len: usize,
        ctx: *const FolditPluginDispatchContext,
        params: *const FolditPluginParamEntry,
        params_len: usize,
        out_assembly: *mut FolditPluginBuffer,
        out_err: *mut FolditPluginError,
    ) -> FolditPluginStatus,

    /// Start a streaming op under the host-assigned `request_id`. The
    /// plugin keys its stream state on that id; subsequent poll / update
    /// / cancel calls thread the same id through.
    pub start_stream: unsafe extern "C" fn(
        handle: FolditPluginHandle,
        session: u64,
        op_id: *const u8,
        op_id_len: usize,
        ctx: *const FolditPluginDispatchContext,
        params: *const FolditPluginParamEntry,
        params_len: usize,
        request_id: u64,
        out_err: *mut FolditPluginError,
    ) -> FolditPluginStatus,

    /// Returns serialized `proto::PollStreamResponse`; the host
    /// decodes the variant. Centralizing the variant set in proto
    /// keeps the C ABI surface smaller; poll_stream is the only place
    /// where the variant tagging matters.
    pub poll_stream: unsafe extern "C" fn(
        handle: FolditPluginHandle,
        request_id: u64,
        out_buf: *mut FolditPluginBuffer,
        out_err: *mut FolditPluginError,
    ) -> FolditPluginStatus,

    /// Apply a live parameter update to an active stream. Plugins may
    /// coalesce or defer updates until the next poll boundary.
    pub update_stream: unsafe extern "C" fn(
        handle: FolditPluginHandle,
        request_id: u64,
        params: *const FolditPluginParamEntry,
        params_len: usize,
        out_err: *mut FolditPluginError,
    ) -> FolditPluginStatus,

    /// Cancel an active stream. The plugin must release stream state
    /// before the next poll returns `Final` or `Error`.
    pub cancel_stream: unsafe extern "C" fn(
        handle: FolditPluginHandle,
        request_id: u64,
        out_err: *mut FolditPluginError,
    ) -> FolditPluginStatus,

    /// Run a read-only query (no assembly mutation). When `assembly` is
    /// non-null (`assembly_len > 0`) it names a specific composition to
    /// read/score instead of the session: committed heads or a
    /// checkpoint; null/0 operates on the session / its in-flight
    /// snapshot. Result bytes are op-defined (e.g. a serialized
    /// `proto::ScoreReport` for the `"score"` query); the caller parses
    /// them against the query contract.
    pub query: unsafe extern "C" fn(
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
    ) -> FolditPluginStatus,

    // Memory cleanup
    /// Free a plugin-allocated buffer. No-op when `data` is null.
    pub free_buffer: unsafe extern "C" fn(buf: *mut FolditPluginBuffer),
    /// Free both inner buffers of a plugin-allocated error struct.
    pub free_error: unsafe extern "C" fn(err: *mut FolditPluginError),
}

/// Symbol name the host looks up via `dlsym`. Returns
/// `*const FolditPluginVtable`.
pub const VTABLE_SYMBOL: &[u8] = b"foldit_plugin_vtable\0";

/// Type signature of the `foldit_plugin_vtable` entry symbol.
pub type FolditPluginVtableFn = unsafe extern "C" fn() -> *const FolditPluginVtable;
