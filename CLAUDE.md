# Claude Code Project Instructions

The active project in this repository is the Rust port in `projects/ffh2-rs/`
(start with `projects/ffh2-rs/CLAUDE.md`).
Other top-level directories hold standalone writing, research and tooling.

### Unit animations

To make idle, strike, flinch or death animations for an M7 unit sprite, follow
`projects/ffh-anim-sprint/ANIMATING_UNITS.md`. It describes the method that won the October 2026
animation sprint (`lib/recipe.py` in that folder); do not start from an ad-hoc approach. Its first
rule: the strike always goes in the direction the sprite is looking. In a sparse agent worktree
that folder is not checked out; run `git sparse-checkout add projects/ffh-anim-sprint` first.

### Agent worktrees are sparse

New worktrees under `.claude/worktrees/` check out only the root files, `.claude/`
and `projects/ffh2-rs/` (a hook from `projects/ffh2-rs/tools/agents/install-hooks.sh`).
To work on a different project there, run `git sparse-checkout disable` first.
