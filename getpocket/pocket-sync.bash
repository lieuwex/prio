DIR="$HOME/entries/pocket"

entries="$(jq -c '.[]' output.json)"

mkdir -p "$DIR"

echo "$entries" | while read e; do

status="$(jq -r '.status' <<< "$e")"
if [[ "$status" != '0' ]]; then
	continue
fi

id="$(jq -r '.item_id' <<< "$e")"
title="$(jq -r '.resolved_title' <<< "$e")"
url="$(jq -r '.resolved_url' <<< "$e")"
excerpt="$(jq -r '.excerpt' <<< "$e")"
time_updated="$(jq -r '.time_updated' <<< "$e")"

cat <<EOF > "$DIR/$id"
$title
$excerpt

$url

https://getpocket.com/read/$id
EOF

touch -d "@$time_updated" "$DIR/$id"

done
