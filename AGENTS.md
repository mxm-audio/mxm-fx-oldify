# AGENTS.md — mxm-fx-oldify

DOX rail for this repository. Project-wide instructions, durable workflow rules, and the
top-level Child DOX Index.

---

# DOX framework

- DOX is a highly performant AGENTS.md hierarchy installed here
- Agents must follow DOX instructions across any edits

## Core Contract

- AGENTS.md files are binding work contracts for their subtrees
- Work products, source materials, instructions, records, assets, and durable docs must stay
  understandable from the nearest applicable AGENTS.md plus every parent AGENTS.md above it

## Read Before Editing

1. Read the root AGENTS.md
2. Identify every file or folder you expect to touch
3. Walk from the repository root to each target path
4. Read every AGENTS.md found along each route
5. If a parent AGENTS.md lists a child AGENTS.md whose scope contains the path, read that child and
   continue from there
6. Use the nearest AGENTS.md as the local contract and parent docs for repo-wide rules
7. If docs conflict, the closer doc controls local work details, but no child doc may weaken DOX

Do not rely on memory. Re-read the applicable DOX chain in the current session before editing.

## Update After Editing

Every meaningful change requires a DOX pass before the task is done.

Update the closest owning AGENTS.md when a change affects:

- purpose, scope, ownership, or responsibilities
- durable structure, contracts, workflows, or operating rules
- required inputs, outputs, permissions, constraints, side effects, or artifacts
- user preferences about behavior, communication, process, organization, or quality
- AGENTS.md creation, deletion, move, rename, or index contents

Update parent docs when parent-level structure, ownership, workflow, or child index changes. Update
child docs when parent changes alter local rules. Correct stale or contradictory text immediately, and move its history to `NOTES.md` rather than deleting it; a discarded idea or replaced design is deleted outright, not archived: git keeps it (the owner, 2026-10-08).
Small edits that do not change behavior or contracts may leave docs unchanged, but the DOX pass
still must happen.

## Hierarchy

- Root AGENTS.md is the DOX rail
- Child AGENTS.md files own domain-specific instructions and their own Child DOX Index
- Each parent explains what its direct children cover and what stays owned by the parent
- The closer a doc is to the work, the more specific and practical it must be

## Child Doc Shape

- Create a child AGENTS.md when a folder becomes a durable boundary with its own purpose, rules,
  responsibilities, workflow, materials, or quality standards
- Work Guidance must reflect current project standards or user instructions; leave it empty if
  there are none yet
- Verification must reflect an existing check; leave it empty until one exists

Default section order: Purpose · Ownership · Local Contracts · Work Guidance · Verification ·
Child DOX Index

## Style

