[//]: # ( ---------------------------------------------------------------------- )
[//]: # (+ Authors: 	Ran# <ran.hash@proton.me> )
[//]: # (+ Created: 	2026/11/12 13:19:21.692778 )
[//]: # (+ Revised: 	2026/11/14 01:02:22.596195 )
[//]: # ( ---------------------------------------------------------------------- )

# TODO

The backlog. Each item is self-contained: reading it alone is enough to
implement it fully, without asking anything. Work top to bottom unless an item
says otherwise.

Next item number: 10

## Index

- [8. Prove a mismatched release tag stops at verify](#8-prove-a-mismatched-release-tag-stops-at-verify)
- [9. Draw only the visible rows of the diff view](#9-draw-only-the-visible-rows-of-the-diff-view)

## Adding an item

- Write for someone who has never seen the conversation: what happens today,
  what should happen instead, and why each non-obvious decision was made.
- Be concrete: real file paths, names, sizes, colours, keys. No open "maybe";
  if the implementer must choose, say how to choose and how to check.
- Use the template below. Implementation is a numbered list of steps in the
  order to do them; Done when is a checklist.
- Number it with "Next item number" above, then increase that number.
- File order is priority: the first item is the next one to do.
- Numbers ascend in file order. Deleting leaves a gap; when adding at the top
  or moving an item breaks the order, renumber every item in file order (from
  the lowest number in use) and update every reference to them.
- Keep the Index in file order: one `- [<n>. title](#anchor)` line per item;
  update it whenever an item is added, retitled, moved or deleted.
- Screenshots go in `docs/todo/<n>-<slug>.png`, linked from the item.
- Split anything bigger than one sitting into items that ship alone.

## Implementing an item

1. Read the whole item and every file it names; check `git status` first.
2. If the code has drifted from the item, adapt and note it; if the item
   contradicts a newer decision, ask before building.
3. Do the Implementation steps in order.
4. Write the listed tests, plus any others the change needs.
5. Lint, format, update file headers and run the full test suite until clean.
6. For visible changes, run the app and check it in the states the item names.
7. Update the listed docs; write the changelog line when committing.
8. Tick every Done when line, with evidence.
9. Delete the item, its Index line and its screenshots; add follow-up work as
   new items.

## Template

```markdown
## <n>. <short imperative title>

**Why:** what happens today and what is wrong with it.
**Goal:** the result, as the user will see it.

### Behaviour
- One bullet per case, edge cases included.

### Out of scope
- What this item deliberately doesn't change.

### Implementation
1. `<path>`: what changes and why.

### Tests
- `<test file>`: each case and what it proves.

### Docs
- `<doc file>`: what to change. Changelog: "<line>".

### Done when
- [ ] Checkable result.
```

---

## 8. Prove a mismatched release tag stops at verify

**Why:** `.github/workflows/release.yml` ran for the real v0.1.0 tag
(2026-11-13): all three targets built, the draft had six files that passed
`b3sum --check`, and the notes were the changelog section. Its `verify` job,
which must stop a tag that disagrees with `Cargo.toml` before any build, has
only ever seen a matching tag. A bad tag that slipped through would publish
binaries named with the wrong version, which the in-app updater would then offer.

**Goal:** One throwaway tag that differs from `Cargo.toml` fails in `verify`,
no build job starts, and nothing from the test is left on GitHub.

### Behaviour
- A tag that differs from `Cargo.toml` fails in the `verify` job with
  "doesn't match Cargo.toml", and `build` and `publish` are skipped.

### Out of scope
- Re-proving the build, checksum and notes steps (done by v0.1.0).

### Implementation
1. Push the tag `v0.0.0-test.1` on the current `main` commit to the `github`
   remote only (`git tag v0.0.0-test.1 && git push github v0.0.0-test.1`).
2. Watch the run (`gh run list -R Ran-n/orin --workflow release.yml`) and check
   that `verify` failed with "doesn't match Cargo.toml" and no `build` job ran.
3. Clean up: delete the tag locally and on `github`
   (`git push github :refs/tags/v0.0.0-test.1`), and check that no release or
   draft named `v0.0.0-test.1` exists (`gh release list -R Ran-n/orin`).
4. If `verify` let the tag through, fix `release.yml` and repeat.

### Tests
- The run itself.

### Docs
- None, unless `release.yml` changes ("Fixed …" in the changelog then).

### Done when
- [ ] A mismatched tag fails at `verify`, with no build.
- [ ] The test tag is deleted from GitHub and locally.

## 9. Draw only the visible rows of the diff view

**Why:** the Code column's diff view (`Workspace::code_content` in `src/desktop/code_layout.rs`, the
`div().id("diff-content")` built from `self.content.lines()`) creates one element per diff line on every frame. Git
output may be up to 8 MiB (`MAX_GIT_OUTPUT` in `src/project/git.rs`), which is over 100 000 lines for a large
generated file. Each frame then builds, lays out and drops all of them, so scrolling and typing elsewhere stutter
while such a diff is open. The editor already shapes only the rows on screen (`EditorLine` in
`src/desktop/editor/paint.rs`); the diff view should get the same treatment. The agent transcript
(`AgentView::conversation` in `src/desktop/agent_layout.rs`) has the same shape but is capped at 1 000 messages and
2 MiB (`MAX_MESSAGES`, `MAX_TEXT_BYTES` in `src/agents/model.rs`), so it is left for later.
**Goal:** a diff of any size scrolls as smoothly as a ten-line one, and looks exactly as it does today.

### Behaviour
- Every diff line keeps its colour and background: `+++`/`---` muted, `+` accent on `added_surface`, `-` error on
  `removed_surface`, `@@` walnut, others `soft_text`; each row is `DIFF_LINE_HEIGHT` tall.
- Horizontal scrolling still reaches the end of the longest line.
- The scroll position is kept when the diff refreshes with the same file selected (the project watcher refreshes it).
- An empty diff still shows its centred state ("This file has no unstaged changes." or "…no staged changes."),
  and "Loading…" until Git answers (`NO_CHANGES` in `src/desktop/code_layout.rs` marks the empty answer).

### Out of scope
- The agent transcript (see Why). Syntax colours inside diffs.

### Implementation
1. `src/desktop/code_layout.rs`: split `self.content` into lines once when the diff arrives (store a `Vec<(Range<usize>,
   LineKind)>` next to `content` in `Workspace`, filled where `Event::Diff` is handled in
   `src/desktop/event_handling.rs`), instead of on every render. `LineKind` is an enum of the five
   cases above, so the colour choice lives in one `match`.
2. `src/desktop/code_layout.rs`: replace the `.children(...)` list with GPUI's `uniform_list("diff-content", count,
   ...)`, which renders only the visible range; every row is the same height, which is what `uniform_list` needs.
   Keep the id `diff-content`, which the rendered tests use.
3. Give the list a `UniformListScrollHandle` stored in `Workspace`, reset only when the selected file or the
   staged/unstaged choice changes, so a watcher refresh keeps the position.
4. For horizontal scrolling, measure the longest line once (in characters, with the code font's advance from
   `window.text_system()`), and set that width on each row so the list scrolls sideways as before.

### Tests
- `src/desktop/code_layout.rs` (unit): `LineKind` for `+++ a`, `--- b`, `+x`, `-y`, `@@ -1 +1 @@`, ` z` and an empty
  line; the precomputed ranges cover `content` exactly.
- `tests/rendered/` (Windows, real driver): open a 50 000-line generated diff; the window keeps answering the driver
  within 1 s, a capture shows coloured `+` and `-` rows, and after scrolling to the end the last line is visible.
- Existing rendered diff tests keep passing unchanged.

### Docs
- `docs/development.md`: note under the desktop section that long lists (editor, diff) draw only visible rows.
- Changelog: "Large diffs scroll smoothly".

### Done when
- [ ] A 50 000-line diff scrolls without visible stutter, and the rendered test proves it.
- [ ] Colours and row heights match the current view in a side-by-side capture.
- [ ] fmt, clippy (`--all-targets --all-features -D warnings`), all tests and the coverage gate pass.
