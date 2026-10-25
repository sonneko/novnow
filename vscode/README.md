# novnow for VSCode

This extension connects VSCode to the official `novnow` language server. It does not implement diagnostics, writing, or export logic itself.

When `novnow` is not installed, the extension can download the GUI-less archive for the current OS and CPU from the matching official GitHub release. Every archive is checked against the release's `SHA256SUMS` before use. Set `novnow.autoDownload` to `false` to disable this behavior.

Commands: **Check Project**, **Export Text**, **New Project**, and **Restart Language Server**.

The extension identifier is `sonneko.novnow`. The extension and novnow share one version number.

## Installation

Download `novnow-<version>.vsix` from the official GitHub Release, then use VSCode's Extensions → … → Install from VSIX. The first numbered release has not been published yet. Marketplace publication is deferred and will start no later than product v1.
