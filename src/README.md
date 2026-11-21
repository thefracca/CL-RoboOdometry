# FTC Path HUD

Editor grafico per progettare e simulare l'**autonoma** di un robot FTC (mecanum).
Disegni il percorso sul campo, evidenzi dove si attivano intake / flywheel / sparo,
guardi la simulazione e **esporti un JSON** che il codice del robot può leggere.

> È solo un HUD di pianificazione: simula la cinematica del robot (posizione, heading,
> tempi). Non simula la fisica delle palline e il campo visivo della camera è solo grafico.

---

## Cosa sa fare

- **Percorso a segmenti**: linee, curve (Bézier) e *ricerca palline* (linea retta con area di ricerca).
- **Heading del robot indipendente dal movimento** (mecanum) con rotazione sul posto tra i segmenti.
- **Azioni a fine segmento**: Wait, Shoot+Stop, Shoot+Move, più un **Align** (gira su se stesso) combinabile con le azioni.
- **Evidenziatore**: tratti del percorso dove sono attivi **Intake**, **Flywheel**, **Intake giù** (barra abbassata) e **Sparo**
  (a tratto oppure puntuale: il robot si ferma e spara).
- **Parcheggio d'emergenza**: se negli ultimi secondi (default 4 s) il robot sta ancora eseguendo il percorso, lascia tutto e va a un punto scelto.
- **Barriere** (zone vietate), **linee focali** (snap su una retta), griglia con snap.
- **Simulazione a 30 s** con timer, indicatori (INTAKE, FLYWHEEL, SEARCH, INTAKE GIÙ, SHOOT) e **grafico velocità/tempo**.
- **Campo visivo camera** (150°) disegnato davanti al robot.
- **Salvataggio** del progetto e **export JSON**.

---

## Installazione e avvio

