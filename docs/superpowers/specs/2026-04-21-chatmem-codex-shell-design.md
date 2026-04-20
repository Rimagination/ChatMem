# ChatMem Codex-Style Shell Design

## Goal

Redesign the ChatMem application shell so it feels closer to the Codex App window chrome and sidebar behavior, while still using the task-oriented information architecture already approved for the new ChatMem UI.

This spec focuses on:

- window chrome and top bar behavior
- left sidebar structure and styling
- project collapse/restore behavior
- shared organize/filter/sort menu behavior
- typography and localization defaults

This spec **supplements and partially overrides** the shell portions of:

- `docs/superpowers/specs/2026-04-21-chatmem-task-nav-help-design.md`

That earlier spec still governs the page model and task destinations:

- Continue Work
- Needs Review
- History
- Help

This document replaces the earlier assumptions about the outer shell, window chrome, and sidebar presentation.

## Product Intent

The new shell should feel:

- lighter
- flatter
- closer to a native Codex-style desktop surface
- less like a floating dashboard card

The result should not read as a branded landing page. It should read as a work tool.

## Approved Constraints

The user-approved shell constraints are:

1. The main task navigation remains at the top
2. The top bar is a **single row**
3. The shell should feel nearly borderless
4. The logo should not appear as a large branded element inside the app shell
5. The left side should blend into the background instead of looking like a rounded card
6. Only the right content area should retain clear rounded framing
7. The left rail should stay in a two-section structure:
   - Projects
   - Chats
8. Agent tabs (`Claude / Codex / Gemini`) stay visible
9. Default UI language is Chinese
10. Settings must allow language switching
11. Typography should match Codex App as closely as practical

## Shell Architecture

## 1. Single-Row Top Bar

The entire app should use one top row for shell navigation and window framing.

### Left side

- small app icon
- `ChatMem v0.1.5` style version label

Notes:

- the version must come from application/package version metadata
- do not place a large brand mark, hero logo, or oversized app badge here
- this label is a utility label, not a marketing lockup

### Center

Task navigation:

- `继续工作`
- `待确认`
- `历史`
- `帮助`

Notes:

- keep the labels short
- make this the main visible navigation in the shell
- this replaces the idea of a second top row or separate page-tab strip

### Right side

- only the three window controls

Notes:

- do not add branded controls or extra toolbar clusters here
- if utility controls such as search or settings need to exist, they should live in the main UI body, not in the window-control area

## 2. Window Chrome Style

The chrome should feel close to borderless.

That means:

- no large top banner
- no internal logo card
- no decorative rounded panel surrounding the whole shell
- no visual separation that makes the left rail feel like a floating card detached from the app canvas

The top row should feel like part of the window, not a card placed inside the window.

## Left Sidebar

## 1. Visual Treatment

The left sidebar should merge into the app background using a light surface.

Requirements:

- same pale background family as the canvas
- no rounded-rectangle outer card for the entire sidebar
- no heavy drop shadow around the sidebar
- no framed, floating dashboard look

The sidebar may still use local pills, toggles, and row highlights, but the rail itself should not look like a separate card.

## 2. Corner Strategy

- left sidebar: no large outer radius
- right content pane: keep rounded corners

This creates the Codex-like hierarchy:

- left side feels structural
- right side feels like the active work surface

## 3. Section Structure

Keep the left rail organized as:

1. agent tabs
2. search
3. `项目`
4. `聊天`
5. `设置`

The user explicitly wants to preserve the current two-section layout for the list area:

- Projects section
- Chats section

Do not collapse those into a single mixed list.

## 4. Divider Rules

There should be **no horizontal divider** between the conversation area and the settings row.

That means:

- remove the line currently separating the scrolling list area from the bottom settings entry
- settings should feel like the last structural row in the rail, not like a separate footer card

## Projects Section

## 1. Section Header Controls

The `项目` header should include two controls modeled after Codex-style interactions:

### Control A: Collapse/restore icon

Purpose:

- first click: collapse all expanded project groups
- second click: restore the previous expansion state

Scope:

- affects only the `项目` section
- must not affect the `聊天` section

State model:

- when all projects are visible in a mixed expansion state, clicking collapse stores the previous expansion map
- while in collapsed-all mode, clicking again restores that stored expansion map
- if no previous expansion map exists, restore should reopen the last meaningful state or fall back to the default expansion state

### Control B: Organize icon

Purpose:

- open a shared organize/filter/sort menu

Scope:

- applies to both `项目` and `聊天`
- same menu, same active rule set

The interaction style should feel close to Codex App, but the content should be customized for ChatMem.

