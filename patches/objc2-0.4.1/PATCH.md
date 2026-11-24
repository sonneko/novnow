# Local patch to objc2 0.4.1

This directory is the `objc2` 0.4.1 crate source from the repository's pinned Cargo vendor snapshot. It has one compatibility-only change: remove the forwarding overrides of `Iterator::nth` and `DoubleEndedIterator::nth_back` from `src/declare/ivar_forwarding_impls.rs`. The standard trait defaults provide the same iteration behavior.

The upstream issue documents the Rust 1.100 E0277 regression and this workaround: https://github.com/madsmtm/objc2/issues/898

`winit 0.29` / `glutin 0.31` / `icrate 0.0.4` require `objc2 0.4.1`, so a normal dependency upgrade cannot resolve this until a compatible upstream patch release exists. The `[patch.crates-io]` override is deliberately limited to this affected old version; other objc2 versions remain unchanged. Remove this directory and override once upstream releases a fixed compatible version or the UI stack upgrades past winit 0.29.
