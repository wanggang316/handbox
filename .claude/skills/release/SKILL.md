---
name: release
description: Cut a HandBox release. Bump the version in package.json + src-tauri/tauri.conf.json + src-tauri/Cargo.toml (and regenerate Cargo.lock), promote CHANGELOG [Unreleased] to a dated version section, commit, tag vX.Y.Z (annotated, --cleanup=verbatim), and push to trigger the GitHub Actions Tauri release pipeline (signed + notarized macOS builds + updater latest.json). Use when shipping a new build.
---

# release: Cut a HandBox release

## Overview

A HandBox release is **a tag push**. The `.github/workflows/release.yml`
pipeline keys off `v*` tags: it creates a draft GitHub Release, builds
the Tauri app for macOS (Apple Silicon + Intel), code-signs with the
Apple Developer ID, notarizes, generates the Tauri **updater artifacts**
(`*.app.tar.gz` + `*.sig` + `latest.json`), uploads everything to the
release, and then **auto-publishes** it (flips the draft to live).

So the release contract is:

> tag `vX.Y.Z` ⇔ `version = X.Y.Z` in **all three** of `package.json`,
> `src-tauri/tauri.conf.json`, `src-tauri/Cargo.toml` (and `Cargo.lock`
> regenerated) ⇔ `CHANGELOG.md` has a `## [X.Y.Z] - YYYY-MM-DD` section.

This skill walks those artifacts into alignment, commits, tags, pushes,
and verifies CI started.

**Two HandBox-specific facts that change the stakes — internalize these:**

1. **CI auto-publishes.** The `publish-release` job flips the release
   from draft to live as soon as the builds succeed. Once you push the
   tag and CI goes green, the release is **public and updater clients
   start pulling it automatically**. There is no human publish gate.
   Pushing the tag is the point of no easy return.
2. **No CI gate verifies tag == version.** Unlike some pipelines, nothing
   in CI rejects a `vX.Y.Z` tag whose `version` fields say something
   else. The bundled app and the `latest.json` carry the version from
   `tauri.conf.json`; the tag name is independent. **This skill is the
   only guard** — get the bump right before tagging.

## When to use

- Ready to ship a new build to GitHub Releases / auto-updater clients.
- `[Unreleased]` in `CHANGELOG.md` has user-visible entries that warrant a release.

**Don't use** for:
- Hot-fixing CI or the release pipeline itself (no version bump).
- Local-only experiments — never tag without intent to publish (CI auto-publishes).

## Project facts (load before acting)

| Concern | Where it lives |
|---|---|
| Version (semver), 3 files in lockstep | `package.json` → `version`; `src-tauri/tauri.conf.json` → `version`; `src-tauri/Cargo.toml` → `version` |
| Lockfile | `src-tauri/Cargo.lock` — regenerate after bump (`cd src-tauri && cargo generate-lockfile`) |
| User-visible changelog | `CHANGELOG.md` (project root), Keep a Changelog format |
| Interactive human release script | `scripts/release.sh` (= `npm run release`) — does the same end-to-end, but **interactive** (`read` prompts). It is the human path; this skill is the agent path. **Run one or the other, never both** (double bump). |
| Release CI | `.github/workflows/release.yml` (tag-triggered on `v*`, also `workflow_dispatch`) |
| Tag format | `vX.Y.Z` **annotated**, created with `--cleanup=verbatim` |
| Build matrix | macOS `aarch64-apple-darwin` + `x86_64-apple-darwin` (Windows removed) |
| Distribution unit | macOS bundles (DMG/app), signed + notarized |
| Auto-update | Tauri updater plugin; clients poll `https://github.com/wanggang316/handbox/releases/latest/download/latest.json`; artifacts signed with `TAURI_SIGNING_PRIVATE_KEY`, pubkey embedded in `tauri.conf.json` |
| Past bump style | `chore: bump version to X.Y.Z` (commit), `Release vX.Y.Z\n\n<changelog>` (tag) |

HandBox is pre-1.0 (`0.1.x`) — **not** strict SemVer. Default cadence is
**patch**; bump minor when behavior is materially different.

## Process

### 0. Pre-flight — refuse to proceed if any check fails

Run in parallel via `Bash`:

```bash
git rev-parse --abbrev-ref HEAD                       # must be 'main' (or confirm an off-main release)
git status --porcelain                                # must be empty
git fetch origin --tags                               # refresh
git rev-list --left-right --count HEAD...origin/main  # must be '0  0' (or 0 N if local-ahead is intentional)
git tag --sort=-creatordate | head -3                 # most recent tags
grep -m1 '"version"' package.json                     # current version
```

Bail with a clear message if:
- Not on `main` and the user hasn't approved an off-`main` release.
- Working tree is dirty.
- Local diverged from `origin/main` in a way the user didn't intend.
- A tag for the proposed version already exists.

### 1. Decide the version — ask the user

