#!/usr/bin/env bash
# build-docs.sh — Regenera la documentación de este repositorio (kit común raysac).
#
# Vive en docs/ de cada repo y se puede ejecutar desde cualquier directorio.
#
#   docs/diagramas/fuentes/*.puml  ->  docs/diagramas/svg/*.svg  y  docs/diagramas/png/*.png
#   docs/informes/<slug>/*.tex     ->  <slug>.pdf (tectonic)  y  <slug>.docx (pandoc)
#
# Uso:  build-docs.sh [--check] [--force] [-h|--help]
#   (sin opciones)  genera sólo lo desactualizado y actualiza docs/.build-hashes
#   --force         regenera todo aunque esté al día
#   --check         NO modifica docs/: regenera todo en un directorio temporal (comprueba que
#                   compila y que los resultados son válidos) y falla (código 1) si algún
#                   .svg/.png/.pdf/.docx versionado falta o está desactualizado respecto de
#                   su fuente. La comparación es por hash de las fuentes (docs/.build-hashes),
#                   no por fecha, porque PDF/DOCX incluyen marcas de tiempo y los archivos
#                   cambian de mtime al clonar con git.
#
# Variables de entorno:
#   PLANTUML   ruta al ejecutable de PlantUML (por defecto /Users/demian/Documents/Github/_sii/orch/bin/plantuml,
#              o 'plantuml' del PATH)
#
# Códigos de salida: 0 ok · 1 documentación desactualizada o error de generación ·
#                    2 uso incorrecto · 3 falta una herramienta
set -euo pipefail

DOCS_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)"
PLANTILLA="$DOCS_DIR/plantilla"
FUENTES="$DOCS_DIR/diagramas/fuentes"
SVG_DIR="$DOCS_DIR/diagramas/svg"
PNG_DIR="$DOCS_DIR/diagramas/png"
INFORMES="$DOCS_DIR/informes"
HASHFILE="$DOCS_DIR/.build-hashes"
PLANTUML_DEFECTO="/Users/demian/Documents/Github/_sii/orch/bin/plantuml"

CHECK=0
FORCE=0

log()  { printf '%s\n' "$*"; }
warn() { printf 'AVISO: %s\n' "$*" >&2; }
die()  { printf 'ERROR: %s\n' "$*" >&2; exit 1; }

usage() { sed -n '2,25p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'; }

for arg in "$@"; do
  case "$arg" in
    --check)    CHECK=1 ;;
    --force)    FORCE=1 ;;
    -h|--help)  usage; exit 0 ;;
    *) printf 'ERROR: opción desconocida: %s\n\n' "$arg" >&2; usage >&2; exit 2 ;;
  esac
done

# ------------------------------------------------------------------ fuentes
PUML_FILES=()
for f in "$FUENTES"/*.puml; do [ -e "$f" ] && PUML_FILES+=("$f"); done
TEX_FILES=()
for f in "$INFORMES"/*/*.tex; do [ -e "$f" ] && TEX_FILES+=("$f"); done

# ------------------------------------------------------------------ herramientas
resolver_plantuml() {
  if [ -n "${PLANTUML:-}" ]; then
    if [ -x "$PLANTUML" ] || command -v "$PLANTUML" >/dev/null 2>&1; then return 0; fi
    return 1
  fi
  if [ -x "$PLANTUML_DEFECTO" ]; then PLANTUML="$PLANTUML_DEFECTO"; return 0; fi
  if command -v plantuml >/dev/null 2>&1; then PLANTUML="$(command -v plantuml)"; return 0; fi
  return 1
}

