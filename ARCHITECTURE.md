# KvantX architecture

KvantX is a **modular monolith**: one crate whose modules form layers with one-way dependencies. The layers correspond to the crates of the long-term plan in
`docs/roadmap.md`, so that each module can later become its own crate without redesign. The rule is enforced by `tests/layering.rs`.

```
gui  ──▶  app  ──▶  analysis, formats  ──▶  domain  ──▶  support
 egui     state,      calculations,          data         infrastructure
          workflows   files
```

| Layer (module) | Planned crate | Responsibility |
|---|---|---|
| `domain` | `kvantx-domain` | The data of an analysis: spectra, targets, parameters, enumerations. No I/O, no calculations. |
| `analysis` | `kvantx-analysis` | Chromatogram extraction, peak finding, integration, calibration, alignment. Pure functions on slices. |
| `formats` | `kvantx-formats` | mzML, `.kvantx` projects, target tables, ThermoRawFileParser, CSV / SVG / PNG output. |
| `app` | `kvantx-application` | `App` (state of the session), import, analysis workflow, undo, projects, export, settings and layout files. |
| `gui` | `kvantx-gui` | egui drawing: menu, toolbar, panels, plots, windows. Changes state only through `app` methods. |
| `support` | (shared) | SHA-256, zlib, audit trail, file locations, undo history, number formats, test data generator. |

## Data flow
```
.raw ─ThermoRawFileParser (stdout, in memory)─▶ mzML ─┐
.mzML ────────────────────────────────────────────────┴▶ formats::mzml ─▶ domain::MzData (MS1 + MS2)
target table ─▶ formats::targets ─▶ domain::Analyte
MzData + Analyte ─▶ analysis::xic ─▶ analysis::peaks (PeakFinder) ─▶ analysis::integration ─▶ analysis::calibration ─▶ results
```
`app::App` owns the samples, targets, fits and `AnalysisSettings`; the `gui` reads it every frame and calls its methods on user input (immediate-mode GUI).

## Design decisions
- **Scientific code is GUI-free and replaceable.** A peak method implements `analysis::peaks::PeakFinder` (boundaries only); integration and calibration are separate steps, so methods can be combined and compared.
  The legacy method and the adaptive method (Savitzky-Golay, noise estimate, prominence, derivative boundaries) are both available; manual boundaries (with a reason) override either.
- **One settings value.** `domain::settings::AnalysisSettings` holds ppm/Da, m/z tolerance, RT window, quantification method, peak method, signal, zoom and more. It is stored per session
  (`KvantX-default/settings.txt`), inside every project, in separate settings files, and has a reset to defaults.
- **Layout is data.** `app::layout::Layout` records open panels, splitter positions (as fractions of the window, so that panes keep their proportions) and window rectangles; it is restored at start, savable to a file and resettable. egui forgets remembered sizes when
  the generation counter `layout_gen` changes, which is how a loaded layout is applied.
- **Undo by snapshots.** Targets, boundaries and parameters are copied (10 steps); raw data is shared through `Arc`.
- **New project dialog.** `NewProjectForm` (in `app::state`) holds the answers of the dialog that opens at the first start and for every new project; `App::apply_new_project_form` applies them.
- **Background jobs** (import, conversion, saving) run in threads and report through typed `Msg` values; a guard sends `Finished` even if a thread panics.
- **Traceability.** One hash-chained audit trail per project (`<project>.audit.log`) or per unsaved session (`KvantX-default/audit/`); raw files are hashed.
- **Hostile input.** Bounded decoders, checked lengths, no `unsafe`, no shell, text neutralised in logs, CSV and SVG (`tests/security.rs`).
- **Nothing third-party in the build.** ThermoRawFileParser is an external program found at run time; `tests/packaging.rs` checks that it is not embedded.

## Extending KvantX
- **New peak or boundary method:** add a type implementing `PeakFinder` in `analysis/peaks.rs`, a variant in `domain::types::PeakMethod`, and one line in `peaks::finder`. The settings window lists all `PeakMethod::ALL`.
- **New file format:** add a reader in `formats/` that returns `domain::MzData`; call it from `app/files.rs`.
- **New setting:** add the field to `AnalysisSettings` (with default, key and range in `apply_pairs`); it is then saved, loaded and reset automatically; add a control in `gui/windows.rs`.
- **New panel:** add a `PanelId` and a field in `Layout`, draw it in `gui/dock.rs`.