## 2. Organize Menu

The menu should use a compact floating panel style similar to the Codex interaction pattern, but with ChatMem-specific menu content.

### Menu sections

#### Section 1: Arrangement mode

Recommended items:

- `按项目`
- `时间顺序列表`
- `聊天优先`

Behavior:

- one active selection at a time
- affects both Projects and Chats sections
- defaults should preserve the existing two-section rail while changing ranking/placement logic within that structure

Recommended default:

- `按项目`

#### Section 2: Sort order

Recommended items:

- `最近更新`
- `最近创建`

Behavior:

- one active selection at a time
- applies to both Projects and Chats

Approved default:

- `最近更新`

#### Section 3: Filters

Approved first-pass filter dimensions:

- `项目`
- `标签`
- `状态`

Behavior:

- filters are shared between Projects and Chats
- a filter state should remain visible after the menu closes
- active filtering should not silently hide the fact that the list is filtered

Recommended follow-up UI:

- small summary chip or text cue near the organize control when filters are active

## Chats Section

The Chats section stays as its own block below Projects.

Requirements:

- shares the same organize/sort/filter state as Projects
- must not be affected by the Projects-only collapse/restore control
- should visually align with Projects rather than appearing as a separate boxed region

## Typography and Language

## 1. Default Language

The shell should default to Simplified Chinese.

That includes:

- top navigation labels
- sidebar section titles
- menu labels
- status labels
- settings labels

English remains available in Settings.

## 2. Font Rules

Typography should match Codex App as closely as practical.

Practical implementation rules:

- use the same system UI font strategy as Codex App
- do not introduce a branded display font
- do not mix a custom Chinese font with a different Latin UI font unless Codex itself does so
- use one consistent UI sans stack for both Chinese and Latin interface copy

On Windows, this likely means a Segoe-style system UI stack. The exact stack should be chosen by checking the Codex App shell and matching its effective UI font rather than inventing a new one.

## 3. Version Label Typography

`ChatMem vX.Y.Z` should be small and calm:

- not bold branding
- not oversized
- not treated as a page heading

## Visual Hierarchy

## 1. Left vs Right Surfaces

The shell should make this hierarchy obvious:

- left rail = library/navigation structure
- right pane = active work surface

### Left rail

- low-contrast structural background
- minimal framing
- highlight selected rows and active controls only

### Right pane

- clear rounded work surface
- primary reading and interaction area
- stronger contrast than the left rail

## 2. Agent Tabs

Keep `Claude / Codex / Gemini` as visible controls in the left rail.

Requirements:

- compact segmented control style
- should not visually overpower the task navigation in the top bar
- should read as a scoped filter/context switch, not as the primary app navigation

## 3. Search Box

Search remains in the left rail.

Requirements:

- no heavy card framing around it
- use a calm inline field style
- keep it visually aligned with the lighter Codex-like shell

## Low-Fidelity Wireframe

```text
+--------------------------------------------------------------------------------------+
| [app icon] ChatMem v0.1.5     继续工作  待确认  历史  帮助                     _ [] X |
+--------------------------------------------------------------------------------------+
| Claude / Codex / Gemini                                                         |
| [ 搜索对话...                          ]                                         |
|                                                                                  |
| 项目                                               [collapse] [organize]         |
|   > VSP                                                                            |
|   > PV                                                                             |
|                                                                                  |
| 聊天                                                                              |
|   最近会话 1                                                                       |
|   最近会话 2                                                                       |
|                                                                                  |
| 设置                                                                              |
+--------------------------------------+-------------------------------------------+
|                                      |                                           |
|  left rail blends into background    |   rounded active content surface          |
|                                      |   Continue Work / Needs Review /          |
|                                      |   History / Help content                  |
|                                      |                                           |
+--------------------------------------+-------------------------------------------+
```

## Implementation Notes

When this shell is implemented:

1. The current task-oriented page model remains valid
2. The earlier task-nav-help spec should still govern page destinations and Help content
3. The implementation plan must be revised before execution so it does not target the old shell assumptions

## Non-Goals

This shell redesign does **not** yet define:

- the Zotero-style library information model
- WebDAV/cloud sync architecture
- the future ChatMem skill

Those remain separate tracks.

## Success Criteria

This shell redesign succeeds if:

- the app no longer looks like a floating rounded dashboard card
- the top bar feels like a single Codex-style working header
- the left rail feels structural and light
- the right pane is the only clearly framed work surface
- Projects gain collapse/restore and organize controls with the approved scope
- organize/sort/filter behavior applies to both Projects and Chats
- the UI defaults to Chinese and keeps a Codex-like font feel