comprobar_herramientas() {
  local faltan=0
  if [ ${#PUML_FILES[@]} -gt 0 ]; then
    if ! resolver_plantuml; then
      faltan=1
      printf 'ERROR: no se encontró PlantUML%s.\n' "${PLANTUML:+ (PLANTUML=$PLANTUML)}" >&2
      printf '       Defina PLANTUML=/ruta/a/plantuml o instale: brew install plantuml openjdk\n' >&2
      printf '       (esperado en %s o en el PATH).\n' "$PLANTUML_DEFECTO" >&2
    fi
    if ! command -v dot >/dev/null 2>&1; then
      warn "no se encontró 'dot' (graphviz); los diagramas de clases/componentes pueden fallar. brew install graphviz"
    fi
  fi
  if [ ${#TEX_FILES[@]} -gt 0 ]; then
    if ! command -v tectonic >/dev/null 2>&1; then
      faltan=1
      printf "ERROR: no se encontró 'tectonic' (LaTeX -> PDF). Instale: brew install tectonic\n" >&2
    fi
    if ! command -v pandoc >/dev/null 2>&1; then
      faltan=1
      printf "ERROR: no se encontró 'pandoc' (LaTeX -> DOCX). Instale: brew install pandoc\n" >&2
    fi
    local req
    for req in raysac-informe.cls pandoc-macros.tex pandoc-filtro.lua referencia.docx; do
      if [ ! -f "$PLANTILLA/$req" ]; then
        faltan=1
        printf 'ERROR: falta plantilla/%s en %s (copie el kit común).\n' "$req" "$DOCS_DIR" >&2
      fi
    done
  fi
  if [ ${#PUML_FILES[@]} -gt 0 ] && [ ! -f "$PLANTILLA/estilo.puml" ]; then
    faltan=1
    printf 'ERROR: falta plantilla/estilo.puml en %s (copie el kit común).\n' "$DOCS_DIR" >&2
  fi
  [ "$faltan" -eq 0 ] || exit 3
}

# ------------------------------------------------------------------ hashes
if command -v sha256sum >/dev/null 2>&1; then
  SHA256() { sha256sum | cut -d' ' -f1; }
elif command -v shasum >/dev/null 2>&1; then
  SHA256() { shasum -a 256 | cut -d' ' -f1; }
else
  printf "ERROR: se requiere 'sha256sum' o 'shasum'.\n" >&2; exit 3
fi

# Hash del contenido de los archivos indicados en stdin (una ruta por línea).
hash_lista() {
  local f
  while IFS= read -r f; do
    [ -n "$f" ] || continue
    SHA256 < "$f"
  done | SHA256
}

hash_diagrama() { # <fuente.puml>
  printf '%s\n' "$1" "$PLANTILLA/estilo.puml" | hash_lista
}

hash_informe() { # <directorio del informe>
  {
    find "$1" -type f ! -name '.*' ! -name '*.pdf' ! -name '*.docx' \
      ! -name '*.aux' ! -name '*.log' ! -name '*.toc' ! -name '*.out' \
      ! -name '*.lof' ! -name '*.lot' | LC_ALL=C sort
    printf '%s\n' "$PLANTILLA/raysac-informe.cls" "$PLANTILLA/pandoc-macros.tex" \
      "$PLANTILLA/pandoc-filtro.lua" "$PLANTILLA/referencia.docx"
    if [ -d "$PNG_DIR" ]; then
      find "$PNG_DIR" -type f -name '*.png' | LC_ALL=C sort
    fi
  } | hash_lista
}

hash_guardado() { # <clave>
  [ -f "$HASHFILE" ] || return 0
  awk -F'\t' -v k="$1" '$1 !~ /^#/ && $1 == k { print $2; exit }' "$HASHFILE"
}

# ------------------------------------------------------------------ validadores
valida_pdf() {
  [ -s "$1" ] && [ "$(head -c 5 "$1")" = "%PDF-" ] && [ "$(wc -c < "$1")" -gt 2000 ]
}
valida_png() {
  [ -s "$1" ] && [ "$(head -c 4 "$1" | od -An -tx1 | tr -d ' \n')" = "89504e47" ]
}
valida_svg() { [ -s "$1" ] && grep -q '<svg' "$1"; }
valida_docx() {
  [ -s "$1" ] || return 1
  if command -v unzip >/dev/null 2>&1; then
    unzip -tq "$1" >/dev/null 2>&1 && [ -n "$(unzip -Z1 "$1" 'word/document.xml' 2>/dev/null)" ]
  else
    [ "$(head -c 2 "$1")" = "PK" ]
  fi
}

# ------------------------------------------------------------------ estado
TMP="$(mktemp -d "${TMPDIR:-/tmp}/build-docs.XXXXXX")"
trap 'rm -rf "$TMP"' EXIT
NUEVOS_HASHES="$TMP/nuevos-hashes"
: > "$NUEVOS_HASHES"
PROBLEMAS=0
GENERADOS=0
SALTADOS=0

problema() { PROBLEMAS=$((PROBLEMAS + 1)); printf '  %s\n' "$*" >&2; }
registrar() { printf '%s\t%s\n' "$1" "$2" >> "$NUEVOS_HASHES"; }

MOTIVO=""
# Devuelve 0 si la salida está desactualizada (y deja el motivo en MOTIVO).
desactualizado() { # <hash esperado> <clave relativa>...
  local h="$1" k; shift
  for k in "$@"; do
    if [ ! -f "$DOCS_DIR/$k" ]; then MOTIVO="falta $k"; return 0; fi
    local g; g="$(hash_guardado "$k")"
    if [ -z "$g" ]; then MOTIVO="$k sin registro en .build-hashes"; return 0; fi
    if [ "$g" != "$h" ]; then MOTIVO="la fuente cambió desde la última generación de $k"; return 0; fi
  done
  return 1
}

# ------------------------------------------------------------------ diagramas
etapa_diagramas() {
  [ ${#PUML_FILES[@]} -gt 0 ] || { log "[diagramas] sin fuentes en diagramas/fuentes/"; return 0; }
  local gen="$TMP/diagramas"
  local src base h obsoleto
  local todo=()
  for src in "${PUML_FILES[@]}"; do
    base="$(basename "$src" .puml)"
    h="$(hash_diagrama "$src")"
    obsoleto=0
    desactualizado "$h" "diagramas/svg/$base.svg" "diagramas/png/$base.png" && obsoleto=1
    if [ "$obsoleto" -eq 1 ] && [ "$CHECK" -eq 1 ]; then
      problema "DESACTUALIZADO diagramas/fuentes/$base.puml: $MOTIVO"
    fi
    if [ "$obsoleto" -eq 1 ] || [ "$CHECK" -eq 1 ] || [ "$FORCE" -eq 1 ]; then
      todo+=("$src")
    else
      SALTADOS=$((SALTADOS + 1))
      registrar "diagramas/svg/$base.svg" "$h"
      registrar "diagramas/png/$base.png" "$h"
    fi
  done
  if [ ${#todo[@]} -eq 0 ]; then
    log "[diagramas] al día (${#PUML_FILES[@]} diagramas)"
    return 0
  fi

  mkdir -p "$gen/svg" "$gen/png"
  log "[diagramas] generando ${#todo[@]} de ${#PUML_FILES[@]} con $PLANTUML"
  local fmt
  for fmt in svg png; do
    if ! "$PLANTUML" -t"$fmt" -charset UTF-8 -nometadata -o "$gen/$fmt" "${todo[@]}" \
         > "$TMP/plantuml-$fmt.log" 2>&1; then
      cat "$TMP/plantuml-$fmt.log" >&2
      die "PlantUML falló al generar $fmt (revise el .puml, Java y graphviz)"
    fi
    if grep -qiE 'error|syntax' "$TMP/plantuml-$fmt.log"; then
      cat "$TMP/plantuml-$fmt.log" >&2
      die "PlantUML informó errores al generar $fmt"
    fi
  done

  for src in "${todo[@]}"; do
    base="$(basename "$src" .puml)"
    h="$(hash_diagrama "$src")"
    if ! valida_svg "$gen/svg/$base.svg"; then
      die "no se generó un SVG válido para $base.puml (¿@startuml con nombre distinto de '$base'?)"
    fi
    if ! valida_png "$gen/png/$base.png"; then
      die "no se generó un PNG válido para $base.puml (¿@startuml con nombre distinto de '$base'?)"
    fi
    if [ "$CHECK" -eq 0 ]; then
      mkdir -p "$SVG_DIR" "$PNG_DIR"
      cp "$gen/svg/$base.svg" "$SVG_DIR/$base.svg"
      cp "$gen/png/$base.png" "$PNG_DIR/$base.png"
      log "  + diagramas/svg/$base.svg  diagramas/png/$base.png"
    else
      log "  ok (temporal) $base"
    fi
    registrar "diagramas/svg/$base.svg" "$h"
    registrar "diagramas/png/$base.png" "$h"
    GENERADOS=$((GENERADOS + 1))
  done
}

# ------------------------------------------------------------------ informes
compilar_pdf() { # <dir> <slug> <salida.pdf>
  local dir="$1" slug="$2" out="$3" outdir="$TMP/pdf-$2"
  mkdir -p "$outdir"
  # tectonic repite las pasadas necesarias (índices, referencias) y no deja intermedios.
  if ! ( cd "$dir" && tectonic -X compile --outdir "$outdir" "$slug.tex" ) \
       > "$TMP/tectonic-$slug.log" 2>&1; then
    cat "$TMP/tectonic-$slug.log" >&2
    die "tectonic falló al compilar informes/$(basename "$dir")/$slug.tex"
  fi
  grep -E 'Missing character|undefined|Undefined' "$TMP/tectonic-$slug.log" | sort -u >&2 || true
  [ -f "$outdir/$slug.pdf" ] || die "tectonic no produjo $slug.pdf"
  valida_pdf "$outdir/$slug.pdf" || die "$slug.pdf generado no es un PDF válido"
  mv "$outdir/$slug.pdf" "$out"
}

compilar_docx() { # <dir> <slug> <salida.docx>
  local dir="$1" slug="$2" out="$3"
  local rp="$dir:$PNG_DIR:$DOCS_DIR/diagramas"
  if ! ( cd "$dir" && pandoc --from=latex --to=docx --standalone \
           --reference-doc="$PLANTILLA/referencia.docx" \
           --lua-filter="$PLANTILLA/pandoc-filtro.lua" \
           -M lang=es-CL -M toc-title=Índice \
           --resource-path="$rp" --toc --toc-depth=3 \
           -o "$out" "$PLANTILLA/pandoc-macros.tex" "$slug.tex" ) \
       > "$TMP/pandoc-$slug.log" 2>&1; then
    cat "$TMP/pandoc-$slug.log" >&2
    die "pandoc falló al convertir informes/$(basename "$dir")/$slug.tex"
  fi
  if grep -q 'Could not fetch resource' "$TMP/pandoc-$slug.log"; then
    cat "$TMP/pandoc-$slug.log" >&2
    die "pandoc no encontró alguna figura de $slug.tex (revise las rutas en --resource-path)"
  fi
  [ ! -s "$TMP/pandoc-$slug.log" ] || sed 's/^/  pandoc: /' "$TMP/pandoc-$slug.log" >&2
  valida_docx "$out" || die "$slug.docx generado no es un DOCX válido"
}

etapa_informes() {
  [ ${#TEX_FILES[@]} -gt 0 ] || { log "[informes] sin fuentes en informes/*/*.tex"; return 0; }
  local tex dir slug rel h obsoleto gen
  for tex in "${TEX_FILES[@]}"; do
    dir="$(dirname "$tex")"
    slug="$(basename "$tex" .tex)"
    rel="informes/$(basename "$dir")/$slug"
    h="$(hash_informe "$dir")"
    obsoleto=0
    desactualizado "$h" "$rel.pdf" "$rel.docx" && obsoleto=1
    if [ "$obsoleto" -eq 1 ] && [ "$CHECK" -eq 1 ]; then
      problema "DESACTUALIZADO $rel.tex: $MOTIVO"
    fi
    if [ "$obsoleto" -eq 0 ] && [ "$CHECK" -eq 0 ] && [ "$FORCE" -eq 0 ]; then
      log "[informes] $rel al día"
      SALTADOS=$((SALTADOS + 1))
      registrar "$rel.pdf" "$h"
      registrar "$rel.docx" "$h"
      continue
    fi
    gen="$TMP/informes/$slug"
    mkdir -p "$gen"
    log "[informes] compilando $rel.tex"
    compilar_pdf  "$dir" "$slug" "$gen/$slug.pdf"
    compilar_docx "$dir" "$slug" "$gen/$slug.docx"
    if [ "$CHECK" -eq 0 ]; then
      cp "$gen/$slug.pdf"  "$dir/$slug.pdf"
      cp "$gen/$slug.docx" "$dir/$slug.docx"
      log "  + $rel.pdf  $rel.docx"
    else
      log "  ok (temporal) $rel"
    fi
    registrar "$rel.pdf" "$h"
    registrar "$rel.docx" "$h"
    GENERADOS=$((GENERADOS + 1))
  done
}

# ------------------------------------------------------------------ huérfanos
etapa_huerfanos() {
  local f base
  for f in "$SVG_DIR"/*.svg; do
    [ -e "$f" ] || continue
    base="$(basename "$f" .svg)"
    if [ ! -f "$FUENTES/$base.puml" ]; then
      if [ "$CHECK" -eq 1 ]; then problema "HUÉRFANO diagramas/svg/$base.svg sin fuente .puml"
      else warn "diagramas/svg/$base.svg no tiene fuente .puml"; fi
    fi
  done
  for f in "$PNG_DIR"/*.png; do
    [ -e "$f" ] || continue
    base="$(basename "$f" .png)"
    if [ ! -f "$FUENTES/$base.puml" ]; then
      if [ "$CHECK" -eq 1 ]; then problema "HUÉRFANO diagramas/png/$base.png sin fuente .puml"
      else warn "diagramas/png/$base.png no tiene fuente .puml"; fi
    fi
  done
  for f in "$INFORMES"/*/*.pdf "$INFORMES"/*/*.docx; do
    [ -e "$f" ] || continue
    base="${f%.*}"
    if [ ! -f "$base.tex" ]; then
      if [ "$CHECK" -eq 1 ]; then problema "HUÉRFANO ${f#"$DOCS_DIR"/} sin fuente .tex"
      else warn "${f#"$DOCS_DIR"/} no tiene fuente .tex"; fi
    fi
  done
}

# ------------------------------------------------------------------ principal
comprobar_herramientas

if [ "$CHECK" -eq 1 ]; then log "== build-docs.sh --check ($DOCS_DIR) =="; else log "== build-docs.sh ($DOCS_DIR) =="; fi

etapa_diagramas
etapa_informes
etapa_huerfanos

if [ "$CHECK" -eq 1 ]; then
  if [ "$PROBLEMAS" -gt 0 ]; then
    printf '\nFALLO: %d problema(s). Ejecute docs/build-docs.sh y versione los archivos generados.\n' "$PROBLEMAS" >&2
    exit 1
  fi
  log "OK: toda la documentación generada está al día y se regenera sin errores."
else
  { printf '# Generado por build-docs.sh; hash (sha256) de las fuentes de cada salida. No editar.\n'
    LC_ALL=C sort "$NUEVOS_HASHES"; } > "$HASHFILE"
  log "Listo: $GENERADOS generado(s), $SALTADOS al día. Hashes en ${HASHFILE#"$DOCS_DIR"/}"
fi
