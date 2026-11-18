# shell/modules/settings/pages/ai.qml ↔ surfaces/settings (src/pages/accounts/ai.rs `page`, ui/settings-accounts.slint "note", "field")

_Re-checked 17 Nov (B5, parity-settings) against main 18628a5 + the B5 ports: statuses below say what's there now; [12 Nov: …] is the old status._

AI & Sidebar (page `ai`; sidebar.qml is its part "Sidebar", own parity
file). Models are config `ai.models` (core services/ai/models.rs as
AiModels.js); saving, removing (8 s undo) and credentials go to the shell
(bridge `aiSave`, `aiRemove`, `aiUndo`, `aiSetModel`, `aiDeleteAll`; feed
`ai.hasKey`, `envKeys`, `sessions`, `pinned`, `undo`). Menus: Dropdowns
Selected model, Provider, Authentication, Style, Delete chats older than.

## Behaviours

| # | Behaviour (QML line) | Rust place | Status |
|---|---|---|---|
| 1 | Group "Your models" + footer; "Selected model": detail providerName / "Add your first model below"; Dropdown 270 "name · provider" of models, disabled with "No models added" when none → Ai.setModel (84–98) | ai.rs 180–198 | same |
| 2 | Per model: name; "Provider · id"; "Edit" → load into the form; "Remove" danger → removeModel (credential forgotten after 8 s), editing it → form cleared (99–119, 38–47) | 199–212; bridge 212–223 | same |
| 3 | Undo group (no title) while undo: "<name> removed" + "Undo" (arrow-counter-clockwise) → restoreModel; 8 s timer (122–137, 28–37) | 219–224; bridge 21–31, 224–230 | same |
| 4 | Group title "Edit model"/"Add model" + footer (139–142) | 289–290 | same |
| 5 | "Display name" TextField 270 "Name shown in Tessera", detail (143–147) | 271 | same |
| 6 | "Provider" Dropdown 270 of AiModels providers (7); detail "Gemini models on Vertex AI" for vertex; choosing sets auth none (ollama)/apiKey, clears message (148–161) | 233–242, 272 | same |
| 7 | "Model ID" TextField 270; detail OpenRouter text / "Copy the exact model ID…" (162–166) | 273 | same |
| 8 | "Authentication" (vertex/custom) Dropdown 270: vertex API key (Vertex Express)/OAuth access token; custom API key/No authentication (167–176) | 243–251, 274 | same |
| 9 | "API base URL" (custom/ollama), placeholder ollamaUrl + "/v1" or example (177–186) | 275–277 | same |
| 10 | "Google Cloud project", "Location" (text "global") for vertex + token (187–197) | 278–279 | same |
| 11 | Credential row (not ollama, auth ≠ none): label "OAuth access token"/"API key"; detail short-lived / "A saved credential or environment variable is available" / "Paste a credential…"; secret field 270, no commit on blur (198–209) | 252–257, 280–282 | same |
| 12 | "Max output tokens" (anthropic) field 120 "4096" (210–215) | 283 | same |
| 13 | "Reads pictures" switch: vision ?? guessed from provider + typed ID (live while typing) (18–20, 216–220) | 85–104, 259, 284 | same |
| 14 | "Adaptive thinking" (anthropic) switch (221–226) | 260, 285 | same |
| 15 | Status row: label = message, danger when error else secondary; "Add model"/"Save changes" primary → save (error shown; else credential saved if typed, form reloaded, "Model saved and selected."); "New model" → form cleared (227–243, 65–82) | 145–160, 263–269, 287 | same |
| 16 | Group "Conversation": "Effort" (selected model anthropic + thinking) Segmented 300 Low…Max → ai.effort (246–260) | 296–322 | same |
| 17 | "Style" Dropdown 200 Balanced…Playful → ai.style (261–276) | 299–323 | same |
| 18 | "Your note" row, detail "… N of 600" live (277–280) | 324 | same |
| 19 | Note box 112 tall: Glass fill + hairline (accent on focus), Material FilledField r20; margins 20/14, bottom 16/12; TextArea wraps, placeholder, saved (cleaned) on editingFinished (282–315) | 327; slint "note" 188–245 | same at a glance |
| 20 | Note **kept to 600 characters while typing** (onTextChanged removes beyond MAX_NOTE) (310) | ai.rs 308–316 truncates only the stored copy | **same (fixed before 17 Nov: noteRev re-syncs the field with the cut text)** [12 Nov: **different**: the field itself keeps the 601st+ characters on screen (the field isn't re-synced when the stored text stays the same); the counter sticks at 600] |
| 21 | Group "Chat history" + footer; "Keep chat history" switch, detail by state (320–328) | 330–358 | same |
| 22 | "Delete chats older than" Dropdown 200 Never/30 days/90 days (329–338) | 333–359 | same |
| 23 | "Saved chats": detail count/pinned text, or the question while confirming; "Keep chats" (confirming) → stop; "Delete all chats" danger (disabled with none) → confirm, **focus moves to "Keep chats"**; "Delete N chats" → deleteAllChats (339–369) | 336–360, `saved_chats` 373–392 | **different (small, remaining): after "Delete all chats" keyboard focus isn't moved to "Keep chats"** [12 Nov: **different**: no focus move to "Keep chats" on the first press] |
| 24 | More options › "Conversation": "Show reasoning summary" switch (373–382) | 362–367 | same |
| 25 | PagePart "Sidebar" (385–388) | 369 `p.part(cx, "Sidebar", sidebar)` | same |
| 26 | Page state (form, message, confirming, note draft) starts over when the page loads again | lib.rs 511/579 keep `ai.*` locals | **same (fixed before 17 Nov: ai.* in PAGE_TRANSIENT)** [12 Nov: **different**: the form, "Delete N chats?" question and message survive leaving the page] |

## Test plan

Setups: `tools/settings-scenarios.sh setup qml|rust material|glass <bin>`;
Material window 1040×700 at 440,205, page column from x≈772. `open ai`.
**Read y from a shot.** Never type or paste a real API key; credential fields
stay empty. Only a keyless Ollama model may be added (local config only).

AI1 Page: shot top to bottom (incl. Sidebar part). PASS: same groups, rows,
details, footers, both looks.

AI2 Menus: open "Provider" (shot the list), Esc; pick "Custom" → Authentication
and API base URL rows appear; pick "Google Vertex AI" → Authentication options
switch, detail "Gemini models on Vertex AI"; pick "OAuth access token" →
project/location rows, credential label "OAuth access token". Pick "Ollama":
credential row gone, base URL placeholder "<ollamaUrl>/v1".

AI3 Vision guess: Provider OpenAI, type `gpt-4o` in Model ID: "Reads pictures"
flips as the QML does while typing.

AI4 Save error: clear Model ID, click "Add model". PASS: red status text (same words).
Then Provider Ollama, ID `test-model`, Add model → "Model saved and selected.",
the model in "Your models". Remove it: "test-model removed" + Undo; Undo
brings it back; remove again and wait 8 s: Undo row goes (time it).

AI5 Note limit: click the note, paste 650 chars (`run "key -t $(printf 'a%.0s' {1..650})"`).
PASS (QML): field holds 600, detail "600 of 600". Rust must match.

AI6 Delete chats (only with 0 chats in the arena the button is greyed: shot
it). If chats exist: click "Delete all chats" once (do NOT click the second
time), then `run 'key -k space'`. PASS (QML): focus was on "Keep chats", so
Space cancels the question. Rust must match.

AI7 Reset: start the question or type in Display name, go to another page and
back. PASS (QML): empty form, no question.

Clean up: remove test-model.

## Fixed in batch 3 (fixer)

- The note is cut at 600 characters on screen too (the field takes the cut text back); typed state starts over with the page.
- Not done: focus to "Keep chats" after the first Delete all chats.

## Test results

(tester)
