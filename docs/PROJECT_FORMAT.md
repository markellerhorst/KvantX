# The `.kvantx` project format (schema 1)

A project is one file that stores the analysis state of KvantX and, optionally, copies of the original
instrument files. KvantX 0.3 and later write schema 1. **Projects of earlier versions (`.quant`) are not
supported.**

## Container

```text
offset 0      8 bytes   magic: "KVANTX" 0x00 0x01   (the last byte is the version of the container layout)
offset 8      8 bytes   length N of the manifest in bytes (unsigned, little endian)
offset 16     N bytes   manifest: UTF-8 JSON (below)
then          for every sample whose "embedded" is true, in the order of the manifest:
              8 bytes length L (little endian), then L bytes: the original file
```

The file ends after the last embedded file; anything after it is an error. A reader refuses a file whose magic
is not `KVANTX`, whose layout version it does not know, whose lengths do not fit the file, whose manifest is
larger than 64 MiB or is not valid UTF-8 JSON, or that has trailing data.

## Manifest (JSON)

| Field | Type | Meaning |
|---|---|---|
| `format` | text | always `"kvantx-project"` |
| `schema` | integer | schema version, here `1`. A larger value is refused with an explanation, never guessed |
| `saved_by` | object | `application` (`"KvantX"`) and `version` of the program that wrote the file |
| `saved_utc` | text | time of saving, ISO 8601, UTC |
| `user` | text | user name at the time of saving |
| `settings` | object | all analysis settings: m/z tolerance and unit, RT window, quantification mode, weighting, peak method, baseline option, signal (MS1/MS2), ion-ratio tolerance, alignment settings, zoom. Values are text, numbers or `true`/`false` |
| `targets` | list | per target: `name`, `rt_min` (number or `null`), `product_ions` (list, the first is the quantifier), `precursor_mz` (number or `null`) |
| `samples` | list | the input files, see below |
| `audit` | object | `entries` (number of audit-trail entries) and `head` (hash of the last one) when the project was saved |

Per sample:

| Field | Type | Meaning |
|---|---|---|
| `path` | text | path of the input file as imported (unique within the project) |
| `raw_sha256` | text | SHA-256 of the **input** file, 64 lowercase hexadecimal digits |
| `kind` | text | `Blank`, `Standard`, `QC` or `Unknown` |
| `rt_shift_min` | number | retention-time shift (alignment), at most 120 min |
| `selected` | boolean | selection state |
| `embedded` | boolean | whether the original file is stored in the container |
| `import` | object | `route`: `direct` (mzML read by KvantX) or `conversion` (converted by an external program); `detail`: format, converter, version and parameters as recorded at import |
| `cells` | list | integration state per target |

Per cell: `target` (index into `targets`, unique per sample), `peak_found`, `manual` (boundaries set by hand),
`start_min`, `end_min` (reversed windows are refused), `use_for_calibration`, `standard_conc` (0 = not a standard,
never negative) and `manual_reason` (the reason given for the last manual change; empty if none).

Unknown extra fields are ignored, so that a later version can add information within schema 1. A change that
could alter the meaning of existing fields raises `schema`.

## Validation and limits

Everything in a project is untrusted. The reader validates field by field (types, ranges, finite numbers, known
enumerations, references between lists) and fails with a message that names the place. Limits: 2000 targets,
2000 files, 2000 cells per file, 64 product ions per target, 256 settings, 256 characters in a name, 4096 in a
path, 8192 in a free text, nesting depth 32. The writer reads its own output back and validates it before the
file is written, so that a project that could not be opened again is never saved. Non-finite numbers are never
written.

## What is preserved

- the identity of every input file (path, SHA-256, import route and conversion details),
- the scientific settings, the targets and the integration boundaries of every target in every file,
- the standard concentrations and the reason of every manual change,
- an anchor to the audit trail: when a project is opened, KvantX verifies the hash chain of the trail next to
  the project (`<project>.audit.log`) and checks that the entry at the recorded position has the recorded hash.

## Limits of this format

- The manifest is not signed and not hashed: someone who can edit the file can edit the manifest. The audit
  anchor detects accidental damage and simple edits of the audit trail only; a local hash chain is not a
  signature and does not make the system compliant with 21 CFR Part 11 or GxP.
- Embedded raw files are verified by the recorded SHA-256 when they are imported, not when the project is opened.
- Results (areas, concentrations) are not stored; they are recomputed from the data and the stored state.
