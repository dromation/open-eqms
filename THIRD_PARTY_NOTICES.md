# Third-Party Notices

Open-EQMS is licensed under GPL-2.0 (see `LICENSE`). This file credits third-party
open source software incorporated into this repository as a build dependency.

This list is generated from `Cargo.lock` and must stay exact: it lists every crate
dependency that is not itself an Open-EQMS package (`open-eqms-*`). As of this
writing, that is exactly one crate.

## regex-lite

- **Version:** 0.1.9
- **Source:** `https://github.com/rust-lang/regex` (`regex-lite` subdirectory)
- **Registry:** crates.io
- **License:** MIT OR Apache-2.0 (dual-licensed; terms below are the MIT option)
- **Used by:** `runtime/object-runtime`, `runtime/event-engine`

```
Copyright (c) 2014 The Rust Project Developers

Permission is hereby granted, free of charge, to any
person obtaining a copy of this software and associated
documentation files (the "Software"), to deal in the
Software without restriction, including without
limitation the rights to use, copy, modify, merge,
publish, distribute, sublicense, and/or sell copies of
the Software, and to permit persons to whom the Software
is furnished to do so, subject to the following
conditions:

The above copyright notice and this permission notice
shall be included in all copies or substantial portions
of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF
ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED
TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A
PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT
SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY
CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION
OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR
IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER
DEALINGS IN THE SOFTWARE.
```

## Maintenance rule

Every future change that adds a new external (non-`open-eqms-*`) crate dependency to
any `Cargo.toml` in this workspace must update this file in the same commit: crate
name, exact version (from `Cargo.lock`), upstream source, license, and which
Open-EQMS component depends on it. A Codex instruction that adds a dependency
without updating this file is incomplete. This mirrors the existing repository
discipline already applied to `runtime/*` scope boundaries (AGENTS.md rule 16:
avoid speculative dependencies) — dependencies are already meant to be rare and
deliberate; this file makes each one individually traceable.
