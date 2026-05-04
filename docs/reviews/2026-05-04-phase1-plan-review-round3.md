# Phase 1 Plan Review — Round 3

Tre specialistgranskningar utförda: säkerhet, systemarkitektur och UX/design.

---

## Kritiska problem (måste fixas innan implementation)

### 1. Escape från Insert mode flyttar inte cursor åt vänster (UX)
I riktig Vim flyttar `Escape` från Insert mode cursor ett steg åt vänster (om inte redan vid kolumn 0). Planen byter bara mode utan att justera cursor-position. Alla vim-användare kommer reagera.

**Fix:** I `apply_action` för `SwitchMode(Normal)`, om nuvarande mode är Insert, flytta cursor ett steg åt vänster (clampad till 0).

### 2. Cursor-clamping skiljer inte mellan Normal och Insert mode (UX + Arkitektur)
`set_cursor` clampar kolumn till `line_content_len`, men i Normal mode ska max vara `line_content_len - 1` (sista tecknet, inte efter det). I Insert mode ska cursor kunna vara vid `line_content_len` (efter sista tecknet). Testet `set_cursor_clamps_to_valid` bekräftar buggen: "hi" clampar till col 2, men Normal mode ska clampa till col 1.

**Fix:** Antingen lägg till en `mode`-parameter i `set_cursor`, eller skapa `set_cursor_normal`/`set_cursor_insert` varianter.

### 3. Undo är per-tecken, inte per insert-session (UX + Arkitektur)
Varje `insert_char` skapar en separat `EditAction`. `u` ångrar bara ett tecken. I Vim ångrar `u` hela insert-sessionen (allt mellan `i` och `Escape`). Detta är den mest användarfientliga undo-buggen möjlig.

**Fix:** Lägg till group/transaction-markörer i `EditAction` (`GroupStart`/`GroupEnd`). Öppna grupp vid Insert mode entry, stäng vid Escape. `undo()` spollar tillbaka till matchande `GroupStart`.

### 4. History struct typ-mismatch: Vec vs VecDeque (Arkitektur)
Step 1 (testscaffold) definierar `undo_stack: VecDeque<EditAction>`, men Step 3 (implementation) definierar `undo_stack: Vec<EditAction>` medan constructor och metoder använder `VecDeque::new()`, `push_back()`, `pop_front()`. Kompilerar inte.

**Fix:** Använd `VecDeque` konsekvent i både struct-definition och implementation.

---

## Höga prioritet (bör fixas)

### 5. Ingen filstorleksbegränsning vid öppning (Säkerhet)
`read_file` kallar `fs::read_to_string` utan storleksgräns. Öppna en multi-GB fil → OOM.

**Fix:** Kolla `metadata.len()` före läsning, begränsa till t.ex. 256 MB. Avvisa icke-reguljära filer.

### 6. `$` med count korsar inte rader (UX)
`3$` i Vim = gå till slutet av raden 2 rader nedanför. Planen loopar `$` 3 gånger på samma rad.

**Fix:** `LineEnd` med count > 1 ska först flytta ner `count - 1` rader, sedan gå till radens slut.

### 7. `x` vid slutet av raden tar inte bort något (UX)
`x` delegerar till `Delete(Right)` motion, men `Right` i Normal mode stannar vid `content_len - 1`. Om cursor redan är där → start == end → inget tas bort.

**Fix:** `x` bör vara en dedikerad action (`DeleteCharAtCursor`) med explicit hantering för sista tecknet.

### 8. Word motions (`w/b/e`) behandlar inte punktuation som ordgräns (UX + Arkitektur)
Implementationen skiljer bara på whitespace vs non-whitespace. Vim har tre klasser: word chars (alfanumerisk + _), punctuation, whitespace. I `foo.bar` ska `w` stanna vid `.`.

**Fix:** Tre-klass karaktärsklassificering: word, punctuation, whitespace. Viktigt för all kod-editering.

### 9. Word motions korsar inte rader korrekt (UX + Arkitektur)
`word_forward` korsar en radgräns men hoppar inte förbi blanka rader. `word_end` korsar inte rader alls. `word_backward` fungerar inkomplett över rader.

**Fix:** Implementera word motions med globalt char-offset via rope-iteration istället för per-rad strängar.

### 10. `set_mode` är `#[cfg(test)]` — kan inte användas i produktion (Arkitektur)
`KeyParser::set_mode` är bara tillgänglig i testbyggen. Men `Editor` behöver ibland ändra mode externt. Design-kontraktet är oklart: mode ändras både internt i KeyParser OCH genom `handle_escape()`.

