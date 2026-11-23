#!/usr/bin/env bash
# Teste de fumaça da CLI para agentes: cria, edita, exporta e reimporta. Falha no primeiro erro.
set -euo pipefail
BIN="${TERRALUX:-target/release/terralux}"
D="$(mktemp -d)"
trap 'rm -rf "$D"' EXIT
ok() { echo "ok  $*"; }

$BIN templates --json | python3 -c 'import json,sys; d=json.load(sys.stdin); assert d["ok"] and len(d["templates"])>=10'; ok templates
$BIN new jogo-montanhas -o "$D/c.ron" >/dev/null; ok new
$BIN info "$D/c.ron" --json | python3 -c 'import json,sys; d=json.load(sys.stdin); assert d["terrain"]["kind"]=="mountains"' ; ok info
$BIN set "$D/c.ron" terrain.peak_height=77 sun.elevation_deg=30 --json | python3 -c 'import json,sys; d=json.load(sys.stdin); assert len(d["changed"])==2'; ok set
[ "$($BIN get "$D/c.ron" terrain.peak_height --json | python3 -c 'import json,sys; print(json.load(sys.stdin)["value"])')" = "77.0" ]; ok get
if $BIN set "$D/c.ron" terrain.peak_height='"x"' --json >/dev/null; then echo "set com tipo errado deveria falhar"; exit 1; fi; ok set-rejeita-tipo-errado
if $BIN set "$D/c.ron" terrain.nao_existe=1 --json >/dev/null; then echo "campo inexistente deveria falhar"; exit 1; fi; ok set-rejeita-campo
$BIN add "$D/c.ron" skull --name Caveira --color '#ff8800' --size 4 >/dev/null; ok add
$BIN add "$D/c.ron" sphere --pos 10,60,10 >/dev/null
$BIN validate "$D/c.ron" --json >/dev/null; ok validate
$BIN export "$D/c.ron" -o "$D/c.glb" --terrain-res 129 --json | python3 -c 'import json,sys; d=json.load(sys.stdin); assert d["triangles"]>30000'; ok export-glb
$BIN export "$D/c.ron" -o "$D/c.obj" --terrain-res 65 >/dev/null; $BIN export "$D/c.ron" -o "$D/c.stl" --terrain-res 65 >/dev/null; ok export-obj-stl
$BIN inspect "$D/c.glb" --json | python3 -c 'import json,sys; d=json.load(sys.stdin); assert d["triangles"]>0'; ok inspect
$BIN convert "$D/c.glb" -o "$D/conv.obj" >/dev/null; $BIN convert "$D/c.stl" -o "$D/conv.glb" >/dev/null; ok convert
$BIN export-terrain "$D/c.ron" -o "$D/h.png" --size 257 --json >/dev/null
$BIN export-terrain "$D/c.ron" -o "$D/h.raw" --size 257 >/dev/null
[ "$(stat -c %s "$D/h.raw")" = "$((257*257*2))" ]; ok export-terrain
$BIN from-heightmap "$D/h.png" -o "$D/d.json" --half-size 100 --height 50 >/dev/null
$BIN info "$D/d.json" --json >/dev/null; ok from-heightmap
$BIN remove "$D/c.ron" Caveira >/dev/null; ok remove
$BIN render "$D/c.ron" --width 160 --height 90 --spp 1 --fast -o "$D/r.png" >/dev/null 2>&1; [ -s "$D/r.png" ]; ok render
cat "$D/c.ron" | $BIN info - --json >/dev/null; ok stdin
out="$($BIN info "$D/naoexiste.ron" --json || true)"
echo "$out" | python3 -c 'import json,sys; d=json.load(sys.stdin); assert d["ok"] is False and d["error"]'; ok erro-em-json
if $BIN info "$D/naoexiste.ron" --json >/dev/null; then echo "deveria sair com 1"; exit 1; fi; ok codigo-de-saida
echo "CLI OK"
