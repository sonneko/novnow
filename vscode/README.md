# novnow for VSCode

This extension connects VSCode to the official `novnow` language server. It does not implement diagnostics, writing, or export logic itself.

When `novnow` is not installed, the extension can download the GUI-less archive for the current OS and CPU from the matching official GitHub release. Every archive is checked against the release's `SHA256SUMS` before use. Set `novnow.autoDownload` to `false` to disable this behavior.

Commands: **Check Project**, **Export Text**, **New Project**, and **Restart Language Server**.

The extension identifier is `sonneko.novnow`. The extension and novnow share one version number.

## Installation

Download [novnow-0.1.0.vsix](https://github.com/sonneko/novnow/releases/download/v0.1.0/novnow-0.1.0.vsix), then use VSCode's Extensions → … → Install from VSIX. Version v0.1.0 is published. Marketplace publication is deferred and will start no later than product v1.
