# Private test data

Real instrument data is **never** committed. It lives in `./testData/` (git-ignored) or in the folder named by
`KVANTX_TEST_DATA_DIR`. CI never uses it: it runs synthetic tests only.

```text
testData/
├── manifest.json            describes the datasets (copy docs/testdata/manifest.template.json)
├── thermo_raw/              Thermo .raw files, any sub-folders (e.g. dataset_01/)
├── mzml/                    .mzML files
├── targets/                 target tables (.csv) that belong to the datasets
└── expected/                independently established results, only where they exist
```

## Running
```powershell
cargo test                                          # synthetic tests only (what CI runs)
cargo test --test real_data -- --ignored            # all real-data tests
cargo test --test real_data thermo -- --ignored     # Thermo RAW only (needs ThermoRawFileParser)
cargo test --test real_data mzml -- --ignored       # mzML only
cargo test -- --include-ignored                     # synthetic and real-data tests together
```
A real-data test that cannot find its data (or the converter) **fails with a clear message**; it never passes silently.
Real-data tests check structure and behavior (import, metadata, extraction, integration, consistency). They assert
concrete analytical results only where such results were established independently; none are asserted yet.
Do not put confidential sample names into `manifest.json`.
