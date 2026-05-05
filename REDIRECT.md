# This repository has moved

The example tree and book of `presentar` have been consolidated into the
**APR Cookbook** umbrella project as part of the sovereign-stack
documentation centralization (spec: [docs/specifications/centralize-cookbooks](https://github.com/paiml/apr-cookbook/blob/main/docs/specifications/centralize-cookbooks.md)).

| Where it used to live | Where it lives now |
|-----------------------|--------------------|
| `examples/{ald,apr,charts,dashboards,edge_cases,prs}/` (28 declarative configs) | https://github.com/paiml/apr-cookbook/tree/main/examples/visualization |
| `book/src/` (mdBook chapters: getting-started, architecture, layout, examples, quality, advanced, appendix) | https://github.com/paiml/apr-cookbook/tree/main/book/src/visualization |

A single Rust validator at `apr-cookbook/examples/visualization/load_visualization.rs`
loads every `.yaml` and `.prs` file via `serde_yaml`, asserts schema validity,
and runs as part of the cookbook's test suite (per
[iiur-conformance.md](https://github.com/paiml/apr-cookbook/blob/main/docs/specifications/centralize-cookbooks/iiur-conformance.md)
Class 2 Strategy B). One validator covers all 28 configs; per-file wrappers
were not generated since the configs share schema.

This repository is now archived (read-only). For new contributions, please use:

- **Cookbook examples and book**: https://github.com/paiml/apr-cookbook
- **Crate source code**: still published on crates.io as `presentar`,
  `presentar-core`, `presentar-layout`, `presentar-terminal`, `presentar-cli`.
  See https://crates.io/crates/presentar

Last live tag (rollback anchor): `pre-archive-2026-05`

For full migration rationale, see:
https://github.com/paiml/apr-cookbook/blob/main/docs/specifications/centralize-cookbooks.md