## Source files
| File | Purpose |
|---|---|
| `src/main.rs` | Opens the main window and sets the icon. |
| `src/lib.rs` | Declares the layers and forbids `unsafe` code. |
| `src/support/sha256.rs` | Incremental SHA-256. |
| `src/support/inflate.rs` | Size-limited zlib decoder. |
| `src/support/json.rs` | Strict, size- and depth-limited JSON reader and writer (RFC 8259). |
| `src/support/audit.rs` | Hash-chained audit trails: logging, moving, verification. |
| `src/support/paths.rs` | Locations of settings, layout, audit trails and temporary files. |
| `src/support/history.rs` | Bounded undo / redo stack. |
| `src/support/fmt.rs` | Scientific notation and fixed formats of numbers. |
| `src/support/testkit.rs` | Synthetic mzML runs for the tests. |
| `src/domain/spectrum.rs` | `Spectrum` and `MzData`, the measured data of a file. |
| `src/domain/analyte.rs` | `Analyte`, a target with retention time, ions and precursor. |
| `src/domain/settings.rs` | `AnalysisSettings` with defaults, text format and limits. |
| `src/domain/types.rs` | Enumerations: file kind, quantification mode, signal, peak method, theme, boundary. |
| `src/analysis/xic.rs` | Extracted-ion chromatograms from spectra. |
| `src/analysis/smoothing.rs` | Savitzky-Golay smoothing and derivative, robust noise estimate. |
| `src/analysis/peaks.rs` | `PeakFinder` trait with the legacy and the adaptive method. |
| `src/analysis/integration.rs` | Trapezoid integration with optional straight baseline. |
| `src/analysis/calibration.rs` | Weighted linear regression. |
| `src/analysis/alignment.rs` | Retention-time alignment by cross-correlation. |
| `src/formats/mzml.rs` | mzML reader (base64, zlib, precursor ions). |
| `src/formats/rawconv.rs` | Finds and runs ThermoRawFileParser, confined to the temporary folder. |
| `src/formats/project.rs` | The `.kvantx` project: versioned JSON manifest (schema 1, validated field by field) in a container with optional embedded raw files; see `docs/PROJECT_FORMAT.md`. |
| `src/formats/targets.rs` | Parser of the target table. |
| `src/formats/csv.rs` | CSV fields with protection against spreadsheet formulas. |
| `src/formats/svg.rs` | SVG figures. |
| `src/formats/png.rs` | PNG writer. |
| `src/formats/icon.rs` | Decoder of the embedded `.ico`. |
| `src/app/state.rs` | `App` and its data types (`Sample`, `Cell`, snapshots, messages). |
| `src/app/session.rs` | Construction, settings files, selection, undo / redo, sorting of files. |
| `src/app/files.rs` | Import of `.raw`, mzML, projects and tables in background threads. |
| `src/app/analysis_ops.rs` | Extraction, peak finding, calibration, alignment and scan selection on the app state. |
| `src/app/targets_ops.rs` | Editing, loading and saving the target table. |
| `src/app/projects.rs` | Opening, saving and clearing projects. |
| `src/app/exports.rs` | Export of results, figures and data. |
| `src/app/layout.rs` | `Layout`: open panels and window sizes; save, load, reset. |
| `src/app/settings_io.rs` | Automatic saving of settings and layout; settings files; reset. |
| `src/gui/frame.rs` | The frame loop: messages, shortcuts, workspaces and windows. |
| `src/gui/menu.rs` | Menu bar and keyboard shortcuts. |
| `src/gui/toolbar.rs` | Toolbar with the workspaces and the status bar. |
| `src/gui/dock.rs` | Panels of the analysis workspace and the calibration workspace. |
| `src/gui/files_panel.rs` | File list with selection, type and context menu (including sorting). |
| `src/gui/chromatogram.rs` | Chromatogram views and boundary editing. |
| `src/gui/spectrum_panel.rs` | MS2 spectrum panel. |
| `src/gui/calibration_view.rs` | Results table, calibration plot, results matrix, calibration setup. |
| `src/gui/windows.rs` | Target table, analysis settings, preferences, audit trail and credits windows. |
| `src/gui/plotting.rs` | Window-free plot geometry: nearest point, zoom box, wheel scaling. |
| `src/gui/macros.rs` | Plot macros shared by the panels. |
| `src/gui/widgets.rs` | Color palette, number widgets and conversion of plots for export. |
| `src/gui/theme.rs` | Light / dark / system theme and the minimalist style. |
| `build.rs` | Embeds the icon into the Windows executable. |

## Tests (`tests/`)
One file per topic; `common/mod.rs` holds shared helpers. Unit-level: `fmt`, `csv`, `figures`, `icon`, `hashing`, `inflate`, `mzml`, `history`, `audit`, `paths`, `project_file` (manifest schema), `project_container`, `json`,
`quant` (extraction, integration, calibration, alignment), `analysis_peaks` (both peak methods, smoothing, noise), `settings_and_layout` (settings, layout, theme), `plotting`.
Application model without a window: `app_calibration`, `app_targets`, `app_selection`, `app_history`, `app_project`, `app_spectrum`, `app_settings`.
Rules: `layering` (layer dependencies), `security` (hostile input), `packaging` (nothing third-party embedded). `real_data` runs against private instrument data from `testData/`; its tests are `#[ignore]`d (not run by `cargo test` or CI) and fail, not skip, when data is missing (`docs/testdata/README.md`).
