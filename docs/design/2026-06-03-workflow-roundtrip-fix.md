# Incident-workflow YAML round-trip fix

## Problem

`pd incident workflow export` produces a YAML file that `pd incident workflow import`
(and `create --from-file`) cannot consume without manual editing. Exporting a workflow,
changing its name, and re-importing - the canonical "clone a workflow" flow - fails.

The PagerDuty create/update API rejects any field whose value is an empty string with
`400 Bad Request: "... is not allowed to be empty"`. It validates one empty field per
request, so the failure surfaces repeatedly, one field at a time.

Three categories of empty value are emitted by PD on export but rejected by PD on import:

1. **Empty `description` (workflow *and* step).** PD exports `description: ""`; create
   rejects it. (An *absent* description is accepted - the plain `create` path already
   omits it.) The workflow-level case is what surfaced in `pagerduty-cli.log`, but
   `StepYaml.description` is the same `Option<String>` and is serialized the same way
   (`definition_to_api_body`, workflows.rs), so a step exported with `description: ""`
   is an identical latent failure. Both levels are normalized.
2. **Empty `Select the Channel` inputs.** When a Slack step uses
   `Channel: Incident Dedicated Channel`, PD's UI auto-adds a companion
   `Select the Channel` input with an empty value. It is not needed in that mode, but
   export carries it through and import rejects the empty value.
3. **Empty `Bookmark emoji`.** A bookmark step with no emoji exports `Bookmark emoji: ""`;
   import rejects the empty value.

Source artifacts for this report: `workflow_before_fixes.yaml`, `workflow_after_fixes.yaml`,
`pagerduty-cli.log` (the six sequential `not allowed to be empty` 400s).

## Verification

Whether dropping an empty input is *universally* safe (versus the emoji being a required
field that must be set, not dropped) was resolved against live PagerDuty. A probe workflow
replicating the full `pshelby` structure, with the Jira bookmark's emoji input dropped
entirely (one variable changed from a known-good import), imported successfully. Conclusion:
`Bookmark emoji` is optional; dropping any empty-valued input is safe. No per-field
special-casing is required. The probe workflow was deleted afterward.

## Fix

The unifying rule, now verified: **an empty-string value is meaningless to the PagerDuty
API - it is neither accepted nor semantically distinct from absence.** Normalize empties
out in both directions of the round-trip.

### Import side (load-bearing) - `definition_to_api_body`

This single function feeds both `import` (via the `_disabled` wrapper) and
`create --from-file`. Fixing it here covers every write path:

1. Omit `description` from the body when it is `None` or empty - at **both** the workflow
   level and the per-step level (matching the proven `create()` behavior).
2. Drop any input whose `value` is empty before assembling the step's `inputs` array.

This makes any definition importable - whether produced by `export` or hand-written.

**Residual risk and its mitigation.** The empty-string rule is the API's own behavior, not
an inference, but it is conceivable that some action (current or future) requires a field
yet accepts an empty string as a meaningful "blank." For such a field, dropping the empty
input would trade `"... is not allowed to be empty"` for `"... is required"`. This is
strictly no worse than today, and is bounded by the API-error surfacing added in `ec42d6a`:
the failing field name and the request URL are printed, so the operator sees exactly which
field PD demands. The claim is therefore "round-trip succeeds, and when PD still rejects a
field it is named," not "round-trip always succeeds."

Note also that intentionally authoring `value: ""` to *clear* a previously set field is not
a capability being removed: PD rejects an empty-string value outright, so that request never
succeeds today. Omitting the input is the only mutation that reaches the API at all.

### Export side (cosmetic) - `api_to_definition`

Apply the same rule so the exported YAML is clean and self-consistent:

1. Map an empty `description` to `None` (serialized as absent / null, not `""`) - at both
   the workflow and step levels.
2. Skip inputs with empty values when building each step's `inputs`.

Not required for correctness once import strips empties, but it keeps the exported artifact
tidy and means a freshly exported file contains no fields PD would later reject.

## Testing

TDD, unit-level (no live API):

- `definition_to_api_body` omits `description` when empty and when `None`; keeps it when
  non-empty - asserted at both the workflow and step levels.
- `definition_to_api_body` drops empty-valued inputs and retains non-empty ones, preserving
  order.
- `api_to_definition` maps empty `description` to `None` (workflow and step) and drops
  empty-valued inputs.
- A round-trip test over a step shaped like the real `Incident Dedicated Channel` /
  bookmark cases asserts no empty value survives into the API body.

## Out of scope

- No change to trigger handling, shadow-workflow resolution, or enable/disable flow.
- No attempt to model PagerDuty's per-action input schema; the empty-string rule is
  sufficient and verified.
