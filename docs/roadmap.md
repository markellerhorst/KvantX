msQuant – Architecture and Refactoring Roadmap
Project Overview

msQuant is a Rust-based desktop application for quantitative LC-MS/MS data analysis.

The project aims to provide a maintainable, extensible, and scientifically reliable platform for mass spectrometry data processing, chromatographic peak integration, calibration, and quantification.

This document describes the architectural direction and key decisions for the planned refactoring.

1. Technology Decisions
Language: Rust
Rust Edition: 2024
GUI: egui / eframe
Build system: Cargo
Architecture: Modular monolith, evolving towards a Cargo workspace
Testing: Unit tests, integration tests, and scientific regression tests

Use stable Rust and avoid unnecessary dependencies or nightly-only features.

2. Architectural Principles

The application will follow Clean Architecture and Hexagonal Architecture principles.

The main objectives are:

Separation of scientific logic from GUI implementation.
Clear module responsibilities and dependency boundaries.
Independently testable analytical algorithms.
Replaceable data import and export implementations.
Explicit application state management.
Maintainable project persistence.
Reproducible scientific calculations.
Extensibility without unnecessary architectural complexity.
3. Planned Workspace Structure

The long-term architecture will use separate Rust crates:

text
msquant/
├── Cargo.toml
├── Cargo.lock
├── crates/
│   ├── msquant-domain/
│   ├── msquant-analysis/
│   ├── msquant-formats/
│   ├── msquant-application/
│   └── msquant-gui/
├── tests/
├── test-data/
└── docs/

Crate Responsibilities
Crate	Responsibility
msquant-domain	Scientific domain models and invariants
msquant-analysis	XIC extraction, peak detection, integration, calibration
msquant-formats	mzML parsing, RAW conversion, file formats
msquant-application	Application services and analytical workflows
msquant-gui	egui interface, visualization, and user interaction

Initially, establish these boundaries within a single crate. Extract separate crates once the interfaces are sufficiently stable.

4. Scientific Analysis Architecture

Scientific algorithms must remain independent of GUI and infrastructure code.

The analysis layer will contain separate components for:

Spectrum processing.
Extracted ion chromatogram generation.
Peak detection.
Peak integration.
Baseline correction.
Calibration and regression.
Retention-time alignment.

Peak detection and peak integration must be independently replaceable.

Multiple algorithms should be supported through well-defined Rust interfaces.

Peak Integration Strategy

Retain the existing integration algorithm as a legacy implementation.

Introduce an adaptive alternative based on:

Local peak detection.
Noise estimation.
Peak prominence.
Adaptive integration boundaries.
Baseline correction.
Trapezoidal numerical integration.

Both implementations should initially remain available for comparison.

No algorithm should be considered scientifically validated solely because its software tests pass.

5. Application and GUI Architecture

The GUI must not implement scientific calculations directly.

The application layer coordinates analytical workflows, while the presentation layer handles rendering and user interaction.

Application state should be divided into logical components:

Project state.
Sample state.
Analyte selection.
Chromatogram state.
Integration editing.
Calibration state.
Background task state.

Long-running operations must execute outside the UI thread.

Use typed messages and results for communication between background workers and the application.

6. Persistence and Reproducibility

Preserve compatibility with existing msQuant project files.

The persistence layer must support:

Project loading and saving.
Analytical parameters.
Sample metadata.
Integration boundaries.
Calibration information.
File references and SHA-256 hashes.
Optional embedded analytical data.
Audit history.

Introduce explicit schema versioning and migration mechanisms where necessary.

Manual analytical modifications must remain traceable.

7. Testing and Quality Assurance

The refactoring must preserve existing functionality and analytical behavior.

Required testing includes:

Unit tests for scientific algorithms.
Integration tests for analytical workflows.
Regression tests against existing behavior.
Project serialization compatibility tests.
Synthetic chromatographic test datasets.
Validation against manually reviewed chromatograms.

Important test scenarios include noisy baselines, asymmetric peaks, tailing peaks, overlapping peaks, and low signal-to-noise ratios.

8. Refactoring Strategy

Implement the architectural changes incrementally:

Analyze the existing codebase and establish baseline tests.
Upgrade to Rust Edition 2024.
Separate domain, analysis, application, infrastructure, and presentation responsibilities.
Refactor scientific algorithms into independently testable components.
Simplify application state management.
Improve background processing and error handling.
Preserve and improve project persistence.
Extract separate workspace crates when appropriate.
Document architecture and extension points.
Run complete tests and release builds.

Each phase must leave the application in a working, compilable