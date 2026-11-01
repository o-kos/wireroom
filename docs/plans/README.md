# Plans

Each implementation starts with an Issue and a committed plan copied from
[TEMPLATE.md](TEMPLATE.md), named `<issue>-<short-description>.md`. A plan is the
reviewable scope contract: state context, decisions, rejected alternatives, ordered
implementation steps and verifiable validation.

Commit the plan before implementation. Tick a step in the same commit that completes
it. Record changed decisions and their rationale, never commit hashes. Keep unrelated
findings in separate Issues. Move the plan to `completed/` before final review and
update links. The completion owner performs that move when implementation is delegated.

The [window shell implementation record](completed/28-window-shell.md) includes its
validation evidence and the remaining intent, CI and owner acceptance checks.
