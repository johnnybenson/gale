---
name: open-migration-pr
description: Open a PR to migrate a repo from Stylelint to Gale. Takes a repo name (e.g., "bootstrap", "grafana") or GitHub URL.
disable-model-invocation: true
argument-hint: <repo-name>
---

# Open Gale Migration PR

Migrate the repo `$ARGUMENTS` from Stylelint to Gale by opening a PR.

## Process

### Step 1: Identify the repo

Find the repo in `tests/differential/repos.json` by name. Extract the GitHub `repo` field (e.g., `twbs/bootstrap`) and the `globs` field.

If the clone doesn't exist at `tests/differential/.clones/<name>`, clone it and install deps.

### Step 2: Verify parity (0 FP / 0 FN)

Run the differential test:
```
python3 tests/differential/run.py --skip-build <name>
```

If there are ANY false positives or false negatives, STOP and report them. Do NOT open a PR for a repo where Gale doesn't match Stylelint exactly.

### Step 3: Measure speedup

Run both linters with `time` from within the clone directory to get rough timing:
```bash
cd tests/differential/.clones/<name>
time node_modules/.bin/stylelint '<globs>' --quiet 2>/dev/null
time ../../target/release/gale '<globs>' --quiet 2>/dev/null
```

Calculate the speedup ratio.

### Step 4: Analyze the repo's package.json

Read the repo's `package.json` and understand:
- Where is `stylelint` listed? (`devDependencies`, `dependencies`, or both?)
- What scripts reference `stylelint`? (e.g., `"lint:css"`, `"stylelint"`, `"css-lint"`)
- Are there any monorepo-specific considerations? (workspaces, multiple package.json files)
- Is there a `.stylelintcache` reference?
- Are there other stylelint plugins in deps? (these stay — Gale reads the same config)

**IMPORTANT:** Only change:
1. The `stylelint` dependency → `@lyricalstring/gale`
2. The `stylelint` **command** in scripts → `gale`
3. `.stylelintcache` → `.galecache` (if present)

**DO NOT change:**
- Script names (keep `"lint:css"` as `"lint:css"`, keep `"css-lint-stylelint"` as-is)
- Config packages like `stylelint-config-standard` (Gale reads these)
- Plugin packages like `stylelint-scss` (Gale has built-in equivalents, but the packages don't hurt)
- The `.stylelintrc` config file itself

### Step 5: Fork and create the PR

1. Fork the repo: `gh repo fork <owner>/<repo> --clone=false`
2. Clone the fork: `gh repo clone <your-user>/<repo> /tmp/gale-pr-<name> -- --depth 1`
3. Create a branch: `git checkout -b gale-migration`
4. Apply the package.json changes
5. Commit with message: `build: replace stylelint with gale (~Nx faster CSS linting)`
6. Push: `git push -u origin gale-migration`
7. Open PR:

```bash
gh pr create --repo <owner>/<repo> --head <your-user>:gale-migration \
  --title "build: replace stylelint with gale (~Nx faster CSS linting)" \
  --body "$(cat <<'BODY'
## What this does

Replaces Stylelint with [Gale](https://github.com/LyricalString/gale), a Rust rewrite that reads your `.stylelintrc` directly and produces identical output — just ~Nx faster.

The only change is in `package.json`. No config migration needed.

## Verification

```bash
npx @lyricalstring/gale "<globs>"
```

This produces the same warnings as Stylelint on this repo (0 false positives, 0 false negatives — verified with [differential testing](https://github.com/LyricalString/gale#differential-testing)).

## Speed

| Tool | Time |
|------|------|
| Stylelint | X.XXs |
| Gale | X.XXs |

BODY
)"
```

### Step 6: Report

Show the user:
- The PR URL
- The speedup
- What was changed in package.json

## Notes

- Current Gale version: read from `Cargo.toml` workspace.package.version
- npm package: `@lyricalstring/gale`
- Check if a PR already exists before creating: `gh pr list --repo <owner>/<repo> --search "gale"`
