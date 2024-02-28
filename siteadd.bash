#!/usr/bin/env bash
title="$(curl -sL "$1" | pup 'title text{}')"
dest="$HOME/entries/$2"

mkdir -p "$(dirname "$dest")"

cat <<EOF | tee "$HOME/entries/$2"
$title
$1
EOF
