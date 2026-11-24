# Local patch to objc2 0.3.0-beta.3.patch-leaks.3

This is the source for the exact objc2 prerelease pinned by `accesskit_macos 0.10.1`, which is pulled into the macOS GUI through `eframe 0.27.2 -> egui-winit 0.27.2 -> accesskit_winit 0.16.1`. Its `DoubleEndedIterator::nth_back` forwarding override fails with E0277 on the Rust 1.100 macOS GUI build. The only source change removes that override and uses the standard trait default, preserving iteration behavior.

The upstream regression is documented at https://github.com/madsmtm/objc2/issues/898. This patch is separate from `objc2 0.4.1`, also used by the same GUI stack, and is required because `accesskit_macos` constrains the prerelease with `<0.3.0-beta.4`.

`[patch.crates-io]` uses a renamed key (`objc2-beta`) to override the pinned prerelease alongside the 0.4.1 patch. Remove both compatibility patches when the UI stack has upgraded past the affected objc2 versions.