**Fix:** Gör `set_mode` publik (ta bort `#[cfg(test)]`), eller säkerställ att alla mode-övergångar går exklusivt genom KeyParser-metoder.

### 11. `cursor_offset() - cursor_col()` istället för `line_to_char()` (Arkitektur + Säkerhet)
`OperatorEngine` beräknar radstart som `buffer.cursor_offset() - buffer.cursor_col()`. Om cursor-state är inkonsekvent → usize underflow → panik. `line_to_char()` finns nu men används inte i operator-koden.

**Fix:** Byt alla `cursor_offset() - cursor_col()` till `buffer.line_to_char(buffer.cursor_line())`.

### 12. `text()` allokerar hela buffern som String (Arkitektur + Säkerhet)
Används vid fil-sparning. 100 MB fil → 100 MB allokering. Ropey har `write_to()` och `chunks()` som kan streama direkt.

**Fix:** Lägg till `write_to<W: Write>()` på TextBuffer som delegerar till `rope.write_to()`. Uppdatera `write_file` att använda den.

### 13. Saknar Ctrl+D/U/F/B (halv-sida/hel-sida) (UX)
Fundamentala navigeringstangenter för alla vim-användare som jobbar med filer längre än en skärm.

**Fix:** Lägg till special-hantering i `handle_input()` för `Ctrl+D`, `Ctrl+U`, `Ctrl+F`, `Ctrl+B`.

### 14. Statusbar visar inte cursor-position (UX)
Ingen indikation på rad/kolumn. Grundläggande information i alla editorer.

**Fix:** Högerställd sektion i statusbaren: `Ln 42, Col 15`.

### 15. Ingen modifierad-indikator i statusbaren (UX)
Inget visuellt tecken på osparade ändringar.

**Fix:** `dirty: bool` i Editor. Visa `[+]` bredvid filnamnet.

### 16. `:q` varnar inte om osparade ändringar (UX)
`CommandResult::Quit` avslutar direkt oavsett buffer-state. Användaren tappar arbete.

**Fix:** Kontrollera dirty-flag vid `:q`. Visa "No write since last change (add ! to override)" och neka avslut.

### 17. Task 4 beror på Task 5 men sammanfattningen säger bara Task 2 (Arkitektur)
Task 4 importerar `crate::vim::mode::Mode` som skapas i Task 5. Kan inte kompilera utan Task 5.

**Fix:** Uppdatera beroendetabellen: `| 4 | Editor rendering | 2, 5 |`.

### 18. Task 9 refererar `self.config` innan Task 11 skapar det (Arkitektur)
Task 9 Step 6 render-anrop använder `self.config.editor.font_size`, men `config`-fältet läggs till i Task 11.

**Fix:** Använd `14.0` i Task 9, eller lägg till Task 11 som beroende för Task 9.

### 19. `recording`-flagga återställs inte vid panik (Säkerhet)
`undo()`/`redo()` sätter `set_recording(false)` före replay, sedan `set_recording(true)` efter. Om en panik inträffar däremellan förblir `recording = false` permanent.

**Fix:** Använd guard-pattern eller strukturera om så att `recording` alltid återställs.

### 20. Saknar `J` (join lines) (UX)
Grundläggande vim-kommando för att slå ihop rader. Används konstant.

**Fix:** Lägg till `J` som dedikerad VimAction.

### 21. Inga `Cmd+`-keybinding stubs (UX — spec-avvikelse)
Specen listar `Cmd+1/2/3`, `Cmd+K`, `Cmd+P` osv. Inget av detta fångas upp. Cmd-tangenter kan läcka in som textinput.

**Fix:** Fånga `Cmd+`-modifierare och antingen ignorera eller visa "Not yet implemented".

---

## Medel prioritet

### 22. Ingen path-validering/kanonisering (Säkerhet)
CLI-argument passas direkt till read_file/write_file. Symlänkar och relativa sökvägar valideras inte.

### 23. `delete_char_before_cursor` kan ge usize underflow (Säkerhet)
`self.cursor_col -= 1` utan `saturating_sub` om invariant bryts.

### 24. `slice()`/`delete_range()`/`insert_text_at()` panikerar på ogiltiga index (Säkerhet)
Delegerar direkt till rope utan bounds-checking.

### 25. Count prefix (max 99999) förstärker destruktiva operationer i loop (Säkerhet)
99999 separata `delete_range`-anrop → fryst UI + 99999 history entries.

### 26. Ingen validering av config-värden efter deserialisering (Säkerhet)
`font_size: 0.0`, `NaN`, `Infinity`, `tab_size: 0` → rendering-buggar.

