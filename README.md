# foldit-plugin-sdk

The contract between the Foldit host and the out-of-process molecular backends
it drives. Foldit compiles no scientific engine into the client; Rosetta,
structure prediction, design, and crystallography all run as plugins hosted by
`foldit-runner`, and every one of them speaks the protocol this crate owns.

The SDK owns three things and exposes them to three kinds of consumer:

- **`plugin.proto`** and the generated message types — the wire schema.
- **The `Plugin` trait** — the worker-side interface a plugin implements.
- **The `PluginError` type** and the small mirror types (`DispatchContext`,
  `ResidueRef`, `ParamValue`, `PollOutcome`) that cross the FFI boundary.

## Who consumes it, and how

The crate builds as `rlib`, `cdylib`, and `staticlib` at once so all three
plugin worlds link the same protocol definitions:

| Consumer | Surface | Link form |
| --- | --- | --- |
| The host (`foldit-runner`) | `Plugin` trait, proto types, orchestrator glue | `rlib` |
| Native plugins (rosetta, design, xtal) | the C ABI vtable (`src/abi.rs`) | `staticlib` + generated `foldit_plugin_sdk.h` |
| Python plugins (foundry, simplefold) | the pyo3 extension module + the `python/` package | `cdylib` (via maturin) |

There is no IPC or proto serialization on the hot path. A native plugin is a
shared library the host `dlopen`s and calls through function pointers; a Python
plugin runs in `foldit-python-host`, which crosses the pyo3 boundary in-process.

## The protocol

`proto/plugin.proto` is compiled at build time by `build.rs` using `protox`
(a pure-Rust protobuf compiler, so no system `protoc` is required) into
`crate::proto::plugin`. The same build step runs `cbindgen` to emit the C
header consumed by the rosetta C++ bridge. The message set covers plugin
registration and op/query catalogs (`PluginRegistration`, `PluginOp`,
`PluginQuery`, `ParamSpec`), session lifecycle (`InitRequest`/`InitResponse`,
`UpdateAssemblyRequest`, `DropRequest`), and the scientific result payloads
plugins return (`ScoreReport`, `ResidueTermScores`, `BonusContribution`,
`ClashReport`, `VoidField`, `ExposedHydrophobicReport`, and the puzzle
constraint types).

Assembly geometry is carried as **molex wire bytes**, not protobuf: an
`UpdateAssemblyRequest` is either a `Full` snapshot or a `Delta` edit list
(decode with molex's `serialize_edits` / `deserialize_edits`). The SDK depends
on `molex` only for `EntityId`, the identifier that crosses every request.

## The `Plugin` trait

Both native and Python plugins implement the same worker-side interface
(`src/plugin.rs`). Its methods form the session lifecycle:

- `register()` — return the op + query catalog (`PluginRegistration`) so the
  host knows what buttons and readouts this plugin offers.
- `init(assembly, assets, params)` — open a session against a starting
  structure, returning the plugin's chosen session id plus its normalized
  assembly.
- `update_assembly(session, payload, from_gen, to_gen)` — push a `Full` or
  `Delta` structure update into an existing session.
- `invoke(session, op, ctx, params)` — a single-shot mutating op (Wiggle,
  Shake, a design move); returns the plugin's post-op assembly bytes.
- `query(session, query, ctx, params, assembly)` — a read-only readout
  (a score breakdown, a clash report) that never mutates the session.
- `start_stream` / `poll_stream` / `update_stream` / `cancel_stream` — a
  long-running op the host polls under a host-assigned `request_id` (a
  minimization that streams intermediate frames, a prediction that reports
  progress).
- `drop_session(session)` — tear the session down; idempotent.

`invoke`, `query`, and the streaming methods have default impls returning
`PluginError::Unsupported`, so a plugin implements only the surface it offers.

The trait takes `&self` throughout: a plugin mutates its own state through
interior mutability (Python's GIL, the C++ bridge's thread-locked FFI), and the
host serializes all calls into a single instance. Plugins must be `Send` —
each call may land on a different worker thread, but never two at once.

## Writing a native plugin (C ABI)

A native plugin is a shared library named `lib<id>.{dylib,so,dll}` that exports
one symbol, `foldit_plugin_vtable`, returning a pointer to a
`FolditPluginVtable` (`src/abi.rs`). The host reads `abi_version` on load and
rejects a mismatch with a clear error, then dispatches through the vtable's
function pointers. Ownership rules, enforced by convention across the ABI:

- Buffers the plugin returns (`register`, `score`, `invoke`, `query`,
  `poll_stream`) are plugin-allocated; the host frees them via `free_buffer`.
- An error struct filled on a `FOLDIT_PLUGIN_ERR` return is plugin-allocated;
  the host frees it via `free_error`.
- Buffers the host passes in (assembly bytes, params, session context) must
  **not** be retained past the call — copy anything you need to keep.

Rosetta is the reference native plugin: its C++ bridge links the `staticlib`
and exports `foldit_plugin_vtable` from the existing `librosetta_interactive`
dylib, so there is no separate plugin binary.

## Writing a Python plugin

Python plugins subclass `PluginInterface`
(`python/foldit_plugin_sdk/plugin_interface.py`), an `ABC` whose abstract
methods mirror the `Plugin` trait (`register`, `init`, `update_assembly`,
`drop`, and the optional op/query/stream methods). The `python/` package also
ships the plugin-author utilities the ML plugins rely on: weight and checkpoint
resolution, device selection, quantization, caching, and multiprocessing
helpers.

The receive- and return-direction types cross the boundary as pyo3 classes
registered by the `foldit_plugin_sdk` extension module (`src/lib.rs`):
`DispatchContext` and `ResidueRef` are read on the way in; `PollOutcome` and
the `ScoreReport` family are constructed on the way out.

## Building and features

```bash
cargo build                 # rlib + staticlib + generated C header
cargo test                  # do NOT combine with the extension-module feature
```

Cargo features:

- `python` — enable the pyo3 bindings (`dep:pyo3`).
- `extension-module` — build the importable Python extension via maturin. It
  tells the linker not to link libpython, so it **cannot** be active during
  `cargo test` (a standalone test binary would fail to resolve `Py*` symbols).

`pyo3` is pinned to `0.29` to match `foldit-python-host`: the SDK's pyclasses
cross that crate's in-process boundary, so the two pyo3 versions must agree.
`cbindgen` is pinned to `=0.29.2` to keep the generated header byte-for-byte
identical to the runner's committed copy.

## In the workspace

This crate lives in its own repository and is pulled into the Foldit tree as
the `crates/foldit-plugin-sdk` submodule. Root crates depend on it from
crates.io by default; a commented `[patch.crates-io]` entry in the root
`Cargo.toml` redirects to the local checkout for SDK development.
