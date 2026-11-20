# ThermalPrint workflow contract

- Settings and named stock profiles use `src/settings.rs`. Changing printers saves the current profile. A setup assistant guides the existing Print Center controls.
- `src/ui/notices.rs` owns persistent errors and action feedback. Controls expose text labels and explanations for disabled actions.
- `Designer::request_close` owns unsaved-label protection. Explicit Quit uses that protection. Background close keeps the document alive.
- Background printing is opt-in. A usable tray must exist before hiding the window. Tray Open restores the same document; Pause cancels waiting automatic jobs; Quit stops the process after unsaved changes are resolved.
- A per-user instance lock prevents two folder watchers from submitting the same label. File-content fingerprints reserve automatic jobs before submission. An uncertain submission stays reserved until explicitly cleared, and manual printing remains available for review.
- History retains bounded recent entries. Reprint uses recorded rendering options and asks for confirmation. Missing or changed source files require review.
- `src/design_render.rs` owns generated-code preflight. Invalid content or unreadable dimensions fail rendering before printing. Physical scan quality still requires a scanner test.
- Lists use bounded history and profiles. egui ComboBox owns selection, Window owns guided overlays, and ScrollArea owns scrolling. Existing themes and i18n routing apply to all new controls.
