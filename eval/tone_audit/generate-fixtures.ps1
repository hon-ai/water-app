# Generates 200 tone-audit fixtures: 10 trigger classes x 5 personas x 4 manuscript excerpts.
# Excerpts are short, original prose stubs intentionally written in a register
# that does not echo any blacklisted phrase. They are committed alongside the
# generator so the fixture set is reproducible from source.

$triggers = @(
    "block_anchored_drift",
    "scene_flow_dip",
    "topic_drift",
    "valence_spike",
    "structural_inflection",
    "pace_floor",
    "world_drift",
    "no_universe_yet",
    "character_dissonance",
    "idle_pause_with_present_character"
)
$speakers = @("echo", "architect", "editor", "cartographer", "chorus")
$excerpts = @(
    "She walked across the square. The doors were open but no one went in. A bell rang somewhere she couldn't see.",
    "Rain on the slate roof. The lamp in the kitchen had been lit since before he came home. He hadn't asked her about the letter.",
    "The river bent twice before reaching the bridge, and at each bend it had left a stone that did not belong.",
    "Maps had been drawn for this country before there was a name for it, and the names came after, slowly."
)

$out = Join-Path $PSScriptRoot "fixtures"
New-Item -ItemType Directory -Path $out -Force | Out-Null

# Remove any stale fixtures so a re-run produces an exact set.
Get-ChildItem -LiteralPath $out -Filter "tone-*.json" -ErrorAction SilentlyContinue |
    Remove-Item -Force -ErrorAction SilentlyContinue

$id = 0
foreach ($t in $triggers) {
    foreach ($s in $speakers) {
        foreach ($e in $excerpts) {
            $id += 1
            $label = $id.ToString('000')
            $obj = [ordered]@{
                id            = "tone-$label"
                trigger       = $t
                speaker       = $s
                scene_excerpt = $e
                expected_pass = $true
            }
            $path = Join-Path $out "tone-$label.json"
            $json = ($obj | ConvertTo-Json -Depth 4)
            # Write UTF-8 *without* BOM. PS 5.1's `Set-Content -Encoding UTF8`
            # emits a BOM, which `serde_json::from_str` rejects ("expected value
            # at line 1 column 1"). The .NET overload is the portable choice.
            [System.IO.File]::WriteAllText($path, $json, (New-Object System.Text.UTF8Encoding($false)))
        }
    }
}

Write-Output "Generated $id fixtures in $out"
