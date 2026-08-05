# Modal-Based Task Editing

Full design: `openspec/specs/web/overview.md` (Overview, Non-goals, `tasks.rs`,
`handlers/`, and HTMX Integration sections).

Replaces the per-task-list-row inline Update-status/Save-criteria forms with
a single "Edit" button that opens a modal (`<dialog>`) covering status,
acceptance criteria, `spec`, `depends_on`, and `blocks` in one combined save.
The acceptance-criteria field becomes a multi-line textarea with a
line-number gutter. Explicitly out of scope for this pass: Gherkin syntax
highlighting/linting, and a pills/chips UI for `depends_on`/`blocks`
(both noted as planned follow-ups in the spec).
