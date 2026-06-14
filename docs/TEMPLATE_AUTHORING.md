# Template authoring

> Status: implemented. The manifest schema and the built-in templates
> (`hello-world`, `nx-monorepo`) ship in v0.1.0.

A template is a folder under `templates/<id>/` with a manifest and a set of
features. The engine never special-cases template ids — every template uses
the same machinery.

## Folder layout

```
templates/<id>/
├── template.toml             # required: manifest
├── AGENTS.md.j2              # required: agent rules seed for the generated project
├── tests/
│   └── snapshot.rs           # required: scaffold-and-assert test
├── base/                     # always copied / rendered
│   └── …                     # mirror of the project root
└── features/<feature_id>/    # one folder per opt-in feature
    ├── feature.toml          # local manifest (deps, hooks)
    ├── files/                # plain copy; *.j2 rendered via minijinja
    ├── merges/               # JSON / TOML deep-merges into anchor configs
    │   └── package.json.merge.json
    └── injections/           # anchor-marker injections into existing files
        └── apps_api_src_main_py.inject.toml
```

## `template.toml`

```toml
[template]
id           = "nx-monorepo"
display_name = "Nx Monorepo (TS + Python)"
version      = "1.0.0"
min_usta     = "0.1.0"
stacks       = ["typescript", "python"]   # informational

[[features]]
id           = "api-fastapi"
display_name = "API: FastAPI + uv"
default      = true
requires     = []
conflicts    = []
stacks       = ["python"]

[[prompts]]
id        = "scope"
type      = "text"
question  = "npm scope (without @)"
default   = "{{ project_name | kebab }}"
validate  = "^[a-z][a-z0-9-]*$"

[hooks]
post_scaffold = ["fmt", "install"]
```

## Anchors

Anchor files live in `base/` (or `features/*/files/`) with marker comments:

```py
# usta:imports
# usta:lifespan_startup
# usta:routers
```

Each feature contributes via `injections/<flat_path>.inject.toml`:

```toml
[[at]]
marker  = "usta:imports"
content = "from src.infrastructure.mongodb.client import connect_to_mongo"

[[at]]
marker  = "usta:lifespan_startup"
content = "await connect_to_mongo()"
```

The engine concatenates contributions in the order features were resolved
(stable: by `feature.id`) and strips markers from the final output.

## Merges

Multiple features may extend the same `package.json` / `pyproject.toml`.
Each contributes a `merges/<flat_path>.merge.json` (or `.merge.toml`):

```json
{
  "dependencies": {
    "@tanstack/react-query": "^5.62.7"
  }
}
```

Merge semantics:

- **Objects**: deep-merged.
- **Arrays**: concatenated, deduplicated by stable equality.
- **Scalars**: last writer wins; warn on conflict.
- **Semver dep ranges**: union (caret-bumped to the higher minor).

## Templating

We use **minijinja** (Jinja-compatible). Available filters:

| Filter | Example | Output |
|---|---|---|
| `kebab` | `{{ "MyApp" \| kebab }}` | `my-app` |
| `pascal` | `{{ "my-app" \| pascal }}` | `MyApp` |
| `camel` | `{{ "my-app" \| camel }}` | `myApp` |
| `snake` | `{{ "my-app" \| snake }}` | `my_app` |
| `upper` / `lower` | standard | — |

Template variables available globally:

- `project_name` (validated kebab-case)
- `scope` (npm scope, no `@`)
- All answers from prompts (keyed by `id`).

## Generated `AGENTS.md.j2`

Every template MUST ship an `AGENTS.md.j2` describing the architecture
choices baked into the generated project (e.g. "this is an Nx monorepo,
prefer `nx run` over the underlying tools; the API uses ports/adapters with
adapters under `infrastructure/`; commands live in
`application/use_cases/`"). This way AI agents working on the generated
project stay on the rails the template author intended.

## Test contract

`templates/<id>/tests/snapshot.rs` MUST:

1. Scaffold the template into `tempfile::tempdir()`.
2. Walk the output tree and assert via `insta::assert_yaml_snapshot!`.
3. Run the template's declared `post_scaffold` hooks and assert they
   succeed.

CI posts the diff of the snapshot when the template changes, as a PR
comment.
