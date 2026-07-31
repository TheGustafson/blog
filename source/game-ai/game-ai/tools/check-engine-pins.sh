#!/usr/bin/env bash
set -euo pipefail

game_ai_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
repository_root=$(cd "$game_ai_root/.." && pwd)

engines=(
  "tictactoe:gai-tictactoe"
  "connect4:gai-connect4"
  "othello:gai-othello"
  "chess:gai-chess"
)

for entry in "${engines[@]}"; do
  engine=${entry%%:*}
  repository=${entry##*:}
  path="game-ai/games/$engine"
  directory="$repository_root/$path"
  expected_url="https://github.com/TheGustafson/$repository.git"
  configured_url=$(git -C "$repository_root" config --file .gitmodules --get "submodule.$path.url")
  version=$(sed -n 's/^version = "\([^"]*\)"$/\1/p' "$directory/Cargo.toml" | head -n 1)
  tag=$(git -C "$directory" describe --tags --exact-match HEAD)

  test "$configured_url" = "$expected_url"
  test "$tag" = "v$version"
  test -z "$(git -C "$directory" status --porcelain --untracked-files=all)"

  printf '%s %s %s\n' "$repository" "$tag" "$(git -C "$directory" rev-parse --short HEAD)"
done
