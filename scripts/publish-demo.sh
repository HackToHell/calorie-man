#!/usr/bin/env bash
# Publish web/dist-demo (from `make demo`) to the gh-pages branch, which GitHub Pages serves at
# https://hacktohell.github.io/calorie-man/. Uses this repository's git identity and remote, adds one
# commit on top of the existing gh-pages history, and leaves the working tree untouched.
set -euo pipefail
cd "$(dirname "$0")/.."

site=web/dist-demo
remote=${REMOTE:-origin}
[[ -f $site/index.html ]] || { echo "No $site/index.html: run 'make demo' first." >&2; exit 1; }

src=$(git rev-parse --short HEAD)
[[ -z $(git status --porcelain) ]] || src="$src plus uncommitted changes"
echo "Publishing $site (built from $src) as $(git config user.name) <$(git config user.email)>"

# Build the commit in a throwaway index so nothing in the working tree or the real index changes.
export GIT_INDEX_FILE
GIT_INDEX_FILE=$(mktemp)
trap 'rm -f "$GIT_INDEX_FILE"' EXIT
rm -f "$GIT_INDEX_FILE"
git --work-tree="$site" add --all --force .
# No Jekyll: serve the files as they are.
git update-index --add --cacheinfo "100644,$(git hash-object -w --stdin < /dev/null),.nojekyll"
tree=$(git write-tree)

parent=()
if git fetch --quiet "$remote" gh-pages 2>/dev/null; then
  if [[ $(git rev-parse "FETCH_HEAD^{tree}") == "$tree" ]]; then
    echo "gh-pages already has this build; nothing to publish."
    exit 0
  fi
  parent=(-p FETCH_HEAD)
fi
commit=$(git commit-tree "$tree" "${parent[@]}" -m "Demo built from $src")
git push "$remote" "$commit:refs/heads/gh-pages"
echo "Pushed $(git rev-parse --short "$commit") to gh-pages."
