# Phase 7: Bot + Settings + Docs + Packaging + Audit - UI Design Contract

**Created:** 2026-09-09
**Status:** Approved

## 1. Visual Hierarchy & Dimensions

- **Bot Management View:** Top navigation tabs (Triggers, Cron Jobs, Webhooks, Logs), table list of rules with active toggles and actions (Edit, Test, Delete), Code Editor view with syntax highlighting area and right-side AI Assistant Drawer (360px).
- **AI Assistant Drawer:** Chat history with user prompts and AI responses, model selector dropdown (Gemini 1.5 Flash, Pro), markdown response container, and "Apply to Script" action button.
- **Settings View:** Categorized settings cards (General, AI Configuration, TTS Engine, Privacy & Sync, About). Password inputs show `••••••••` when masked, with an eye icon to unmask.
- **Documentation View:** Left docs index menu, right markdown content viewer.

## 2. Interaction Flows

1. **AI Assistant Apply Code:** When AI returns a code block, clicking "Apply Code" writes the snippet directly into the bot script editor.
2. **Masked Keys:** Typing API keys automatically marks them sensitive; backend returns boolean flags `hasGeminiKey`, `hasFishKey` without leaking plaintext.
3. **Trigger / Cron Testing:** Test console executes script against mock incoming message and displays stdout/stderr output.