### 27. Atomic write förstör original-filens permissions (Säkerhet)
Tempfil skapas med default permissions, rename tar bort originalet.

### 28. Per-rad allokering i render-loop (Arkitektur)
`line_slice.to_string()` × 50 rader × 60fps = 3000 allokeringar/s.

### 29. `painter.layout_no_wrap` kan saknas på `Painter` (Arkitektur)
API:et finns på `egui::Fonts`, inte `egui::Painter`. Kanske kompilerar inte.

### 30. Alla Editor-fält är `pub` — läckande inkapsling (Arkitektur)
`NyxApp` når direkt in i `editor.key_parser` och `editor.command_parser`.

### 31. `delete_range` uppdaterar inte cursor (Arkitektur)
Efter radering kan `cursor_line` peka på ogiltigt index.

### 32. `Right` motion begränsar för Normal mode oavsett aktuellt mode (Arkitektur)
Insert mode borde tillåta cursor vid `content_len`, inte `content_len - 1`.

### 33. Inget stöd för `r` (replace character) (UX)

### 34. Inga `f/F/t/T` (find char in line) motions (UX)

### 35. Inget `.` (dot-repeat) — explicit Phase 2, men starkt saknat (UX)

### 36. Ingen sticky/desired column för `j/k` (UX)

### 37. `o`/`O` edge case vid trailing newline/EOF (UX)

### 38. Status message har ingen timeout/auto-clear (UX)

### 39. Ingen feedback vid okända Normal mode-tangenter (UX)

### 40. Backspace i tom command line gör inget — borde lämna command mode (UX)

### 41. Ingen `:e` (öppna fil) command (UX)

### 42. `relative_line_numbers` config-option implementeras inte i rendering (UX)

### 43. Theme JSON-filer laddas inte — bara hårdkodad default (UX)

### 44. Gutter-bredd hårdkodad till 50px (UX)

### 45. Arrow keys hanteras inte (UX)

### 46. Tab-tangent hanteras inte (UX)

### 47. Ingen musstöd (klick/scroll) trots att spec nämner det (UX)

### 48. `keybindings.json` laddas inte (spec-avvikelse)

### 49. Config schema smalare än spec (saknar panels/lsp sektioner)

### 50. Config modul beror på file_io — oredovisad beroende (Arkitektur)

---

## Låg prioritet

### 51. `line_content_len` hanterar inte `\r\n` (Arkitektur)
### 52. `tempfile` i dev-deps men behövs runtime (Arkitektur)
### 53. `handle_escape` muterar OCH returnerar action — oklart kontrakt (Arkitektur)
### 54. `line_slice()` panikerar på out-of-range (Säkerhet)
### 55. Config-katalog skapas utan explicita permissions (Säkerhet)
### 56. `pending`-sträng i KeyParser har ingen längdgräns (Säkerhet)
### 57. Ingen binärfils-detektion (Säkerhet)
### 58. Ingen per-entry storleksgräns i History (Säkerhet)
### 59. Command mode visar ingen cursor (UX)
### 60. Mode-label har hårdkodad x-position (UX)
### 61. `line_numbers: false` respekteras inte (UX)
### 62. Ingen visuell markering av aktuell rad (UX)
### 63. Linux-tangentmotsvarigheter inte explicita (UX)
### 64. Welcome-buffer redigerbar och markeras som dirty (UX)
### 65. Cursor-position sparas inte i undo-entries (UX)
### 66. Inget test för Ctrl+R redo-path i KeyParser (UX)

---

## Sammanfattning per granskare

| Granskare | Antal | Toppfynd |
|-----------|-------|----------|
| **Säkerhet** | 18 | Obegränsad filstorlek, path traversal, panik-osäker recording-flagga, bounds-checking saknas |
| **Arkitektur** | 23 | Vec/VecDeque mismatch, undo ej atomär, line_to_char inkonsekvent, task-beroenden fel |
| **UX/Design** | 43 | Escape flyttar ej cursor, mode-omedveten clamping, per-tecken undo, saknade vim-kommandon |

## Rekommenderad fix-prioritering

**Måste fixas innan implementation (4 st):**
1. Undo-gruppering (insert session = en undo-enhet)
2. Escape från Insert → flytta cursor åt vänster
3. Mode-medveten cursor-clamping (Normal vs Insert)
4. History Vec/VecDeque mismatch

**Bör fixas (17 st):**
5–21 i listan ovan.

**Notera:** Många av UX-reviewerns "saknade features" (f/F/t/T, J, r, ., %, Ctrl+D/U) är rimliga att skjuta till Phase 2 om de dokumenteras explicit som kända begränsningar.
