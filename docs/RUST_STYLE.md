# Rust style guide

How we write Rust in this repo so it stays legible for years. Each rule is tagged by how it is
enforced:

- **[lint]**: clippy or rustc, failing CI via `-D warnings`.
- **[tool]**: rustfmt, cargo-deny, cargo-shear, typos, or a CI grep.
- **[review]**: a reviewer checks it.

When a rule and convenience disagree, the rule wins. When a rule and clarity truly disagree,
write the clear code and add a `// why:` comment that explains the exception.

---

## 1. Files and modules

1. **Tests never share a file with the code they test.** [tool: CI greps for `mod tests {`]
   - Every module with tests ends with `#[cfg(test)] mod tests;`, and the tests live in
     `<module>/tests.rs`. For the crate root, they live in `src/tests.rs`.
   - Shared fakes and builders live in `src/testing.rs`, behind
     `#[cfg(any(test, feature = "test-support"))]`.
2. **Use `foo.rs` plus a `foo/` directory, never `mod.rs`.** [lint `mod_module_files`] No module may
   be named after its parent (`view/view.rs`). [lint `module_inception`]
3. **Keep files small.** Aim for 400 lines of non-test code; 600 is the limit, after which you
   split. [review]
4. **`lib.rs` only declares modules and re-exports.** It holds no logic. Re-exports are explicit,
   never globs, and cover the 5–15 most-used types. [review]
5. **Wire a module into `lib.rs` in the same change that creates it.** Code the compiler has not
   seen does not count as written. [review]
6. **Use the smallest visibility that works**: private, then `pub(crate)`, then `pub`. Make fields
   `pub` only on plain data with no invariants. [lint `unreachable_pub`; review]
7. **Modules depend in one direction, with no cycles.** Example: model → classify → snapshot →
   diff → tombstone. Crates follow the graph `domain ← config, ado ← auth ← service ← platform,
   ui ← bin`. `prm-domain` does no IO, no async, no egui and no OS calls. [review]

## 2. Naming

1. **Follow RFC 430 casing, with acronyms as one word**: `PrKey`, `AdoClient`, never `PRKey`. [lint]
2. **Prefix conversions by cost**: `as_` is free and borrowed, `to_` allocates, `into_` consumes.
   Prefer `From`, `TryFrom` and `FromStr` impls. [review]
3. **Constructors:**
   - `new(parts)` cannot fail.
   - `parse(&str) -> Result` validates, paired with `FromStr` and `TryFrom<String>`.
   - `from_x` builds from an alternate source.
   - `with_x(self) -> Self` sets an option.
   - Never use `create_` or `make_`. [review]
4. **Getters are bare nouns** (`repo(id)`, `expose()`), with no `get_`. [review]
5. **Booleans read as predicates**: `is_`, `has_`, `can_`, `should_`. No negatives: write
   `enabled`, not `not_disabled`. [review]
6. **Errors are named `<Thing>Error`**, one enum per failure domain, and never a crate-wide
   catch-all `Error`. [review]
7. **No single-letter names**, except in closures of 3 lines or fewer and in conventional math
   (`i` as an index). Never rebind a name inside its own scope (`k @ …` inside a loop over `k`).
   [review]
8. **Use the same word for the same thing everywhere.** A `*_name` field is text; a bare
   `project` or `repo` field is an id. [review]

## 3. Types and invariants

1. **Every identifier and every validated string gets a newtype.** Never pass a raw `Uuid`, `i32`
   or `String` where an id is meant. [review]
2. **Validate once, at construction.** Use `#[serde(try_from = "String", into = "String")]` so
   deserialization cannot bypass the check. [review]
3. **Secrets are `Secret` or `SecretBytes`**: no `Display`, no `Serialize`, redacted `Debug`,
   zeroized on drop. Log the `TokenId`, never the value. [review]
4. **Use an enum instead of a bool parameter or a pair of related bools.** [review;
   lint `fn_params_excessive_bools`]
5. **Use `Option<T>` for absence, never a sentinel** (`""`, `0`, a nil UUID), unless an external
   API defines the sentinel. [review]
6. **Persisted and hashed values use dedicated, stable codes.**
   - Anything written to disk or hashed into persisted state gets its own value, commented
     "persisted: never renumber".
   - Never reuse a display-order helper for this.
   - Persisted enums are `snake_case`, and config enums put `#[serde(other)]` on their default
     variant.
   [review]
7. **Each domain rule lives in exactly one place.** That includes vote meanings, "blank string"
   checks and app-name strings. If a module doc claims a single source of truth, keep it true.
   [review]
8. **Build values once.** Compute derived fields first, then construct; never fill in a
   placeholder and patch it afterwards. [review]

