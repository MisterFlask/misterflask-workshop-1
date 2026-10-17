# Claude Code Project Instructions

The active project in this repository is the Rust port in `projects/ffh2-rs/`
(start with `projects/ffh2-rs/CLAUDE.md` and `projects/ffh2-rs/docs/HANDOFF.md`).
Other top-level directories hold standalone writing, research and tooling.

### Agent worktrees are sparse

New worktrees under `.claude/worktrees/` check out only the root files, `.claude/`
and `projects/ffh2-rs/` (a hook from `projects/ffh2-rs/tools/agents/install-hooks.sh`).
To work on a different project there, run `git sparse-checkout disable` first.
