# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

### Changed

### Fixed

### Removed


## [0.5.2] - 2026-08-14

### Added
- A New Chat button in the sidebar starts a fresh conversation, with the
  greeting and input box centered on screen until you send the first message.
  Clicking it again reopens the blank chat instead of piling up empty ones.
- Agents can stop and ask you a question mid-task — single choice, multiple
  choice, or free text — in a card above the input box, then carry on with your
  answer instead of guessing.
- Hooks reach two more moments: when the agent finishes a reply, and when a tool
  call stops to ask for your approval. A finish-reply hook can send the agent
  back to work with a reason — "you said you'd run the tests" — while an
  approval hook only reports, leaving the decision to you.
- The window reopens at the size and position you last left it.

### Changed
- Settings navigation is regrouped into General and Agents, and Quick Tools now
  sits under General.
- Long conversations open immediately instead of freezing while the whole
  history builds; older messages fill in behind you.

### Fixed
- Going back to an older version of the app no longer leaves it refusing to
  start.
- Returning from Settings keeps the session you had open, at the scroll position
  you left it — and scrolling up mid-reply no longer yanks you back down on
  every new word.


## [0.5.1] - 2026-08-10 [NOT RELEASED]

Tagged but never published — the build failed at notarization before any
artifact was uploaded. These changes ship in 0.5.2 above.


## [0.5.0] - 2026-08-07

### Added
- Hooks: Settings → Hooks lets you run your own command when the agent submits
  a prompt or calls a tool. A hook can hand extra context to the model, rewrite
  what a tool is about to do, block the call outright, or simply do its job
  quietly — like formatting a file right after it's written.
- Every hook that fires now shows up in the conversation beside the tool call it
  belongs to, and command hooks expand to reveal what the command printed.
- Settings → Session decides when a session gets an automatic title: after the
  first message, after every message, or never. Titles generated later read the
  whole conversation, and sessions you renamed by hand are left alone.
- Copy an assistant reply to the clipboard with one click.
- The sidebar can wear the macOS frosted-glass look — on by default, switchable
  in Settings → General.

### Changed
- Reasoning stays out of the way: thinking blocks start collapsed, and long
  reasoning is clipped behind a "show more" toggle once opened.
- Token counts moved off the permanent line under every reply into a tooltip on
  a small chart icon.
- The update dialog is wider, with more room to read release notes.

### Fixed
- Switching the theme mid-session no longer leaves stray text in the previous
  theme's colors, such as white labels on a now-light dialog.
- The message column lines up with the input box instead of running slightly
  wider and off-center.
- Help tooltips inside dialogs appeared away from the icon they belong to.


## [0.4.4] - 2026-07-30

### Added
- The update dialog now shows what's new in the version it offers, with
  formatted release notes and links that open in your browser.

### Changed
- Focus feels native again: clicking into a text field shows just the caret
  instead of a colored border and glow, while keyboard navigation still rings
  the control it lands on.
- Text selection is limited to content — messages, code and release notes stay
  selectable, while sidebar labels, buttons and other app chrome no longer
  highlight as you drag across them.

### Fixed
- Right-clicking in the sidebar no longer opens the system menu with Look Up /
  Translate / Inspect Element.
- The update dialog no longer opens with a stray ring around one of its buttons.
- The changelog link in Settings → About opened a missing page.


## [0.4.3] - 2026-07-29

### Added
- Agents can now show interactive HTML cards right inside the conversation.
- Agents can build small apps that open in a side panel, with a live preview
  and a code view.
- Settings → Agent Tools now lets you toggle these UI abilities and on-demand
  skill loading for new sessions.


## [0.4.2] - 2026-07-28

### Added
- Agents can now search the web with a built-in Web Search tool. Choose the
  search provider and add your API key in Settings → Agent Tools.

### Changed
- A more polished, consistent interface: buttons and dropdown menus now share
  one refined design across the app, and the brand color has shifted to a
  deeper blue.

### Fixed
- Menus and pop-ups no longer show mismatched colors when the app theme
  differs from the system theme.
- Dropdown menus now open aligned with the control that triggered them.
- The red used for destructive actions is now easier to read in light mode.


## [0.4.1] - 2026-07-22

### Added
- Sessions get automatic titles generated from your first exchange, and you can
  right-click a session to regenerate one on demand — your own manual names are
  always kept.
- Give each agent its own icon; it now appears in the sidebar, the agents list,
  and the composer's agent picker.

### Changed
- The sidebar is now an Agent → Project → Session tree with cleaner alignment
  and instant switching; sessions without an agent are grouped under "Chats."
