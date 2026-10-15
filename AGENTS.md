# Force

Force is a local-first desktop focus timer built with Rust and GPUI. Keep all user data on the device; the app needs no account, server, or network client.

- `crates/force-core`: SQLite persistence, timer intervals, goals, history, CSV export and statistics. Keep it independent of GPUI and OS APIs.
- `crates/force-app`: native GPUI interface and platform integration. Support macOS, Windows and Linux without duplicating business logic.
- Force is a floating utility: a compact timer strip by default, with a same-width panel for task setup, records and settings. Do not turn it into a dashboard or introduce a large app shell.
- Use black, white and neutral gray, with semantic colors only for errors. Keep controls compact and keyboard accessible.
- Active sessions survive restart. Use absolute timestamps, not tick counts. Pauses exclude time; computer sleep does not implicitly pause. Sum parallel task investment separately from merged focus time.
- Write changes transactionally. Preserve user data on errors. Never recover a damaged database by overwriting it with a blank one.
- Test with a dedicated temporary data directory. Do not read or modify a real user's records during acceptance.
- macOS previews must be signed with an existing private-key-backed identity. Use a separate Force Dev bundle and app data directory. Do not replace a formal installation.
- Run `scripts/check.sh` before delivery. UI acceptance needs the actual native app; compilation alone is not visual or cross-platform acceptance.
- Name branches `feat/...`, `fix/...` or `docs/...`. A substantial rewrite needs explicit approval before merging main. Do not publish a release without a release instruction.
- Do not commit credentials, private data, local paths, installers or build outputs. Keep documentation concise and about current behavior.
- Remove task-created temporary artifacts after verification. Preserve source, active app artifacts, useful build caches and concise verification evidence.
