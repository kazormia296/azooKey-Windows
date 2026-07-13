# Windows TSF Composition Behavior Contract plan

Windows follows `composition-behavior-v1` but keeps named-pipe gRPC, TSF edit
sessions, and the candidate-window process as native transports/UI.

All nine shared scenarios are copied under `tests/composition-behavior-v1`,
SHA-256 pinned, and verified by `cargo test -p shared`. `gap-matrix.json`
records a non-skippable status, concrete reason, and target migration slice for
every scenario. The test also pins the exact nine scenario IDs, filename/ID
agreement, action presence, status vocabulary, and snapshot shape so the lock
and gap matrix cannot be weakened together by silently deleting a trace. This
is a baseline audit, not a claim that the current TSF implementation already
conforms.

## Current baseline

`crates/client/src/engine/composition.rs` still owns preview, suffix, raw input,
candidate selection, and corresponding counts while the server owns another
composition representation. This is the known split-ownership gap; the current
consumer and installer remain usable while the migration is staged.

## Migration slices

1. Add a semantic action/snapshot adapter and fixture IDs without changing TSF
   range application.
2. Add `revision`, candidate generation, request IDs, and effect IDs to the
   Windows IPC DTOs; reject stale responses before opening an edit session.
3. Move composition authority to the server-side state machine. Rust keeps
   only the latest snapshot and TSF range cache.
4. Convert semantic caret boundaries to UTF-16 ranges in one adapter and add
   COM reentrancy, x64/x86, secure-input, and candidate-window tests.
5. Remove the legacy `AppendText`/`RemoveText` ownership path only after the
   shared P0 scenarios are green.

This plan intentionally does not copy the Linux protobuf or Fcitx code.

## Exit criteria

The overhaul is complete only when each gap-matrix entry moves to `conforming`,
the Rust adapter executes every shared action/snapshot trace, x64 and x86 TSF
edit-session tests pass, and the procedural `AppendText`/`RemoveText`/
`ShrinkText` ownership path is removed. Until then, Windows remains a separate
follow-up EPIC and does not block the Linux release gate.