- Creating or editing an agent or a scheduled job now opens a full page instead
  of a pop-up modal, including a live preview of a schedule's next runs.
- The composer's reasoning-effort picker is now a tidy popup menu with an icon
  and a short description for each level.
- Refreshed the app icon.

### Fixed
- An agent's linked Generative UI template and skills now actually take effect
  while it replies.
- The window no longer shows a dark border when the app runs in light mode on a
  dark system.
- Switching between sessions feels instant — pages no longer blank to a spinner
  and already-loaded conversations aren't re-fetched.


## [0.4.0] - 2026-07-11

### Added
- Choose the model for an agent session right in the composer, independent of
  the agent's saved default.
- Pick which agent to talk to from the composer toolbar, and switch agents
  without leaving an empty session behind.
- Set the working folder for an agent session from the composer.
- Bind specific MCP servers to a session so their tools are available in the
  conversation; tools that run actions on your behalf now ask for approval
  first.
- Quick Action sends using the default model you set in Settings → Shortcuts.

### Changed
- Chat and Agents are now one experience: every conversation — and every
  scheduled prompt job — runs through the agent engine. Rich Generative UI
  replies are unchanged.
- Settings now open inside the main window instead of a separate window.
- A refreshed, more consistent interface across Agents, Jobs and Settings —
  cleaner page headers, calmer use of color, standardized form controls, and
  smoother collapse and hover animations.
- Faster, flicker-free startup: the launch screen no longer flashes from black
  to white, and pages paint their real content on the first frame instead of
  showing default values that then jump to the loaded ones.

### Fixed
- The agent session header no longer overlaps the window controls when the
  sidebar is hidden.
- Prompt-based scheduled jobs can pick their model again.

### Removed
- The standalone Chat feature has been removed — start an Agent session instead.
- The Vocabulary feature has been removed.
- The agent form no longer exposes the top-p and top-k sampling sliders.


## [0.3.1] - 2026-06-25

### Added
- Quick Action: a global hotkey opens a floating overlay from any app where you
  can pick an agent, type a request, and get a streaming reply without switching
  to HandBox.
- Continue any Quick Action conversation in the main window with one click.
- A new Settings → Shortcuts page to record the Quick Action hotkey and choose
  the default model it uses.


## [0.3.0] - 2026-06-25

### Added
- Generative UI: assistants and agents can now reply with rich, interactive
  cards — titles, key-value lists, tables and more — that build up live as the
  response streams in, instead of plain text.
- A GenUI template workspace (from the Agents page) where you can create and
  name your own response layouts, preview them live, and start from built-in
  example templates.
- Link a GenUI template to an agent so its replies follow that layout.

### Removed
- The Artifacts feature has been removed, including the Artifacts page and the
  ability to schedule an artifact as a job.
- The Favorites feature has been removed.
- Global search has been removed.


## [0.2.6] - 2026-06-21

### Changed
- Refreshed the app icon with new artwork.
- The launch splash screen is simpler — just the HandBox wordmark and a loading
  indicator; the logo image and tagline are gone.


## [0.2.5] - 2026-06-19

### Changed
- The text-selection floating menu is now a cleaner row of flat Copy /
  Translate / Ask buttons without colored backgrounds; the redundant Show
  button is gone.

### Fixed
- The interface no longer briefly flickers between Chinese and English when you
  bring the app to the foreground.


## [0.2.4] - 2026-06-18

### Added
- Switch the interface language between Chinese and English at any time — the
  change applies instantly across the whole app, with no restart.
- Agent sessions have a new "Open in…" button in the header that opens the
  session's working folder in your editor, terminal, or Finder, and remembers
  the app you choose.

### Changed
- Cleaner message input: focusing the Chat or Agent composer no longer shows a
  colored outline, and the divider above the Agent input is gone.
- The slash-command menu supports Ctrl/Cmd+N/P to move through items and keeps
  the highlighted one in view; the skill you pick now appears inline in the
  input as `/name` instead of a chip below it.

### Removed
- The unused Wordbook and Components pages have been removed from Settings.


## [0.2.3] - 2026-06-17

### Added
- Scheduled Jobs: a new Jobs workspace where you can schedule a saved chat,
  agent, or prompt to run automatically on a recurring schedule.
- A schedule editor with quick presets and an advanced cron mode that shows
  the timing in plain language.
- Each job keeps a timeline of its past runs, and you can trigger any job
  immediately with Run now.
