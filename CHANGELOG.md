# Changelog

All notable changes to Fontabulous Studio are documented here. The format
follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the
project uses [Semantic Versioning](https://semver.org/).

## [Unreleased]

## [0.1.0] - 2026-11-22

### Added
- The first release: a font editor with the ArtCraft look, Lucide icons and
  light and dark themes, companion to the Fontabulous font manager.
- Open TrueType/OpenType fonts (`.ttf`, `.otf`, `.ttc`, `.otc`), installed
  fonts, UFO font folders and designspace files. Save as TrueType (one or
  all masters), UFO, or the native `.fstudio` format. Import and export
  kerning, feature code and classes.
- Font window: a filterable glyph grid with flags and template letters in
  empty glyphs; create, build and rename glyphs, names from Unicode and
  Unicode from names, detect composites, copy template outlines.
- Glyph window: 15 drawing tools (pen, nodes, shapes, knife, measure and
  more), components, anchors, guides, zones, grid, snapping, and contour
  commands.
- Metrics and kerning tabs; link glyph metrics, center glyphs, equalize
  sidebearings, adjust and remove pairs.
- Masters: add variations, merge fonts to masters, match masters, generate
  instances; compare and overlay fonts.
- Font Info, Preferences, about 30 side panels, Actions, FontAudit, and a
  list of every command with its shortcut.
- MCP server for AI agents, over stdio (`--mcp`) or a local-only HTTP
  endpoint (Preferences ▸ Agents, off by default). Agent changes show live
  and can be undone.
- An installer for the current user (no administrator rights needed) with
  Start menu and desktop shortcuts and an Installed apps entry.

### Not yet available
- OpenType PS (`.otf`) and web font export, Remove Overlap, compiling
  OpenType features, hinting, and the brush, stroke, image, scripting, chat
  and Superkerning panels are marked "coming soon".

[Unreleased]: https://github.com/deepfold9118/fontabulous-studio/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/deepfold9118/fontabulous-studio/releases/tag/v0.1.0
