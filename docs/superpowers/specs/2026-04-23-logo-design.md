# Agent Orchestrator Logo Design Spec

## Overview

Replace the default Tauri yin-yang logo with a custom logo that represents Agent Orchestrator's core function: managing multiple parallel AI agent sessions.

## Design: Parallel Streams

Three pill-shaped vertical bars of distinct heights with floating circular status indicators above each bar. The bars represent agent sessions running in parallel at different stages of progress.

### Concept

- **Left bar (short)**: An agent that recently started or is near completion
- **Center bar (tall)**: An agent deep into a long-running task
- **Right bar (medium)**: An agent at a mid-point of execution
- **Dots above bars**: Active status indicators, echoing the app's real-time status system

The staggered heights immediately communicate parallelism and varying progress — the core experience of using Agent Orchestrator.

### SVG Specification

Viewbox: `0 0 140 140`

Elements (all use `currentColor` or a single fill color):

| Element | x | y | width | height | rx | type |
|---------|---|---|-------|--------|-----|------|
| Left bar | 25 | 80 | 18 | 40 | 9 | rect |
| Left dot | cx=34 | cy=68 | r=5 | — | — | circle |
| Center bar | 61 | 18 | 18 | 102 | 9 | rect |
| Center dot | cx=70 | cy=6 | r=5 | — | — | circle |
| Right bar | 97 | 52 | 18 | 68 | 9 | rect |
| Right dot | cx=106 | cy=40 | r=5 | — | — | circle |

### SVG Source

```svg
<svg width="140" height="140" viewBox="0 0 140 140" fill="none" xmlns="http://www.w3.org/2000/svg">
  <rect x="25" y="80" width="18" height="40" rx="9" fill="currentColor"/>
  <circle cx="34" cy="68" r="5" fill="currentColor"/>
  <rect x="61" y="18" width="18" height="102" rx="9" fill="currentColor"/>
  <circle cx="70" cy="6" r="5" fill="currentColor"/>
  <rect x="97" y="52" width="18" height="68" rx="9" fill="currentColor"/>
  <circle cx="106" cy="40" r="5" fill="currentColor"/>
</svg>
```

### Color Usage

- **Monochrome**: Single color that adapts to context
- **On dark backgrounds**: White (`#ffffff`)
- **On light backgrounds**: Near-black (`#111111`)
- Uses `currentColor` in SVG for automatic theme adaptation

### Size Guidelines

The logo is tested and readable at these sizes:

| Context | Size | Notes |
|---------|------|-------|
| App icon / splash | 120px+ | Full detail visible |
| Sidebar / header | 40-64px | Bars and dots clearly distinct |
| Favicon | 32px | Still readable |
| Tab / tiny icon | 16px | Minimal but recognizable |

### Lockup (Logo + Wordmark)

When paired with the app name:
- Logo at 40px height
- "Agent Orchestrator" in system sans-serif, 600 weight, -0.5px letter-spacing
- 16px gap between logo and text
- Vertically centered alignment

## Implementation Scope

1. Create SVG logo file(s) in the project assets
2. Replace Tauri app icon (macOS .icns generation)
3. Update any in-app references to the old logo
4. Update favicon if applicable

## Files to Modify

- `src-tauri/icons/` — Tauri app icons (various sizes, .icns, .ico, .png)
- `src/assets/` — Any in-app logo references
- `index.html` — Favicon if present
- `src-tauri/tauri.conf.json` — Icon configuration if needed
