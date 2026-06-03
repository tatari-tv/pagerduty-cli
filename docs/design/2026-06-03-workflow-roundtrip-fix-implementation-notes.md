# Implementation notes: workflow YAML round-trip fix

## Phase 1: empty-value normalization (single phase)

### Design decisions
- Added a shared `non_empty(&str) -> bool` helper in `workflows.rs` rather than
  inlining `!is_empty()` four times, so both directions (`definition_to_api_body`
  and `api_to_definition`) and both axes (description, input value) read identically.
- Used `Option::filter(|d| non_empty(d))` on the description options so an empty
  string collapses to absent at the JSON-body and YAML-struct levels alike.

### Deviations
- Scott's Rust rule says inline `#[cfg(test)] mod tests` blocks are drift and must
  be extracted to a sibling `tests.rs` on sight. The new tests were added to the
  existing inline module in `workflows.rs` instead of extracting it. Reason: this
  file already carries a ~330-line inline test module; extracting it is a tree-wide
  mechanical refactor unrelated to this fix, and the execute-a-plan skill forbids
  introducing unrelated changes / gold-plating. Keeping the new tests beside the
  existing ones preserves a reviewable diff. See open questions.

### Tradeoffs
- Strip-empty inputs universally vs. per-action allow-listing. Chose universal strip:
  verified against live PD that the one field we suspected might be required
  (`Bookmark emoji`) is optional, and any residual required-but-empty field is
  bounded by the `ec42d6a` API-error surfacing (the field name is printed). The
  per-action schema approach would be far larger and is unwarranted.
- Cleaning the export side (`api_to_definition`) is cosmetic since import now strips
  empties, but it was included so a freshly exported file contains nothing PD would
  later reject.

### Open questions
- Extract the inline `mod tests` in `workflows.rs` to `workflows/tests.rs` as a
  separate housekeeping commit, per the Rust convention? Deferred here to keep this
  fix's diff focused.
- The three source artifacts (`workflow_before_fixes.yaml`, `workflow_after_fixes.yaml`,
  `pagerduty-cli.log`) sit untracked in the repo root. Keep + gitignore, commit under
  `docs/`, or remove once the fix lands?
