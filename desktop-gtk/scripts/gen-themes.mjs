// gen-themes.mjs — port web UI theme presets into Go for the desktop app.
//
// Usage (from repo root):  node desktop-gtk/scripts/gen-themes.mjs
//
// Reads web/src/data/themes.ts (flat array of {name, label, colors{17 hex
// keys}}) and emits desktop-gtk/internal/ui/themes_gen.go. The generated file
// is committed; rerun this script only when the web presets change.
import { readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const root = join(here, "..", "..");
const srcPath = join(root, "web", "src", "data", "themes.ts");
const outPath = join(root, "desktop-gtk", "internal", "ui", "themes_gen.go");

const src = readFileSync(srcPath, "utf8");

const KEYS = [
  ["background", "Background"],
  ["foreground", "Foreground"],
  ["card", "Card"],
  ["cardForeground", "CardForeground"],
  ["primary", "Primary"],
  ["primaryForeground", "PrimaryForeground"],
  ["secondary", "Secondary"],
  ["secondaryForeground", "SecondaryForeground"],
  ["muted", "Muted"],
  ["mutedForeground", "MutedForeground"],
  ["accent", "Accent"],
  ["accentForeground", "AccentForeground"],
  ["destructive", "Destructive"],
  ["destructiveForeground", "DestructiveForeground"],
  ["border", "Border"],
  ["input", "Input"],
  ["ring", "Ring"],
];

const entryRe =
  /\{\s*name:\s*"([^"]+)",\s*label:\s*"([^"]+)",\s*colors:\s*\{([^}]*)\}\s*\}/g;

const themes = [];
let m;
while ((m = entryRe.exec(src)) !== null) {
  const [, name, label, body] = m;
  const vals = {};
  for (const [key, field] of KEYS) {
    const km = body.match(new RegExp(`${key}:\\s*"(#[0-9a-fA-F]{6})"`));
    if (!km) {
      console.error(`error: key "${key}" missing or not plain hex in theme "${name}"`);
      process.exit(1);
    }
    vals[field] = km[1];
  }
  themes.push({ name, label, vals });
}

if (themes.length === 0) {
  console.error("error: no themes parsed from", srcPath);
  process.exit(1);
}

let go = `// Code generated from web/src/data/themes.ts by scripts/gen-themes.mjs.
// DO NOT EDIT by hand — rerun the script instead.

package ui

// ThemeColors holds the 17 color tokens shared by every web preset.
type ThemeColors struct {
	Background            string
	Foreground            string
	Card                  string
	CardForeground        string
	Primary               string
	PrimaryForeground     string
	Secondary             string
	SecondaryForeground   string
	Muted                 string
	MutedForeground       string
	Accent                string
	AccentForeground      string
	Destructive           string
	DestructiveForeground string
	Border                string
	Input                 string
	Ring                  string
}

// ThemePreset is one color scheme ported from the web UI.
type ThemePreset struct {
	Name   string // machine id ("dracula-dark"), matches the web preset name
	Label  string // human display name ("Dracula")
	Colors ThemeColors
}

// DefaultThemeName is used when nothing (or an unknown name) is configured.
const DefaultThemeName = ${JSON.stringify(themes[0].name)}

// Themes lists every preset in the same order as the web UI.
var Themes = []ThemePreset{
`;

for (const t of themes) {
  go += `\t{Name: ${JSON.stringify(t.name)}, Label: ${JSON.stringify(t.label)}, Colors: ThemeColors{\n`;
  for (const [, field] of KEYS) {
    go += `\t\t${field}: ${JSON.stringify(t.vals[field])},\n`;
  }
  go += "\t}},\n";
}

go += `}\n`;

writeFileSync(outPath, go);
console.log(`wrote ${outPath} with ${themes.length} themes`);