## 4. Signatures and ownership

1. **Borrow by default** (`&str`, `&[T]`, `&T`). Take ownership only when the function stores or
   consumes the value. Use `impl Into<…>` only on storing constructors. [review]
2. **Shared immutable text is `Arc<str>`**, owned and never shared text is `Box<str>`, and text
   still being built is `String`. Return `&str` or `&[T]`, never `&String` or `&Vec`. [review]
3. **Pass small `Copy` types by value.** [lint `trivially_copy_pass_by_ref`]
4. **Take at most 5 parameters**; beyond that, pass a struct. A helper takes only the fields it
   uses, not a whole context struct. [lint `too_many_arguments`, threshold 5]
5. **Use struct literals by default.** Write a builder (`with_x` … `build() -> Result`) only for
   more than 6 optional knobs or validation across fields. [review]

## 5. Functions and control flow

1. **Keep functions to 60 lines or fewer, and aim for 30.** Split them into helpers named after
   the concern. [lint `too_many_lines`, threshold 60]
2. **Nest at most 3 levels.** Use early returns and `let … else` for preconditions. [lint
   `excessive_nesting`; review]
3. **Iterator chains are fine up to about 4 adaptors with pure closures.** Use a `for` loop when
   the body has side effects, early exits or more than one `if`. Closures are one expression;
   anything longer becomes a named `fn`. [review]
4. **No clever tricks.**
   - Prefer the obvious form over the shortest one: `any(..) && all(..)`, not a `peekable` trick.
   - `macro_rules!` is allowed only to remove 3 or more copies of real boilerplate. It must be
     private and documented, and must say why a function would not do.
   - No proc macros of our own.
   [review]
5. **Match exhaustively on our own enums, with no `_` arm**, so a new variant breaks every
   caller. Describe each variant's behaviour in one place (one `profile()` match), not in four
   parallel matches. `_` is fine on integers, strings and foreign `#[non_exhaustive]` enums.
   [lint `wildcard_enum_match_arm`; review]
6. **Name every magic number as a `const` with its unit** (`MAX_NAMED_AUTHORS`,
   `CATALOG_TTL: Duration`). User-facing text that mentions a limit is built from the const.
   [review]
7. **Mark trivially pure functions `const fn`.** [review]

## 6. Traits and generics

1. **Ports are small, single-purpose and object-safe.** Each is `Send + Sync + 'static` with at
   most about 5 methods, used as `Arc<dyn Port>`, and lives in the lowest crate that needs it.
   Use `#[async_trait]` only when the port is used as `dyn`. [review]
2. **Every port has a hand-written fake in `testing.rs`.** No mocking frameworks. [review]
3. **Don't write a trait with one implementation and no fake.** Use a concrete type. [review]
4. **Implement the std traits (`Display`, `FromStr`, `From`, `Default`, `Ord`) before inventing
   methods.** A type that holds a secret has a hand-written `Debug`. [review]
5. **Use `dyn` at architectural seams and generics inside a crate.** At most 2 generic
   parameters, and `where` clauses on their own lines. [review]
6. **Derive in a fixed order:** `Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Default,
   Serialize, Deserialize`. Public data types derive at least `Debug`, `Clone` and `PartialEq`.
   [review]

## 7. Errors

1. **Each crate has its own `thiserror` enum**, with one variant per distinct outcome and never a
   string where a variant fits. Any catch-all variant carries a `// why:` comment. `anyhow` is
   allowed only in the bin's `main`. [review]
2. **`#[error]` text is user-facing copy**: sentence case, it says what happened, and it never
   apologizes. Diagnostics (stderr, codes, detail) go in fields and logs. [review]
3. **Keep the cause chain with `#[source]` or `#[from]`.** Turn errors into strings only at the UI
   edge. Map foreign errors (reqwest, windows) once, at the crate boundary. [review]
4. **No `unwrap` or `expect` outside tests.** Use `?`, `get()`, saturating arithmetic and
   `let … else`. If an invariant truly cannot fail, write
   `#[expect(clippy::expect_used, reason = "…")]`. [lint `unwrap_used`, `expect_used`]
5. **No `panic!` or `unimplemented!` outside tests.** Return an error, or degrade gracefully and
   log. Test fixtures that fail loudly on bad literals use a module-level allow with a reason; a
   build script may panic to fail the build. [lint `panic`, `unimplemented`; clippy.toml
   `allow-panic-in-tests`]

## 8. Comments and docs

1. **Every module starts with `//!`, stating its contract in 1–3 lines.** [review]
2. **Every `pub` item has a `///` doc** saying why it exists, its invariants and its units. It
   must not restate the signature. [review]
3. **Inline `//` comments explain a non-obvious *why*** (an ADO quirk, Windows behaviour), with a
   link when one exists. [review]