- Keep docs concise, current, and operational
- Document stable contracts, not diary entries
- **A value the code holds is named, not copied** (the owner, 2026-09-24: *"Why are you writing the
  opening pages size in such detail. Is that not already described in the code?"*). A size a test
  derives — an opening size, a minimum, a card floor — or a constant the code declares is stated in
  DOX as its rule, its constant and the test that holds it, never its number: a copied number goes
  stale the day the code moves. Two exceptions: the design system states its own tokens and
  rules, because it is the normative source the code implements; and a plan's revision history
  records what was measured when — a dated record, not the current value
- Put broad rules in parent docs and concrete details in child docs
- Prefer direct bullets with explicit names
- Do not duplicate rules across many files unless each scope needs a local version
- Keep AGENTS.md current: correct a stale note and move its history to `NOTES.md` instead of explaining it here;
  a discarded idea is deleted, never kept as history
- Trim obvious statements, repeated rules, misplaced detail, and warnings for risks that no longer
  exist

## Closeout

1. Re-check changed paths against the DOX chain
2. Update nearest owning docs and any affected parents or children
3. Refresh every affected Child DOX Index
4. Correct stale or contradictory text; move the reasoning still in force to `NOTES.md`, and delete a
   discarded idea outright
5. Run existing verification when relevant
6. Report any docs intentionally left unchanged and why

---


# Purpose

**mxm-fx-oldify** is an MXM effect that makes sound old: cassette tape and old records (vinyl and
shellac), from a new deck to a battered 78 on a radiogram.

**Pre-alpha. Both engines are in.** The DSP crate holds the chain — Drive, the record and the tape, Width
and Mix against the dry input delayed by the chain's fixed latency — and both media: the
record (era, player, cutting and replay curves, groove, groove echo, skips and a stuck groove, the
disc's surface, hum) and the cassette
(noise reduction, saturation, the measured tape responses, generations, print-through, wow and
flutter with Flange, a tape stop, hiss that follows or stops with the input, dropouts, the deck's faults and the
age's spacing loss); a Vinyl and a Tape switch put each in the chain, and both on tape the record. The plugin syncs the wobble rates to
the host's tempo and reports the latency and the chain's tail to the host, and the plugin ships a
factory bank of fifty sounds across both media and past them, and a Quality switch whose High works the
tape's saturation and the moving read harder. The §14 brief is [`docs/briefs/mxm-fx-oldify.md`](docs/briefs/mxm-fx-oldify.md).

It is one of the MXM products, each in its own repository under
[github.com/mxm-audio](https://github.com/mxm-audio), built on the MIT-licensed
[mxm-kit](https://github.com/mxm-audio/mxm-kit) — the design system, keyboard navigation, presets,
modulation, the control map and the checks every plugin shares.

Reference-quality open source: clarity beats cleverness, and every nontrivial algorithm names the
technique or paper it comes from.

# Ownership

Root owns `Cargo.toml`, `Cargo.lock`, `LICENSE`, `NOTICE.md`, `TRADEMARKS.md`, `README.md`,
`CONTRIBUTING.md`, `.cargo/`, `.github/`, `bundler.toml`, `test-bundles.txt` and `xtask/`.
Each folder with an `AGENTS.md` owns its contents; the index is below.

**Dependencies follow each repository's `main`, and `Cargo.lock` pins the exact commit.** The kit
and the player come from their `main`, and nice-plug and egui-baseview from their MXM forks
(`[patch.crates-io]`).

**Two tiers of tests.** `cargo test` builds the plugin and its DSP only — the loop for a
change. `plugins/mxm-fx-oldify/host-tests` loads the release bundle through MXM Player: it
is a separate package so the fast tier never builds the player.

## Windows, Linux and macOS — all three, always

**An absolute requirement.** Everything here runs on all three; a change that works on one and
breaks another is a broken change. CI builds and tests on all three, when started by hand (the
owner, 2026-10-06); during the work only Windows is checked, and Linux and macOS later, together
(*Verification*).

- **Anything platform-specific is `cfg`-gated with every arm implemented**, never one arm and a
  silent nothing elsewhere.
- **Linux needs system libraries** the other two carry in their SDKs — ALSA (and JACK) for audio,
  and X11, xkbcommon and a GL loader for the window.
- **A dependency that does not support all three cannot be taken**, whatever else it offers.

## MSRV is per crate

| Crate | MSRV | Why |
|---|---|---|
| `crates/mxm-fx-oldify-dsp` | **1.87** | No dependencies at all: the chain, both engines and their building blocks. **Verified on 1.87**, library and tests |
| `plugins/mxm-fx-oldify` | **1.95** | Its dynamically paged editor pulls in egui; depends on `mxm-fx-oldify-dsp`, which stays at 1.87 |

## Licensing

**GPL-3.0-or-later** (`LICENSE`). `NOTICE.md` lists the third-party code in its builds. The MXM
name and logo are not covered by the licence: see `TRADEMARKS.md`.

- **MPL-2.0 is accepted** for symphonia, through `mxm-audio-file-decode` only, used unmodified;
  never vendor, patch or modify an MPL crate.
- Check the licence before porting any algorithm, and record source and licence in a comment at
  the top of the file. Cite techniques even when the implementation is original.

## The design and its measurements are private

The research, the plan and our own measurements behind this effect are in the workspace's private
`ops` repository, under `ops:notes/oldify/` (`reports/`, `research_notes/`, `measurements/`).
**Cite them as plain text in a code span, never a link**, exactly as a `research:` page is cited,
and nothing here may depend on them at build or test time. Facts, numbers and our own
measurements cross into this repository; a third-party product's name, files, renders and verbatim
text never do — no name here claims a maker, a model or a commercial plugin (mxm-kit's
`docs/collection-rules.md`, *Naming*).

## Research citations

A citation written `` `research:<path>` `` names a page in MXM's private research repository. It
is plain text in a code span, never a link, and nothing here depends on it at build or test
time. Facts, numbers, our own measurements and short quotations cross into this repository;
third-party files, images and verbatim text never do.

# Verification

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test                                   # the fast tier: the plugin and its DSP
cargo xtask bundle mxm-fx-oldify --release
cargo xtask fetch                            # plugins from other repositories its tests load
cargo test -p mxm-fx-oldify-host-tests       # the slow tier: through MXM Player
```

Before a push, run the first three on Windows; Linux and macOS are checked later, together (the
owner, 2026-10-06). CI runs the same on Windows, macOS and Linux, only when started by hand, so
only CI reaches macOS.

# Child DOX Index

| Doc | Scope |
|---|---|
| [`crates/mxm-fx-oldify-dsp/AGENTS.md`](crates/mxm-fx-oldify-dsp/AGENTS.md) | The chain and the cassette engine: order, latency, tail, smoothing, exact paths, the building blocks (filter sections, tape responses, transport, saturation, spacing), sample rates and numeric contracts |
| [`plugins/AGENTS.md`](plugins/AGENTS.md) | Shared plugin conventions: nice-plug, the init patch, presets, `process()` rules, the editor contract, and installing a bundle in a DAW |
| [`plugins/mxm-fx-oldify/AGENTS.md`](plugins/mxm-fx-oldify/AGENTS.md) | Identity, the sixty-seven permanent parameters and the retired `medium` and `output`, Init as the DSP's init patch, tempo sync, latency and the endless tail, the factory bank, the empty control map, telemetry and the eight-card editor with its two group switches |
