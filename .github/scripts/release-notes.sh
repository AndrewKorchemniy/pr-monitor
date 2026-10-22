#!/usr/bin/env bash
# Release notes from the pull requests merged since the last published release. Each pull
# request's title is a heading, followed by what its description says (its "What and why", not
# the test notes or checklist) and the commits it was squashed from. Needs only `gh`.
#
# Usage: release-notes.sh <owner/repo> <target-sha> <tag>
# PREV_TAG overrides the release the notes start from (default: the latest published one).
set -euo pipefail

repo=$1
sha=$2
tag=$3
prev=${PREV_TAG:-$(gh release list -R "$repo" --exclude-drafts --exclude-pre-releases -L 1 \
  --json tagName --jq '.[0].tagName // ""')}

# The pull requests behind the commits in range, oldest first, each once.
pulls() {
  local range commits
  if [ -n "$prev" ]; then
    commits=$(gh api "repos/$repo/compare/$prev...$sha" --jq '.commits[].sha')
  else
    commits=$(gh api "repos/$repo/commits?sha=$sha&per_page=100" --jq 'reverse | .[].sha')
  fi
  for commit in $commits; do
    gh api "repos/$repo/commits/$commit/pulls" \
      --jq '[.[] | select(.merged_at != null) | .number][0] // empty'
  done | awk '!seen[$0]++'
}

# A description's "What and why": no HTML comments, nothing from the test notes or checklist on,
# no attribution footer, and its own headings demoted under the pull request's.
summary() {
  awk '
    /<!--/ { comment = 1 }
    comment { if (/-->/) comment = 0; next }
    /^## *(How it was tested|Checklist)/ { exit }
    /Generated with \[Claude Code\]/ { next }
    /^## *What and why *$/ { next }
    /^##+ / { sub(/^#+/, "####") }
    { print }
  ' | sed -e '/./,$!d' | sed -e ':a' -e '/^\n*$/{$d;N;ba' -e '}'
}

echo "## What's changed"
for number in $(pulls); do
  title=$(gh api "repos/$repo/pulls/$number" --jq '.title')
  url=$(gh api "repos/$repo/pulls/$number" --jq '.html_url')
  echo
  echo "### $title ([#$number]($url))"
  body=$(gh api "repos/$repo/pulls/$number" --jq '.body // ""' | tr -d '\r' | summary)
  if [ -n "$body" ]; then
    echo
    echo "$body"
  fi
  commits=$(gh api "repos/$repo/pulls/$number/commits" --paginate \
    --jq '.[].commit.message | split("\n")[0]' |
    grep -v -e '^Merge ' | grep -v -x -F -e "$title" || true)
  if [ -n "$commits" ]; then
    echo
    echo "<details><summary>Commits</summary>"
    echo
    while IFS= read -r line; do echo "- $line"; done <<<"$commits"
    echo
    echo "</details>"
  fi
done

if [ -n "$prev" ]; then
  echo
  echo "**Full changelog**: https://github.com/$repo/compare/$prev...$tag"
fi
