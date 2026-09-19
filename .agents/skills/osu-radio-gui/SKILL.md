---
name: osu-radio-gui
description: Modify osu-radio GUI layouts, interactions, styles, or assets in apps/osu-radio-gui-vizia or apps/osu-radio-qt. Use for visual and interactive GUI changes; not for scanner work or backend API design.
---

# osu-radio GUI changes

Read [shared guidance](../../../AGENTS.md) and the [frontend guide](../../../docs/agent/frontend.md) before editing. Use [development](../../../docs/agent/development.md) for verification and Context7 when upstream API advice is needed.

1. Trace the affected view from the shell through its existing components, state and event handlers. Inspect its stylesheet registration and asset references. Establish whether the requested interaction already works or is a placeholder.
2. Change the existing owner: view construction/events in the GUI, styling in the owning area sheet, reusable frontend behavior in the toolkit-free client. Reuse existing components and view models. Do not implement backend access or osu! file reads inside views, or silently expand a visual task into API/persistence work.
3. For interactive changes, inspect hit targets, clickable children, decorative overlaps and bubbling. Preserve the empty-textbox placeholder workaround. Keep signals on the UI thread and route background results through existing events.
4. Verify the affected stylesheet is included; a new leaf can use an existing area sheet. Select the relevant checks from the development matrix, including the release GUI check for stylesheet paths. Exercise affected interactions, empty search fields and window behavior when a GUI session is available. A successful build alone does not prove hit testing or interaction behavior; report unperformed manual checks.
5. Update only affected guide facts if behavior, ownership or known limitations changed. Keep procedures here and technical detail in the linked guide.


For Qt changes, trace the QML root through shared `qml/components`, the typed
Rust adapter and the shared client `AppController`. Production launch is live;
`--component-gallery` remains offline with the opt-in client `mock` actions.
Keep toolkit/resource paths and decoded artwork caches in the Qt app; QML owns
focus and popup/window presentation. Preserve Vizia through the same controller.
Use the Qt checks in the development guide: client feature off/on tests, Qt
integration probes/build, generated-import `qmllint`, scoped Clippy and formatting.
Offscreen smoke tests are permitted; leave desktop input/visual checks to the user
and report Windows separately. Do not apply Vizia-specific CSS/textbox workarounds
to native QML controls.
