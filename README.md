# Fontabulous Studio

A font editor: draw and edit glyphs, set spacing and kerning, and work with
masters, then save as TrueType or UFO. Written in Rust with
[egui](https://github.com/emilk/egui), with [Lucide](https://lucide.dev)
icons and the look of the ArtCraft apps
([VectorCraft](https://github.com/deepfold9118/vectorcraft),
[PhotoCraft](https://github.com/deepfold9118/photocraft)). It is the
companion to the [Fontabulous](https://github.com/deepfold9118/fontabulous)
font manager.

Windows only for now; Linux is planned.

## Download

Get `Fontabulous-Studio-Setup-<version>.exe` from the
[latest release](https://github.com/deepfold9118/fontabulous-studio/releases/latest).
It installs for the current user (no administrator rights needed), adds a
Start menu entry and a desktop shortcut, and appears in Windows' Installed
apps, where it can be uninstalled. The release also has the bare
`fontabulous-studio.exe`, which runs without installing.

## What it does

- **Open** TrueType and OpenType fonts (`.ttf`, `.otf`, `.ttc`, `.otc`),
  installed fonts, UFO font folders and designspace files.
- **Save** as TrueType (one master, or all masters), UFO, or the app's own
  `.fstudio` format. Import and export kerning, feature code and classes.
- **Font window**: a filterable glyph grid (by name, Unicode, category and
  script) with flags, and faint template letters in empty glyphs. Create,
  build and rename glyphs, generate names from Unicode and Unicode from
  names, detect composites, and copy the template outlines into empty
  glyphs as a starting point.
- **Glyph window**: 15 drawing tools (pen, nodes, shapes, knife, measure
  and more), components and anchors, guides, zones, grid and snapping, and
  contour commands (smooth, break, join, reverse, simplify, nodes at
  extremes, convert to cubic, round to integer).
- **Spacing and kerning** in text views: metrics and kerning tabs, link
  glyph metrics, center glyphs, equalize sidebearings, and adjust or remove
  pairs.
- **Masters**: add variations, merge fonts into masters, match masters and
  generate instances. Compare and overlay fonts.
- **Font Info**, **Preferences**, about 30 side panels, Actions, FontAudit
  and a list of every command with its shortcut. Light and dark themes.

Marked "coming soon" in this version: OpenType PS (`.otf`) and web font
export, Remove Overlap, compiling OpenType features, hinting, and the
brush, stroke, image, scripting, chat and Superkerning panels.

Preferences are kept in `%APPDATA%\fontabulous_studio\prefs.json`.

## AI agents (MCP)

Fontabulous Studio is also a [Model Context Protocol](https://modelcontextprotocol.io)
server, so an AI agent can do anything a person can: run any of the menu
commands, edit outlines, spacing, kerning, font info and preferences, and
take screenshots. Every change shows live in the window, appears in the
Output panel and can be undone.

- **stdio**: start the program with `--mcp`; the agent's client launches it
  and talks over stdin/stdout while the window stays open.
- **HTTP**: Preferences ▸ Agents turns on a local-only endpoint
  (`http://127.0.0.1:7457/mcp` by default; the port can be changed there)
  so an agent can attach to the window you are already using. It is off by
  default.

## Building

Needs Rust and the Visual Studio C++ build tools.

```powershell
cargo build --release
```

`build-installer.ps1` builds the program and the installer (`installer\`,
a small egui program that embeds the release build) with the build folders
stripped from them, and writes both to `dist\` with SHA-256 files.

## Licence

Apache-2.0. See [LICENSE](LICENSE).

The interface uses the [Inter](https://rsms.me/inter/) typeface (SIL Open
Font License 1.1, [assets/OFL-Inter.txt](assets/OFL-Inter.txt)) and the
[Lucide](https://lucide.dev) icon font (ISC,
[assets/LICENSE-lucide.txt](assets/LICENSE-lucide.txt)).