4. **No commented-out code, no `todo!()`, and docs track the code.** Delete doc references to
   fields that no longer exist, and delete unused fields. [lint `todo`; review]

## 9. Logging

1. **Use structured fields**: `warn!(org = %org, status, "poll failed")`, not interpolated
   strings. [review]
2. **Levels:**
   - `error`: a user-visible feature is broken.
   - `warn`: degraded, or retrying.
   - `info`: lifecycle events.
   - `debug`: per-poll summaries.
   - `trace`: per-request detail.
   [review]
3. **Never log secrets, auth headers, URLs with query strings, or PR or comment bodies.** Use
   `#[instrument(skip_all, fields(..))]`. [review]
4. **No `println!`, `eprintln!` or `dbg!`.** [lint]

## 10. Tests

1. **Name tests `<unit>_<behavior>`**, test one behaviour each, and lay them out as
   arrange / act / assert. [review]
2. **Write table tests as a loop**, with the case in the assert message. [review]
3. **Tests are deterministic:**
   - no real network (use wiremock);
   - no wall clock (pass a `Timestamp` or `Clock`);
   - no `sleep` (use `start_paused` with `advance`);
   - no assertions that depend on `HashMap` iteration order;
   - seeded randomness only.
   [review; tool: CI greps for `thread::sleep` in tests]
4. **Golden values come from a run**, never typed by hand. Use insta for large structured output,
   and read every `.snap` diff like code. Use `assert_eq!` for scalars. [review]
5. **Fixtures go in `crates/<crate>/tests/fixtures/`**: trimmed, scrubbed real payloads, loaded
   with `include_str!`. [review]
6. **Tests that run the built exe live in `crates/pr-monitor/tests/cli.rs`**, using
   `env!("CARGO_BIN_EXE_pr-monitor")`, a temporary `PRMONITOR_HOME` and a bounded wait. They never
   start Run mode or `--unregister` (no windows, dialogs or registry). [review]
7. **A regression test says what broke.** Its `///` doc names the issue (#N) when there is one and
   the user-visible failure; assertion messages include captured output. [review]
8. **Parsers of untrusted input** (activation URIs, the handoff frame) get a seeded mutation sweep that
   asserts they never panic and that every successful parse round-trips. [review]

## 11. Suppressions, `unsafe` and dependencies

1. **Suppress with `#[expect(lint, reason = "…")]`**, as narrowly as possible. Use `#[allow]` only
   for cfg-dependent lints, and give it a reason. Workspace-wide allows live only in `Cargo.toml`,
   each with a comment. [lint `allow_attributes_without_reason`]
2. **`unsafe` is allowed only in `prm-platform/src/win/*`**, with per-module
   `#[allow(unsafe_code, reason)]`. Every other crate root says `#![forbid(unsafe_code)]`, so no
   local allow can slip past review.
   Each block makes one FFI call, carries a `// SAFETY:` comment, and sits behind a safe wrapper.
   [lint `unsafe_code` (forbid outside prm-platform), `undocumented_unsafe_blocks`,
   `multiple_unsafe_ops_per_block`]
3. **Declare every dependency in `[workspace.dependencies]`, and add it in the change that first
   uses it.**
   - Use minimal features and stay within MSRV 1.95.
   - Justify the binary-size cost in the PR.
   [tool `cargo shear`, `cargo deny`]

## 12. UI performance

1. **The egui frame is allocation-free in steady state.** View models are precomputed per
   snapshot, not per frame; no sorting or `format!` in `ui()`; pass `&[T]` or `Arc<[T]>`. [review]
2. **Clarity wins unless a measurement says otherwise.** An optimization cites its measurement in
   a comment. [review]

## Review checklist

- [ ] CI green: clippy `-D warnings`, fmt, deny, shear, typos, the test grep.
- [ ] Tests are in `<module>/tests.rs`, deterministic, named `<unit>_<behavior>`, with golden values from a run.
- [ ] New modules are wired into `lib.rs`; no dependency cycles; no `mod.rs`.
- [ ] Functions: ≤60 lines, ≤5 params, ≤3 nesting levels, descriptive names.
- [ ] No `_` arm on our own enums; one place per rule, per variant behaviour, per constant.
- [ ] Errors are variants; copy goes in `#[error]` and detail in fields; no `unwrap`/`expect`.
- [ ] Public items documented with why and invariants; no stale docs or dead fields.
- [ ] Logs are structured, with no secrets or PR content.
- [ ] Suppressions use `#[expect(.., reason)]`; `unsafe` only in prm-platform, with `SAFETY` comments.
- [ ] New dependencies justified; UI adds no per-frame allocation.
