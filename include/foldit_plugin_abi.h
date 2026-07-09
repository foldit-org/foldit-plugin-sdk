#pragma once

#include <stdarg.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdlib.h>

/**
 * Current ABI version. Bump on any layout change.
 */
#define FOLDIT_PLUGIN_ABI_VERSION 7

/**
 * Payload tag for [`FolditPluginVtable::update_assembly`].
 *
 * `Full` carries fresh assembly bytes; discard prior state, decode and
 * install. `Delta` carries delta bytes; decode via molex's
 * `molex_delta_to_edits` and apply incrementally; preserves derived
 * plugin state across mutations. Plugins that don't track incremental
 * state may treat `Delta` the same as `Full` by reconstituting the
 * assembly from the decoded edits.
 */
enum FolditPluginAssemblyPayloadKind
#ifdef __cplusplus
  : uint8_t
#endif // __cplusplus
 {
  /**
   * Payload is a fresh assembly snapshot.
   */
  FOLDIT_PLUGIN_ASSEMBLY_PAYLOAD_KIND_FULL = 0,
  /**
   * Payload is a delta edit list.
   */
  FOLDIT_PLUGIN_ASSEMBLY_PAYLOAD_KIND_DELTA = 1,
};
#ifndef __cplusplus
typedef uint8_t FolditPluginAssemblyPayloadKind;
#endif // __cplusplus

/**
 * Status code returned by every fallible vtable method.
 */
enum FolditPluginStatus
#ifdef __cplusplus
  : uint32_t
#endif // __cplusplus
 {
  /**
   * Success. Out-parameters are valid.
   */
  FOLDIT_PLUGIN_STATUS_OK = 0,
  /**
   * Plugin returned an op-level error. `out_err` is populated; host
   * frees it via `free_error`.
   */
  FOLDIT_PLUGIN_STATUS_ERR = 1,
  /**
   * Plugin doesn't implement this method. Out-parameters are
   * untouched. (E.g. a plugin without streaming returns this from
   * `start_stream`.)
   */
  FOLDIT_PLUGIN_STATUS_UNSUPPORTED = 2,
};
#ifndef __cplusplus
typedef uint32_t FolditPluginStatus;
#endif // __cplusplus

/**
 * Tag for [`FolditPluginParamValue`]. Mirrors `proto::ParamType`.
 */
enum FolditPluginParamTag
#ifdef __cplusplus
  : uint32_t
#endif // __cplusplus
 {
  /**
   * Default / unset; treated as "skip this param" by the plugin.
   */
  FOLDIT_PLUGIN_PARAM_TAG_UNSPECIFIED = 0,
  /**
   * `int_value` is valid.
   */
  FOLDIT_PLUGIN_PARAM_TAG_INT = 1,
  /**
   * `float_value` is valid.
   */
  FOLDIT_PLUGIN_PARAM_TAG_FLOAT = 2,
  /**
   * `bool_value` is valid.
   */
  FOLDIT_PLUGIN_PARAM_TAG_BOOL = 3,
  /**
   * UTF-8 string (also used for ENUM-typed params).
   */
  FOLDIT_PLUGIN_PARAM_TAG_STRING = 4,
  /**
   * `vec3_value` is valid.
   */
  FOLDIT_PLUGIN_PARAM_TAG_VEC3 = 5,
};
#ifndef __cplusplus
typedef uint32_t FolditPluginParamTag;
#endif // __cplusplus

/**
 * Plugin-allocated byte buffer. Host frees via
 * [`FolditPluginVtable::free_buffer`].
 */
typedef struct FolditPluginBuffer {
  /**
   * Pointer to the bytes. Null + `len = 0` for an empty buffer.
   */
  uint8_t *data;
  /**
   * Byte length of the valid region pointed to by `data`.
   */
  size_t len;
  /**
   * Capacity (typically `Vec` capacity); used by the plugin's
   * `free_buffer` to reconstruct the original allocation.
   */
  size_t capacity;
} FolditPluginBuffer;

