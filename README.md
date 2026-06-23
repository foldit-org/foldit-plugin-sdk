# foldit-plugin-sdk

The Foldit plugin SDK. A Rust core owns `plugin.proto`, the protocol types,
and the `Plugin` trait. It exposes a cbindgen C-ABI (consumed by the rosetta
C++ bridge) and pyo3 Python bindings, plus a `python/` source layer for
Python-only plugin-author utilities.

This is an early skeleton; the protocol types and `Plugin` trait are not yet
implemented.
