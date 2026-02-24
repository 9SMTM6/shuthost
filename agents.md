The role of this document is to provide guidance for common mistakes and confusion points for agents in this project. If you encounter something in this project that you find confusing, please alert the developer and indicate a short fix in this document.

- **Playwright**: `just playwright --reporter=line` (line reporter avoids blocking output).
- **Rust tests**: `just cargo_tests <pattern>` to run tests matching a workspace package pattern.