Requisiti: [Rust](https://rustup.rs) (edizione 2021).

```bash
cargo run --release
```

Dipendenze principali: `eframe`/`egui`, `image`, `serde_json`.
Servono i file `assets/field.png` (immagine del campo) e `assets/icon.png` (icona finestra):
vengono incorporati nell'eseguibile con `include_bytes!`, quindi l'`.exe` finale è uno solo.

---

## Uso rapido

1. **Modalità** (barra a sinistra o tasti `1`–`8`): scegli cosa piazzare.
2. **Percorso**: in modalità *Line* / *Curva* / *Ricerca* ogni click aggiunge un punto di arrivo partendo dall'ultimo punto.
3. **Regola i segmenti**: ogni segmento ha un menu a tendina nella barra a sinistra (velocità, heading, azione, align).
4. **Evidenzia** (tasto `7`): scegli il tipo (INTAKE, FLYWHEEL, INTAKE GIÙ, SPARO) e **trascina sul percorso**.
   Per lo **SPARO puntuale** fai un **click su un nodo** del percorso (un punto di arrivo, start escluso): il robot si ferma lì e spara per N secondi, come uno Shoot+Stop.
5. **Parcheggio** (tasto `8`): piazza il punto di emergenza e regola "ultimi s", velocità e heading.
6. **Simula** con `Space` o il pulsante ▶ Avvia.
7. **Salva** (`Ctrl+S`) ed **esporta** (`Ctrl+E`).

### Scorciatoie

| Tasto | Azione |
|---|---|
| `1` … `8` | Select, Line, Curva, Barriera, Focale, Ricerca, Evidenzia, Parcheggio |
| `G` | Griglia + snap ON/OFF |
| `Space` | Play / Stop simulazione |
| `Home` | Robot → Start |
| `Ctrl+Z` | Annulla ultimo segmento |
| `Ctrl+S` / `Ctrl+O` | Salva / Carica progetto |
| `Ctrl+E` | Esporta JSON |
| `Shift+Del` | Cancella percorso |
| `Shift` + click | Snap ortogonale (orizzontale/verticale) |
| `Shift` + drag sul robot | Ruota il robot |
| Tasto destro | Elimina il punto sotto il cursore |
| Doppio click (Curva/Select) | Aggiunge un punto di controllo alla curva |

---

## Sistema di coordinate

- Unità: **centimetri**. Campo 365.76 × 365.76 cm (6 tile da 60.96 cm).
- Origine `(0,0)` in basso a sinistra, **+Y verso l'alto**.
- Heading in **gradi**: `0° = destra`, `90° = su`, antiorario.
- Una **posizione sul percorso** è `indice segmento + frazione` (es. `2.35` = 35% del segmento n. 2).

---

## File prodotti

Nella cartella da cui lanci il programma:

| File | Contenuto |
|---|---|
| `progetto.hudproject` | Progetto salvato (si ricarica con `Ctrl+O`). |
| `path.json` | Export per il robot (viene anche copiato negli appunti con il pulsante *Esporta JSON*). |

### Formato `path.json`

```json
{
  "units": "cm",
  "match_duration": 30,
  "start": { "x": 182.88, "y": 182.88, "heading": 90.00 },
  "segments": [
    {
      "id": 0,
      "type": "line",
      "to": { "x": 250.00, "y": 182.88 },
      "robot_heading": { "start": 90.00, "end": 90.00, "rotation_speed": 180.00 },
      "speed": { "v_start": 0.00, "v_max": 80.00, "v_min": 25.00,
                 "accel": 120.00, "decel": 200.00, "brake_point": 0.7500 },
      "action": "move",
      "align": null
    },
    {
      "id": 1,
      "type": "search",
      "to": { "x": 250.00, "y": 280.00 },
      "search": { "v": 60.00, "area_width": 60.00, "time_limit": 5.00,
                  "shoot_after": false, "shoot_time": 1.00 },
      "robot_heading": { "start": 90.00, "end": 90.00, "rotation_speed": 180.00 },
      "action": "move",
      "align": 180.00
    }
  ],
  "mechanisms": [
    { "type": "intake",      "from": { "segment": 0, "t": 0.20 }, "to": { "segment": 0, "t": 0.90 } },
    { "type": "intake_down", "from": { "segment": 0, "t": 0.10 }, "to": { "segment": 0, "t": 0.95 } },
    { "type": "shoot",       "from": { "segment": 1, "t": 0.30 }, "to": { "segment": 1, "t": 0.60 } },
    { "type": "shoot_point", "after_segment": 1, "time": 1.50 }
  ],
  "park": { "x": 300.00, "y": 60.00, "heading": 90.00, "trigger_time": 4.00, "v": 100.00 }
}
```

Note sul formato:

- `type` del segmento: `line`, `curve` (ha anche `controls`) o `search`.
- `speed`: le **linee** hanno il profilo completo; le **curve** solo `v_start`, `v_max`, `accel` (nessuna frenata); i segmenti **search** non hanno il blocco `speed` ma il blocco `search` (velocità `v` costante).
- `action`: `"move"`, `{"wait": s}`, `{"shoot_stop": s}` oppure `{"shoot_move": s}`.
- `align`: gradi a cui ruotare sul posto a fine segmento, oppure `null`. Si esegue **prima** dell'azione.
- `mechanisms`: tratti in cui un meccanismo è attivo (`intake`, `flywheel`, `intake_down`, `shoot`) o sparo puntuale (`shoot_point`: finito il segmento `after_segment` il robot si ferma e spara `time` secondi; è sempre su un nodo).
- `park`: `null` se il parcheggio d'emergenza è disattivato.
- Le **barriere non sono esportate** (restano solo nel `.hudproject`).

---

## Come ragiona la simulazione (in breve)

- Ogni segmento: il robot parte da `v_start`, accelera fino a `v_max`, dal `brake_point` frena fino a `v_min` e **si ferma sul posto all'arrivo**.
- Tra due segmenti ruota sul posto dall'heading finale del precedente a quello iniziale del successivo.
- A fine segmento l'ordine è: **Align → sparo puntuale → azione** (Wait / Shoot).
- Il timer è di **30 s**: scaduto il tempo il robot si ferma dove si trova.
- Se è attivo il parcheggio e mancano meno di *N* secondi con il percorso ancora in corso, il robot va dritto al punto di parcheggio.

## Limiti noti

- Nessuna fisica delle palline né rilevamento reale con la camera: l'area di ricerca e il cono visivo sono solo indicazioni grafiche.
- Il *tempo limite* della ricerca è un parametro per il robot (nel JSON); nella simulazione cambia solo l'etichetta.
- Il salvataggio usa sempre `progetto.hudproject` nella cartella di lavoro.