/**
 * Plugin-allocated error payload. Host frees via
 * [`FolditPluginVtable::free_error`].
 */
typedef struct FolditPluginError {
  /**
   * UTF-8 machine-readable error code (e.g. `"INVALID_INPUT"`).
   */
  struct FolditPluginBuffer code;
  /**
   * UTF-8 human-readable message.
   */
  struct FolditPluginBuffer message;
} FolditPluginError;

/**
 * Mirror of `proto::Vec3`.
 */
typedef struct FolditPluginVec3 {
  /**
   * X component.
   */
  float x;
  /**
   * Y component.
   */
  float y;
  /**
   * Z component.
   */
  float z;
} FolditPluginVec3;

/**
 * Mirror of `proto::ResidueRef`.
 */
typedef struct FolditPluginResidueRef {
  /**
   * Entity the residue belongs to.
   */
  uint64_t entity_id;
  /**
   * 0-indexed residue within `entity_id`.
   */
  uint32_t residue_index;
  /**
   * Padding to align `entity_id` slots in arrays.
   */
  uint32_t padding;
} FolditPluginResidueRef;

/**
 * Mirror of `proto::DispatchContext`. Borrowed view; host owns the
 * `selection` array for the duration of the call.
 */
typedef struct FolditPluginDispatchContext {
  /**
   * 0 = no focused entity, 1 = `focused_entity_id` is valid.
   */
  uint8_t has_focused_entity;
  /**
   * Padding so `focused_entity_id` is 8-aligned.
   */
  uint8_t padding[7];
  /**
   * Focused entity id when `has_focused_entity == 1`; otherwise
   * undefined.
   */
  uint64_t focused_entity_id;
  /**
   * Pointer to a host-owned array of `selection_len` residue refs.
   * May be null when `selection_len == 0`.
   */
  const struct FolditPluginResidueRef *selection;
  /**
   * Number of entries in `selection`.
   */
  size_t selection_len;
  /**
   * Pointer to a host-owned array of `designable_len` residue refs: the
   * residues the plugin may redesign (the puzzle's design mask). May be
   * null when `designable_len == 0`.
   */
  const struct FolditPluginResidueRef *designable;
  /**
   * Number of entries in `designable`.
   */
  size_t designable_len;
} FolditPluginDispatchContext;

/**
 * Mirror of `proto::ParamValue`. Tagged struct with all variant
 * fields inlined (avoids `repr(C) union` portability concerns and
 * keeps cbindgen happy).
 */
typedef struct FolditPluginParamValue {
  /**
   * Discriminator selecting which payload field is valid.
   */
  FolditPluginParamTag tag;
  /**
   * Padding for 8-alignment.
   */
  uint32_t padding;
  /**
   * Valid when `tag == Int`.
   */
  int32_t int_value;
  /**
   * Valid when `tag == Float`.
   */
  float float_value;
  /**
   * Valid when `tag == Bool` (0 / 1).
   */
  uint8_t bool_value;
  /**
   * Padding to keep the following pointer 8-aligned.
   */
  uint8_t padding2[7];
  /**
   * UTF-8 string body when `tag == String`. Borrowed; not
   * null-terminated.
   */
  const uint8_t *string_data;
  /**
   * Byte length of `string_data`.
   */
  size_t string_len;
  /**
   * Valid when `tag == Vec3`.
   */
  struct FolditPluginVec3 vec3_value;
} FolditPluginParamValue;

/**
 * One entry in a parameter map. Borrowed view; the host owns the
 * underlying memory for the duration of the call.
 */
typedef struct FolditPluginParamEntry {
  /**
   * UTF-8 key; not null-terminated.
   */
  const uint8_t *key_data;
  /**
   * Byte length of `key_data`.
   */
  size_t key_len;
  /**
   * The parameter value.
   */
  struct FolditPluginParamValue value;
} FolditPluginParamEntry;