1. Read the current `version` and the `## [Unreleased]` section of `CHANGELOG.md`.
2. Propose **patch** by default (`0.1.3 → 0.1.4`).
3. Show the user a summary:

   ```
   Current: 0.1.3
   Next:    0.1.4   <-- patch (default)
   Or:      0.2.0   <-- minor (user-visible scope changed)

   Unreleased highlights:
     Added — ...
     Changed — ...
     Fixed — ...
   ```

4. Wait for explicit confirmation of `X.Y.Z` before proceeding.
   **Never invent a version silently.**

### 2. Verify the changelog has shippable content

If `[Unreleased]` is empty (only category headers, no `- ` bullets),
**stop**. Ask the user to populate it (or run a changelog extraction
pass over recent commits) first, then resume.

**Writing style — user-facing language.** CHANGELOG follows
[Keep a Changelog](https://keepachangelog.com/) — *for humans, not
machines*. Rewrite raw commit messages; don't paste them.

Categories used by this project (the four the release script knows):

| Section | What goes here |
|---|---|
| `Added` | New features |
| `Changed` | Modifications to existing behavior, shortcuts, defaults |
| `Fixed` | Bug fixes |
| `Removed` | Features gone in this version |

`Deprecated` and `Security` are valid Keep-a-Changelog sections — add
them by hand when needed, but note `scripts/release.sh`'s extractor only
migrates the four above, so if you might fall back to the script, keep
those two out of `[Unreleased]`.

Entry rules, in priority order (earlier wins):

1. **Describe what the user gets.** Lead with the feature/outcome, or —
   for bugs — the symptom that's now gone. Skip the mechanism.
   - Good: "Tab colors survive a restart." / "Sidebar no longer flickers when switching chats."
   - Bad: "Persist `Tab.colorToken` to store." / "Fix race in `PaneHostView`."
2. **Clarity beats brevity.** One or two short lines per entry. Three lines means it's doing too much.
3. **Skip engineering-only changes** (refactors, renames, CI tweaks, lint, dep bumps, build-script edits). The git log is their home. Two exceptions: user-perceivable side effects (min-OS bump, noticeably faster startup → list under `Changed` with the user impact), and **removals / breaking changes (always listed)**.
4. **Drop developer jargon** — no commit prefixes (`feat:`/`fix:`), PR/issue numbers, hashes, module or type names, protocol terms (`EPIPE`, `WKWebView`). Refer to features by the UI surface ("Settings → Updates", "sidebar").
5. **Consolidate within a release.** Fold many commits chasing one bug/feature into one entry naming the end state. Bundle tiny polish into one bullet.

Sanity check each entry: *would a user who only uses the app care, and
could they understand it?* If either is "no", rewrite or drop.

### 3. Cut the changelog version section

Edit `CHANGELOG.md`:

1. Replace `## [Unreleased]` with two sections:
   - A fresh empty `## [Unreleased]` at top with the four headers
     (`### Added` / `### Changed` / `### Fixed` / `### Removed`) left empty.
   - The previous `[Unreleased]` body promoted under
     `## [X.Y.Z] - YYYY-MM-DD` (today's local date — `date +%Y-%m-%d`).
2. Show the diff to the user before writing.

### 4. Bump the three version files + regenerate the lockfile

All three `version` fields must match the tag. Edit each, then regen `Cargo.lock`:

```bash
# package.json:               "version": "X.Y.Z"
# src-tauri/tauri.conf.json:   "version": "X.Y.Z"
# src-tauri/Cargo.toml:        version = "X.Y.Z"   (the [package] one, not a dep)

cd src-tauri && cargo generate-lockfile && cd ..    # updates Cargo.lock to the new version
```

Verify all three agree before continuing:

```bash
grep -m1 '"version"' package.json
grep -m1 '"version"' src-tauri/tauri.conf.json
grep -m1 '^version' src-tauri/Cargo.toml
```

(Forgetting `Cargo.lock` leaves a stale entry and can fail the build.)

### 5. Optional build verification

Default: **skip** (CI catches real problems; a full local Tauri build is
slow). Run the fast type check only if you want the bump commit clean:

```bash
npm run check          # ~seconds; svelte-check
```

A full `npm run tauri build` only on explicit request.

### 6. Commit — only the release files

Show the user the staged diff first.

```bash
git add package.json src-tauri/tauri.conf.json src-tauri/Cargo.toml src-tauri/Cargo.lock CHANGELOG.md
git status            # confirm ONLY those five files are staged
git diff --cached      # final eyeball
```

Commit message (matches `scripts/release.sh` and project history):

```bash
git commit -m "$(cat <<'EOF'
chore: bump version to X.Y.Z
EOF
)"
```

No attribution trailer. (With `git commit -m`, default cleanup is
`whitespace`, so this is fine — but see the tag note below.)

### 7. Tag — annotated, `--cleanup=verbatim`, with changelog body

```bash
git tag --cleanup=verbatim -a vX.Y.Z -m "$(cat <<'EOF'
Release vX.Y.Z

<the CHANGELOG body for this version — paste the ### Added / ### Fixed
sections verbatim, user-facing language>
EOF
)"
```

**`--cleanup=verbatim` is mandatory.** A tag's default cleanup is
`strip`, which deletes every line starting with `#` — that would erase
all the `### Added` / `### Fixed` headers from the changelog body.
`verbatim` preserves them.

**Do not push yet.** Confirm the tag looks right:

```bash
git show vX.Y.Z --stat
```

### 8. Push — commit first, tag second

```bash
git push origin main
git push origin vX.Y.Z     # triggers .github/workflows/release.yml
```

Push order matters: the tag's commit must be on the remote before the
tag, or CI races a not-yet-visible commit.

Remember: once this tag's CI goes green, the release **auto-publishes**
and updater clients begin pulling it. If you want to dry-run without
shipping, use `workflow_dispatch` against an existing tag instead.

### 9. Verify CI started, and the release landed correctly

```bash
gh run list --workflow=release.yml --limit 1
gh run watch                              # follow the active run, optional
# when green:
gh release view vX.Y.Z                     # now published (auto)
gh release view vX.Y.Z --json assets -q '.assets[].name'   # confirm DMG/app + latest.json + .sig attached
```

Tell the user:
- CI status (queued / running / passed / failed).
- The release is **auto-published** — no manual publish needed.
- **Known gap:** the GitHub Release *body* is hardcoded to
  `Release vX.Y.Z` by the workflow, so the page won't show the
  changelog. To enrich it after publish (optional):
  ```bash
  gh release edit vX.Y.Z --notes-file <(awk '/^## \[X.Y.Z\]/{f=1;next} /^## \[/{f=0} f' CHANGELOG.md)
  ```
- Confirm `latest.json` is attached — without it, auto-update silently
  does nothing for existing users.

If CI fails, **do not** delete the tag or force-push without explicit
user approval — diagnose first (`gh run view --log-failed`).

## Failure modes & recovery

| Symptom | Cause | Fix |
|---|---|---|
| Build fails at notarization | Apple-side / secrets; often transient | `gh run view --log-failed`; re-run via `workflow_dispatch` against the existing tag |
| Auto-update does nothing for users | `latest.json` missing, or artifacts unsigned (bad `TAURI_SIGNING_PRIVATE_KEY`) | Confirm `latest.json` + `.sig` are in the release assets and `createUpdaterArtifacts` is true; re-run the build job |
| Version fields disagree (e.g. `tauri.conf.json` not bumped) | Skipped one of the three files in step 4 | No CI gate catches this. Bump the missing file in a new commit, delete & recreate the tag (confirm before deleting a remote tag) |
| Tag pushed without CHANGELOG section | Skipped step 3 | Add the section in a follow-up `docs(changelog): record vX.Y.Z` commit on `main`; do not retag |
| Tag body lost its `###` headers | Tagged without `--cleanup=verbatim` | Recreate the annotated tag with `--cleanup=verbatim` (delete + recreate; confirm before touching the remote tag) |
| Wrong version pushed | Bumped to e.g. 0.2.0 when 0.1.4 was intended | Roll forward — bump again and supersede with a new tag. Do not rewrite `main` history. |
| **Broken release already auto-published** | Bug/signing issue surfaces after CI flipped it live; updater clients are now pulling it | Move fast on two fronts: (a) **stop the bleed** — convert the bad release to a draft or prerelease so `releases/latest/download/latest.json` stops resolving to it (the updater endpoint only serves the latest non-prerelease), and/or `gh release delete vX.Y.Z`; (b) cut a follow-up version with the fix and mark the bad one `## [X.Y.Z] - YYYY-MM-DD [YANKED]` in `CHANGELOG.md` with a one-line reason. |

## Verification checklist

Before reporting "done":

- [ ] `git tag --sort=-creatordate | head -1` shows the new tag.
- [ ] `git log -1 --oneline origin/main` shows `chore: bump version to X.Y.Z`.
- [ ] All three `version` fields (`package.json`, `tauri.conf.json`, `Cargo.toml`) equal `X.Y.Z`, and `Cargo.lock` was regenerated.
- [ ] `CHANGELOG.md` has `## [X.Y.Z] - YYYY-MM-DD` with a non-empty body and a fresh empty `## [Unreleased]` above.
- [ ] `gh run list --workflow=release.yml --limit 1` shows a queued/running job.
- [ ] User told the release **auto-publishes** and that updater clients will pull it once green.

## Anti-patterns

- ❌ Bumping the version files and `CHANGELOG.md` in separate commits — keep the bump atomic.
- ❌ Bumping fewer than all three version files, or forgetting to regenerate `Cargo.lock`.
- ❌ `git add -A` / `git add -u` — stage only the five release files.
- ❌ Lightweight tag (`git tag vX.Y.Z`) or tagging without `--cleanup=verbatim` (drops the changelog headers).
- ❌ Pushing the tag before the commit.
- ❌ Running both `scripts/release.sh` and these steps — that double-bumps.
- ❌ Tagging a local experiment "just to test" — CI auto-publishes.
- ❌ Hand-editing `latest.json` — the CI pipeline is its only writer.
- ❌ Force-pushing `main` to "fix" a bad bump — roll forward instead.
