# Extract pipeline

> Status: contract spec. Implementation lands in P3.

`usta extract <repo>` synthesizes a template from an existing repository
**deterministically** — no LLM calls, no network beyond cloning the source.

## Pipeline

```
   ┌─────────┐   ┌──────┐   ┌──────────┐   ┌──────────┐   ┌──────────┐   ┌────────────┐   ┌────────┐
   │ acquire │ → │ scan │ → │ classify │ → │ sanitize │ → │ synth    │ → │ verify     │ → │ write  │
   │ (clone) │   │ ignore│   │ buckets  │   │ tree-sit │   │ template │   │ scaffold-it│   │ to disk│
   └─────────┘   └──────┘   └──────────┘   └──────────┘   └──────────┘   └────────────┘   └────────┘
```

### 1. Acquire

If `<repo>` is a URL, clone shallowly into a tempdir. If it's a local path,
use it in place (read-only).

### 2. Scan (`RepoScanner`)

Walks the repo respecting `.gitignore` and an optional
`.usta-extract-ignore`. Returns `Vec<ScannedFile>`.

### 3. Classify (composed `StackDetector`s + path heuristics)

Each file is placed in one of:

- **`Infrastructure`** — kept verbatim. Config files, lockfiles, Dockerfiles,
  `.gitignore`, `tsconfig.json`, `eslint.config.js`, `nginx.conf`,
  `docker-compose.yaml`, etc.
- **`Anchor`** — kept, but bodies are stripped and `usta:*` markers are
  inserted where business code lived. Entry points: `main.py`, `main.tsx`,
  `App.tsx`, root router files, `vite.config.ts`.
- **`Primitive`** — kept verbatim. UI primitives, generic utils, anything
  that imports nothing from a domain folder.
- **`Business`** — dropped. Anything under `domain/`, `entities/`,
  `use_cases/`, `pages/<FeatureName>/`, `routers/<feature>.py`, etc.
- **`Ambiguous`** — interactive prompt with a preview. Default = drop.
  Batch-confirmable.

### 4. Sanitize (`SourceSanitizer` per language)

Tree-sitter–backed:

- Replace function/method bodies with the language-appropriate stub
  (`pass`, `throw new Error("not implemented")`, etc.).
- Remove now-unused imports.
- Replace project-specific identifiers via the `replacements` table
  (e.g. `my-existing-app` → `{{ project_name }}`, `MyExistingApp` → `{{ project_name | pascal }}`).

### 5. Synthesize (`TemplateSynthesizer` in `usta-app`)

- Group surviving files into features by **path heuristics** (e.g. files
  under `apps/api/src/infrastructure/mongodb/` → feature `api-mongodb`).
- Emit `template.toml` with declared features and prompts.
- Emit `merges/` and `injections/` for files multiple features touch.

### 6. Verify

Scaffold the new template into a tmpdir and run its declared post-hooks.
If lint/typecheck fail, mark the extract unstable and tell the user
exactly what to fix manually.

### 7. Write

Atomic move from tempdir to `--out`.

## `.usta-extract.toml`

Lives at the source repo root. Optional. Lets the user override
classification deterministically:

```toml
keep_paths   = ["packages/shared/ui/**"]
drop_paths   = ["apps/web/src/pages/Manager/**"]

# longest-first; CLI sorts before applying
[identifiers]
"my-existing-app"  = "{{ project_name }}"
"MyExistingApp"  = "{{ project_name | pascal }}"
"MY_EXISTING_APP"  = "{{ project_name | upper }}"

# manual feature partitioning, overriding heuristics
[[features]]
id = "api-mongodb"
paths = ["apps/api/src/infrastructure/mongodb/**"]

[[features]]
id = "web-i18n"
paths = ["apps/web/src/i18n/**", "packages/shared/i18n/**"]
```

## Determinism contract

- Same `<repo>` + same `.usta-extract.toml` + same `usta` version → same
  output, byte-for-byte.
- `--interactive` is allowed; the choices made are recorded into a sidecar
  TOML so re-running with `--replay <choices.toml>` is deterministic.
- A snapshot test in CI runs `extract` against a checked-in fixture repo
  and asserts the output tree.
