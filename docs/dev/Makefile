# Render Mermaid sources to SVG/PNG using mermaid-cli (Node required).
# Usage:
#   make svgs
#   make pngs

MMD_SRCS := $(wildcard img/*.mmd)
SVG_OUTS := $(MMD_SRCS:.mmd=.svg)
PNG_OUTS := $(MMD_SRCS:.mmd=.png)

svgs: $(SVG_OUTS)
pngs: $(PNG_OUTS)

img/%.svg: img/%.mmd
	@npx -y @mermaid-js/mermaid-cli -i $< -o $@ --scale 1.2

img/%.png: img/%.mmd
	@npx -y @mermaid-js/mermaid-cli -i $< -o $@ --png --scale 1.5

.PHONY: svgs pngs
