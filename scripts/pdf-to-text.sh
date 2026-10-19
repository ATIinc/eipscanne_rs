#!/usr/bin/env bash
# Extracts the text layer of a PDF into a plain text file, one "=====PAGE n=====" marker per page,
# so the result can be searched with grep and read in page-sized pieces.
#
# Works for PDFs whose text is drawn with embedded (CID) glyph fonts: poppler maps the glyphs back
# to characters through the fonts' ToUnicode tables. Scanned PDFs only give text if they carry an
# OCR layer, and OCR mistakes (e.g. "0xAl" for "0xA1") are kept as-is.
#
# Requires poppler-utils (pdftotext, pdfinfo), installed by the devcontainer.
#
# Usage: scripts/pdf-to-text.sh <input.pdf> [output.txt]
#        output defaults to the input path with a .txt extension

set -euo pipefail

if [[ $# -lt 1 || $# -gt 2 ]]; then
    echo "Usage: $0 <input.pdf> [output.txt]" >&2
    exit 1
fi

input="$1"
output="${2:-${input%.*}.txt}"

for tool in pdftotext pdfinfo; do
    if ! command -v "$tool" >/dev/null; then
        echo "$tool not found: install poppler-utils" >&2
        exit 1
    fi
done

pages=$(pdfinfo "$input" | awk '/^Pages:/ { print $2 }')

: >"$output"
for ((page = 1; page <= pages; page++)); do
    printf '\n=====PAGE %d=====\n' "$page" >>"$output"
    pdftotext -layout -enc UTF-8 -f "$page" -l "$page" "$input" - >>"$output"
done

echo "Wrote $pages pages to $output"