/**
 * Plugin opaque handle. The plugin allocates this in `create` and
 * frees it in `destroy`. The host treats it as opaque and threads it
 * through every other call.
 */
typedef void *FolditPluginHandle;

/**
 * One puzzle asset delivered at Init: a name plus its raw bytes.
 *
 * Borrowed view; the host owns the underlying memory for the duration
 * of the `init` call. The name carries the original filename (extension
 * included) so the plugin can sniff the asset's format.
 */
typedef struct FolditPluginAsset {
  /**
   * UTF-8 asset name (original filename); not null-terminated.
   */
  const uint8_t *name_data;
  /**
   * Byte length of `name_data`.
   */
  size_t name_len;
  /**
   * Pointer to the asset bytes.
   */
  const uint8_t *data;
  /**
   * Byte length of `data`.
   */
  size_t data_len;
} FolditPluginAsset;

/**
 * Function-pointer table exported by every native plugin dylib.
 *
 * The plugin exports a single C symbol (`foldit_plugin_vtable`) that
 * returns `*const FolditPluginVtable`. The host calls this once at
 * load time, validates `abi_version`, and stores the pointer.
 */
typedef struct FolditPluginVtable {
  /**
   * MUST equal [`FOLDIT_PLUGIN_ABI_VERSION`] - host rejects the
   * dylib otherwise.
   */
  uint32_t abi_version;
  /**
   * Padding to 8-align the following function pointers.
   */
  uint32_t padding;
  /**
   * Construct a plugin instance from a UTF-8 JSON-encoded config
   * dict. Returns null on failure.
   */
  FolditPluginHandle (*create)(const char *config_json, size_t config_len);
  /**
   * Free the plugin instance. Called once; safe to assume no
   * in-flight calls when invoked.
   */
  void (*destroy)(FolditPluginHandle handle);
  /**
   * Returns serialized `proto::PluginRegistration` (registration is
   * nested + only called once per session, so paying the proto cost
   * here is fine).
   */
  FolditPluginStatus (*register_)(FolditPluginHandle handle,
                                  struct FolditPluginBuffer *out_buf,
                                  struct FolditPluginError *out_err);
  /**
   * Open a session with the initial assembly bytes. Writes the
   * assigned session id to `*out_session` on success. `assets`
   * carries the puzzle assets (e.g. a density map, ligand params) as
   * borrowed name+bytes views valid only for this call. Also writes
   * assembly bytes of the assembly the plugin settled on after any
   * post-Init normalization (e.g. Rosetta builds a full-atom pose
   * from the input, which may add missing atoms, hydrogens, or
   * terminal O, changing the atom count) into `*out_initial_buf`.
   * Plugins with no normalization step write an empty buffer; host
   * then keeps its input assembly. Host owns the buffer afterward
   * (released via the same `free_buffer` path as `register`/`score`).
   */
  FolditPluginStatus (*init)(FolditPluginHandle handle,
                             const uint8_t *assembly,
                             size_t assembly_len,
                             const struct FolditPluginAsset *assets,
                             size_t assets_len,
                             const struct FolditPluginParamEntry *params,
                             size_t params_len,
                             uint64_t *out_session,
                             struct FolditPluginBuffer *out_initial_buf,
                             struct FolditPluginError *out_err);
  /**
   * Push an Assembly update to a session. The payload is either a
   * full assembly snapshot (`payload_kind = Full`) or a delta edit
   * list (`payload_kind = Delta`). `from_gen` / `to_gen` are the
   * host's broadcast generation counters; a plugin whose local gen
   * doesn't match `from_gen` should arm a `STALE_GEN` error to
   * return on its next dispatch so the host re-syncs.
   */
  FolditPluginStatus (*update_assembly)(FolditPluginHandle handle,
                                        uint64_t session,
                                        FolditPluginAssemblyPayloadKind payload_kind,
                                        const uint8_t *bytes,
                                        size_t bytes_len,
                                        uint64_t from_gen,
                                        uint64_t to_gen,
                                        struct FolditPluginError *out_err);
  /**
   * Tear down a session and release its per-session state.
   */
  FolditPluginStatus (*drop_session)(FolditPluginHandle handle,
                                     uint64_t session,
                                     struct FolditPluginError *out_err);
  /**
   * Run a one-shot op. Writes resulting assembly bytes (typically
   * delta bytes) to `*out_assembly` on success.
   */
  FolditPluginStatus (*invoke)(FolditPluginHandle handle,
                               uint64_t session,
                               const uint8_t *op_id,
                               size_t op_id_len,
                               const struct FolditPluginDispatchContext *ctx,
                               const struct FolditPluginParamEntry *params,
                               size_t params_len,
                               struct FolditPluginBuffer *out_assembly,
                               struct FolditPluginError *out_err);
  /**
   * Start a streaming op under the host-assigned `request_id`. The
   * plugin keys its stream state on that id; subsequent poll / update
   * / cancel calls thread the same id through.
   */
  FolditPluginStatus (*start_stream)(FolditPluginHandle handle,
                                     uint64_t session,
                                     const uint8_t *op_id,
                                     size_t op_id_len,
                                     const struct FolditPluginDispatchContext *ctx,
                                     const struct FolditPluginParamEntry *params,
                                     size_t params_len,
                                     uint64_t request_id,
                                     struct FolditPluginError *out_err);
  /**
   * Returns serialized `proto::PollStreamResponse`; the host
   * decodes the variant. Centralizing the variant set in proto
   * keeps the C ABI surface smaller; poll_stream is the only place
   * where the variant tagging matters.
   */
  FolditPluginStatus (*poll_stream)(FolditPluginHandle handle,
                                    uint64_t request_id,
                                    struct FolditPluginBuffer *out_buf,
                                    struct FolditPluginError *out_err);
  /**
   * Apply a live parameter update to an active stream. Plugins may
   * coalesce or defer updates until the next poll boundary.
   */
  FolditPluginStatus (*update_stream)(FolditPluginHandle handle,
                                      uint64_t request_id,
                                      const struct FolditPluginParamEntry *params,
                                      size_t params_len,
                                      struct FolditPluginError *out_err);
  /**
   * Cancel an active stream. The plugin must release stream state
   * before the next poll returns `Final` or `Error`.
   */
  FolditPluginStatus (*cancel_stream)(FolditPluginHandle handle,
                                      uint64_t request_id,
                                      struct FolditPluginError *out_err);
  /**
   * Run a read-only query (no assembly mutation). When `assembly` is
   * non-null (`assembly_len > 0`) it names a specific composition to
   * read/score instead of the session: committed heads or a
   * checkpoint; null/0 operates on the session / its in-flight
   * snapshot. Result bytes are op-defined (e.g. a serialized
   * `proto::ScoreReport` for the `"score"` query); the caller parses
   * them against the query contract.
   */
  FolditPluginStatus (*query)(FolditPluginHandle handle,
                              uint64_t session,
                              const uint8_t *query_id,
                              size_t query_id_len,
                              const struct FolditPluginDispatchContext *ctx,
                              const struct FolditPluginParamEntry *params,
                              size_t params_len,
                              const uint8_t *assembly,
                              size_t assembly_len,
                              struct FolditPluginBuffer *out_data,
                              struct FolditPluginError *out_err);
  /**
   * Free a plugin-allocated buffer. No-op when `data` is null.
   */
  void (*free_buffer)(struct FolditPluginBuffer *buf);
  /**
   * Free both inner buffers of a plugin-allocated error struct.
   */
  void (*free_error)(struct FolditPluginError *err);
} FolditPluginVtable;

/**
 * Type signature of the `foldit_plugin_vtable` entry symbol.
 */
typedef const struct FolditPluginVtable *(*FolditPluginVtableFn)(void);
