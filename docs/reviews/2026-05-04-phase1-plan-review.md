# Phase 1 Plan Review — Sammanställd Rapport

Tre specialistgranskningar utförda: säkerhet, systemarkitektur och UX/design.

---

## Kritiska problem (måste fixas innan implementation)

### 1. Byte vs char-indexering (Säkerhet + Arkitektur)
Renderer och word-motions använder byte-indexering (`display[..cursor_col]`, `as_bytes()`) men `cursor_col` är en char-offset. **Kommer att krasha vid icke-ASCII-tecken** (emoji, CJK, svenska tecken som å/ä/ö).

**Fix:** Använd `char_indices()` genomgående. Lägg till `TextBuffer::slice()` som delegerar till `rope.slice()`.

### 2. Ingen synlig mode-indikator (UX)
Phase 1 sätter `status_line` men renderar den aldrig. Användaren har **noll visuell feedback** om vilket vim-mode de befinner sig i. Kommandoraden (`:`) renderas inte heller.

**Fix:** Lägg till en minimal statusrad längst ner som visar mode + filnamn. Rendera command-input vid Command mode.

### 3. Cursor ändrar inte form mellan modes (UX)
Block-cursor alltid — vim-användare förväntar sig line-cursor i Insert mode.

**Fix:** Block-cursor i Normal, vertikal linje i Insert.

### 4. `Cmd+H` gömmer fönstret på macOS (UX)
`Cmd+H` är macOS system-shortcut för "Hide application". Kommer att gömma Nyx istället för att fokusera vänster panel.

**Fix:** Använd `Ctrl+h/j/k/l` eller `Cmd+Shift+h/j/k/l` för panelfokus på macOS.

### 5. Operatorer kringgår undo-historik (Arkitektur)
`OperatorEngine::execute()` anropar `buffer.delete_range()` direkt istället för `_recorded`-varianter. `dd`, `dw` etc. **kan inte ångras med `u`**.

**Fix:** Alla mutationer ska gå genom recorded-metoder, eller gör TextBuffer alltid-inspelande.

---

## Höga prioritet (bör fixas)

### 6. Tysta fel vid filsparning
`save_file()` sväljer errors med `let _`. Användaren får ingen feedback om sparningen misslyckas.

**Fix:** Visa felmeddelande i statusraden.

### 7. Config skriver över vid parse-fel
Malformerad `config.json` → tyst fallback till defaults + **överskrivning av existerande config**. Data loss.

**Fix:** Använd defaults i minnet, skriv inte över. Varna i statusraden.

### 8. `buffer.text()` i hot paths (Arkitektur)
`OperatorEngine` anropar `buffer.text()` (O(n) rope→string-konvertering) i loopar. Motverkar hela poängen med att använda en rope.

**Fix:** Lägg till `TextBuffer::slice(start, end) -> String` via `rope.slice()`.

### 9. Publika cursor-fält
`cursor_line` och `cursor_col` är `pub` — kan sättas till ogiltiga värden utifrån.

**Fix:** Gör privata med accessors som validerar/clampar.

### 10. Saknade grundläggande vim-kommandon
`a`/`A`/`o`/`O`/`I` saknas (plus att `a` i planen inte flyttar cursor åt höger). Count-prefix (`5j`, `3dw`) saknas helt. Dot-repeat (`.`) saknas.

**Fix:** Lägg till `a`/`A`/`o`/`O`/`I` i Phase 1. Planera count-prefix i KeyParser.

---

## Medel prioritet

### 11. Ingen atomic write
`fs::write` kan lämna korrupt fil vid krasch. Fix: Skriv till temp-fil, sedan `rename()`.

### 12. Saknar logging
Noll logging i hela planen. Fix: Lägg till `tracing` som dependency.

### 13. String-allokeringar i render-loop
`buffer.line(i)` returnerar `String` per synlig rad per frame (60fps × 50 rader = 3000 allokeringar/s). Fix: Returnera `RopeSlice`.

### 14. `font_id.clone()` i render-loop
`FontId` innehåller en `String` — klonas flera gånger per rad. Fix: Allokera en gång utanför loopen.

### 15. app.rs god-object risk
`NyxApp` äger redan 8+ fält och växer. Fix: Extrahera `Editor`-struct som äger buffer + vim-state.

### 16. Vim types i fel fil
`VimAction`, `MotionKind`, `OperatorAction` definieras i `keyparser.rs` men används överallt. Fix: Flytta till `src/vim/action.rs`.

### 17. `io`-modulen skuggar `std::io`
Fix: Döp om till `file_io` eller `storage`.

### 18. Tom första start
Ingen welcome-screen, ingen hjälp, ingen indikation om att moduler finns. Fix: Visa en minimal welcome-buffer vid första start.

---

## Framtida att tänka på (ej Phase 1)

- **Terminal:** PTY escape sequence-attacker, kommandoinjektion via `terminal:run`
- **LSP:** Untrusted binaries, ingen sandbox, download-integritet
- **Plugins:** Överväg WASM istället för native shared libraries för sandboxing
- **Keybindings:** Inget stöd för mode-specifika bindings i formatet
- **Zen mode:** Ingen toggle för att maximera editor utan allt chrome

---

## Sammanfattning per granskare

| Granskare | Toppfynd |
|-----------|----------|
| **Säkerhet** | Byte/char-kraschar, inga atomic writes, tyst error-svallning, framtida plugin-sandboxing |
| **Arkitektur** | Byte/char-bugg, operator-undo-bugg, buffer.text() i hot paths, god-object risk |
| **UX/Design** | Ingen mode-indikator, Cmd+H-konflikt, saknade vim-kommandon, ingen onboarding |
