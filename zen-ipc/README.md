# zen-ipc

Types and helpers for interfacing with the [zen](https://github.com/cl5rr/zen) Wayland compositor.

## Backwards compatibility

This crate follows the zen version.
It is **not** API-stable in terms of the Rust semver.
In particular, expect new struct fields and enum variants to be added in patch version bumps.

Use an exact version requirement to avoid breaking changes:

```toml
[dependencies]
zen-ipc = "=26.4.0"
```
