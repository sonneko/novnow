# Known issues

This list covers unresolved issues in published releases. A source-code fix does not remove an issue from this list; it stays until a release containing the fix is published.

## KI-001: GitHub Action fails on macOS and Windows

- **Affected version**: v0.1.0, including the `v0` tag pointing to that release.
- **Environment**: macOS and Windows GitHub Actions runners. Linux runners are unaffected by this issue.
- **Symptoms**: Empty arguments stop execution on macOS. On Windows, a valid archive may be rejected by checksum verification or fail to extract.
- **Workaround**: Run the action with `runs-on: ubuntu-latest`. This issue does not indicate a defect in the desktop or Python distributions.
- **Status**: The source fix passed checks on all three operating systems. Given the limited impact, it will ship with the next release rather than a standalone patch. The fixed version and publication date have not been assigned.

## KI-002: The view shifts on iOS PWA with a physical keyboard

- **Affected version**: v0.1.0
- **Environment**: The PWA on iOS Safari with a physical keyboard.
- **Symptoms**: The visible area may shift each time you move between sections, making the text you are writing hard to see.
- **Workaround**: The shift never affects your text or saved data. Touching the screen restores the view.
- **Status**: A fix that avoids repeated refocusing of the hidden input and restores the scroll position after focusing has landed in the development build. This entry will be removed once the fix is verified on a real device and released.
