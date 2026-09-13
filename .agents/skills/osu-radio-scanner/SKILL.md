---
name: osu-radio-scanner
description: Modify osu-radio discovery, source import mapping, or the C# Realm helper. Use for radio-core import contracts and radio-scanner behavior; not for persistence workflows, database API design, or GUI-only changes.
---

# osu-radio scanner changes

Read [shared guidance](../../../AGENTS.md) and the [scanner guide](../../../docs/agent/scanner.md). Use [development](../../../docs/agent/development.md) for scoped commands and Context7 when upstream API advice is needed.

1. Trace the affected function and all callers, including CLI entry points and backend consumers. Follow discovery options into the collector/walk, or helper output through the Rust parser into domain types, before choosing the edit location.
2. Preserve read-only source access and referenced audio paths. Keep discovery/import reusable in crates; do not choose persistence policy or copy audio in the scanner. Inspect current cancellation and failure paths rather than assuming receiver drop immediately stops all work.
3. For protocol changes, compare the producer, wire parser and domain shape together; synchronize each affected layer and its fixtures. Keep data on stdout and diagnostics on stderr. Check helper exit, malformed output and process cleanup implications.
4. Select relevant discovery, mapping and helper-process tests from the guide. Use explicit temporary roots for discovery tests and explicit markers for authorized source reads; never use broad OS discovery as verification. Add a focused regression check for changed behavior. Distinguish Rust fake-helper checks from a real Realm/.NET check and state host-specific test limits.
5. Update affected guide facts when behavior or contracts change. Keep technical facts in the guide and task procedure here; database contracts remain deferred.
