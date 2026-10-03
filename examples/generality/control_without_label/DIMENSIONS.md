# `control_without_label` — challenge dimensions

A form control must have something that names it.

| axis | file | why |
|---|---|---|
| indirect invalid | `caught.pw` | a `<select>` rather than an `<input>` |
| neighbour | `neighbour.pw` | the same control, with a `<label for>` |
| indirect invalid | `id-alone.pw` | an `id` and a placeholder, and no label points at the `id` |
| indirect invalid | `label-for-another.pw` | a label, for a different `id` |
| indirect invalid | `labelledby-nothing.pw` | `aria-labelledby` names an element that is not there |
| neighbour | `wrapped.pw` | the label wraps its control, with no `for` and no `id` |
| neighbour | `labelledby-heading.pw` | `aria-labelledby` names a heading with text |

The rows after the first two were added on 2026-10-02 (ADR-0143). Until then
an `id` counted as a name and was never checked, so each caught row passed.
`wrapped.pw` was refused, because the rule read only the control's own
attributes.
