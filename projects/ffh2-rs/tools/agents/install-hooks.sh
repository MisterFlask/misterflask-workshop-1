#!/bin/bash
# install-hooks.sh [--uninstall | --check]: install (or remove) the post-checkout hook that makes new
# agent worktrees under .claude/worktrees/ sparse (root files, .claude/, projects/ffh2-rs/).
# See hooks/post-checkout and docs/AGENTS.md. Idempotent; run from any checkout of the repository.
#
# Installing copies the hook into the repository's hooks dir (shared by all worktrees) and sets
# extensions.worktreeConfig=true, so that sparse-checkout settings stay per worktree (this is what
# `git sparse-checkout` would do itself on first use). A post-checkout hook that is not ours is kept
# as post-checkout.local and still runs first.
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
src="$here/hooks/post-checkout"
mark='ffh2-rs agents: sparse agent worktrees'

common=$(git rev-parse --path-format=absolute --git-common-dir)
hooks=$(git config --get core.hooksPath || true)
if [ -z "$hooks" ]; then
    hooks="$common/hooks"
elif [ "${hooks#/}" = "$hooks" ]; then
    hooks="$(dirname "$common")/$hooks"     # relative core.hooksPath: relative to the main checkout
fi
dst="$hooks/post-checkout"

ours() { [ -f "$1" ] && grep -q "$mark" "$1"; }

case "${1:-}" in
--check)
    if ours "$dst" && cmp -s "$src" "$dst"; then
        echo "installed (current): $dst"
    elif ours "$dst"; then
        echo "installed (outdated; rerun install-hooks.sh): $dst"
    else
        echo "not installed ($dst)"
    fi
    echo "extensions.worktreeConfig = $(git config --get extensions.worktreeConfig || echo unset)"
    exit 0 ;;
--uninstall)
    if ours "$dst"; then
        rm -f "$dst"
        if [ -f "$hooks/post-checkout.local" ]; then
            mv "$hooks/post-checkout.local" "$dst"
            echo "restored the previous post-checkout hook"
        fi
        echo "removed $dst (existing sparse worktrees stay sparse: 'git sparse-checkout disable' in one restores it)"
    else
        echo "not installed"
    fi
    exit 0 ;;
'') ;;
*) echo "usage: install-hooks.sh [--uninstall | --check]" >&2; exit 2 ;;
esac

mkdir -p "$hooks"
if [ -e "$dst" ] && ! ours "$dst"; then
    if [ -e "$hooks/post-checkout.local" ]; then
        echo "error: $dst is not ours and post-checkout.local exists too; merge them by hand" >&2
        exit 1
    fi
    mv "$dst" "$hooks/post-checkout.local"
    echo "kept the existing hook as post-checkout.local (it runs first)"
fi
if [ "$(git config --get extensions.worktreeConfig || true)" != true ]; then
    # Linked worktrees then read <gitdir>/config.worktree too; core.bare=false and no core.worktree
    # in the common config need no migration (cf. git's own init_worktree_config()).
    if [ "$(git config --file "$common/config" --bool core.bare || echo false)" = true ] ||
       git config --file "$common/config" --get core.worktree >/dev/null; then
        echo "error: core.bare=true or core.worktree set in $common/config; enable worktreeConfig by hand (git help config)" >&2
        exit 1
    fi
    git config --file "$common/config" extensions.worktreeConfig true
    echo "set extensions.worktreeConfig=true in $common/config"
fi
tmp="$dst.tmp.$$"
cp "$src" "$tmp"
chmod 755 "$tmp"
mv -f "$tmp" "$dst"
echo "installed $dst"