- Jobs automatically retry failed runs with a back-off delay, time out runs
  that take too long, and notify you when a job keeps failing.


## [0.2.2] - 2026-06-17

### Added
- Automatic update checks: HandBox now checks for a new version on launch and
  shows a dialog to download and install it. You can turn auto-checking off or
  check manually from the About page.
- A branded splash screen while the app launches.

### Fixed
- Provider logos are now legible in dark theme.


## [0.2.1] - 2026-06-16

### Added
- The agent can now create and edit files and run shell commands, in addition
  to reading, listing, and searching — a full coding toolset.
- Tool approval: before the agent writes a file or runs a command, HandBox pauses
  and asks you to allow it once, always allow it for the session, or deny it.
- Settings → Agent Tools: choose which tools new agent sessions start with.

### Changed
- The agent's file and command tools are now confined to each session's working
  directory.
- The agent composer's per-tool toggles are collapsed into a single tools icon
  with a popover.

### Removed
- The agent's web-page fetch tool.


## [0.2.0] - 2026-06-12

### Added
- Agent mode: a new Agent workspace alongside Chat (toggle in the sidebar) for
  multi-turn agent runs with streaming replies, thinking blocks, and token usage.
- Agent tools — the agent can read files, list directories, and fetch web pages,
  sandboxed to a per-session working directory (web fetches are SSRF-guarded).
- Image attachments in agent messages.
- Mid-run steering: send extra guidance to an agent while it's still running.
- Per-session system prompt editor, opened from the session header.
- Agent sessions grouped by project in the sidebar, with project create / rename /
  delete and zero-dialog session creation from the project header.
- Skills: a slash-command popover in the agent composer, a Settings → Skills page,
  and per-session skill enablement.
- Custom providers: manual model entry and custom-endpoint chat for providers that
  aren't in the catalog.
- Favorites: collapsible tag filter that detects overflow.

### Changed
- Provider catalog is now sourced live from the models.dev catalog with runtime
  refresh, fixing stale / drifting OpenRouter and Cerebras model lists; provider
  icons also come from models.dev instead of bundled assets.
- Reworked the new-chat flow and made session title generation reliable.
- Moved catalog sync off the startup critical path for faster launch.

### Fixed
- API keys are now redacted from debug and log output.

## [0.1.3] - 2026-05-24

### Added
- Release v0.1.3

### Fixed
- Load `llm_config.json` from the Tauri resource directory instead of the current working directory, so packaged builds no longer fall back to an empty config and fail chat requests with `Unknown provider type` (and lose provider icons).

## [0.1.2] - 2026-05-24

### Added
- Linear-inspired design system: dual-mode surface ladder, hairline borders, and a 3-tier radius hierarchy (button 6 / card 8 / panel 12).
- Geist Variable + Geist Mono Variable as the default UI fonts.
- `docs/ui-design.md` capturing the Linear design system and HandBox deviations.

### Changed
- Refactored chat, settings, edge routes (words/selection/agent/favorite), and modals to the new Linear surface ladder.
- Inverted sidebar/main surface roles to match the Linear pattern; main card bleeds to all four window edges and fills the viewport when the sidebar is closed.
- Tightened typography and spacing across sidebar, title bar, message bubbles, and modals; aligned all card radii to `rounded-xl` (12px).
- Switched base inputs and modals to `bg-base-300` inset with hairline borders and removed semibold weight.
- Remapped `@theme` tokens to a Linear-inspired palette with tighter primary contrast and a real blue tint on dark surfaces.

### Fixed
- Layout gap between sidebar and main content card when the sidebar is closed.
- Modal surface lift moved into `Modal.svelte` to remove the redundant wrapper in `AddProviderModal`.
- A11y warnings and dead code surfaced at dev startup.

### Removed
- Dropped the unused `TextSelectionMenu` wrapper from message bubbles.
- Removed Windows from the release matrix.

## [0.1.1] - 2026-05-06

### Added
- System tray (menu bar) icon with Open / Do Something / Quit menu.
- In-app updater wired through `tauri-plugin-updater` with a Settings page check/install flow.
- Release script `scripts/release.sh` and GitHub Actions release workflow.

### Changed
- Replaced local path crate dependencies (`openai-rust`, `google-genai-rust`) with remote git references.
- Hide main window on close instead of destroying it, so the tray Open command can always restore it.

### Fixed
- Corrected misleading error message reporting `OSSEndpoint` when `OSSRegion` was missing from environment.

## [0.1.0] - 2026-05-06

### Added

- Initial baseline release of handbox.
