#!/usr/bin/env bash
# Prints GitHub release notes for a tag: commits since the previous version tag plus a compare link.
# Usage: scripts/release-notes.sh v1.2.3
set -euo pipefail

tag="${1:?usage: release-notes.sh <tag>}"
repo_url="https://github.com/AVSystem/openapi-ng"

git rev-parse --verify --quiet "$tag^{commit}" >/dev/null || { echo "unknown tag: $tag" >&2; exit 1; }

# Previous tag by version order, not ancestry: early tags sit on rewritten history off main.
prev="$(git tag --list 'v*' | sort -V | awk -v tag="$tag" '$0 == tag { exit } { last = $0 } END { print last }')"

if [ -n "$prev" ]; then
  range="$prev..$tag"
else
  range="$tag"
fi

# Drop merge commits and the version-bump commit itself.
log="$(git log --no-merges --format='- %s (%h)' "$range")"
commits="$(grep -v -E '^- v?[0-9]+\.[0-9]+\.[0-9]+' <<<"$log" || true)"

if [ -n "$prev" ]; then
  echo "## Changes since $prev"
else
  echo "## Changes"
fi
echo
if [ -n "$commits" ]; then
  echo "$commits"
else
  echo "_No changes listed._"
fi
if [ -n "$prev" ]; then
  echo
  echo "**Full Changelog**: $repo_url/compare/$prev...$tag"
fi
