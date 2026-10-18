# draw-rs

Terminal drawing library and standalone application, extracted from
[clin-rs](https://github.com/reekta92/clin-rs), revision `1e5710f3`.
Copyright 2026 clin-rs contributors. Licensed GPL-3.0-only; see LICENSE.

The `.draw` v2 format preserves item IDs and transforms; v0/v1 documents
migrate in memory without rewriting on load. Saving legacy documents removes
unsupported legacy images, as clin already did.
