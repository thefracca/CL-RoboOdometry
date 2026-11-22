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
- **Azioni a fine segmento**: Move o Wait, più un **Align** (gira su se stesso) che si combina con il Wait.
- **Evidenziatore**: tratti del percorso dove sono attivi **Intake**, **Flywheel**, **Sparo**, **Intake giù** e **Barra giù**. Intake / Flywheel / Sparo si possono mettere anche **su un singolo nodo** (il robot si ferma) con **ritardo** e **durata** (es.: spara 4 s e l'intake parte dopo 2 s).
- **Parcheggio d'emergenza**: se negli ultimi secondi (default 4 s) il robot sta ancora eseguendo il percorso, lascia tutto e va a un punto scelto.
- **Barriere** (zone vietate create da te), **zone vietate permanenti** (poligoni) che disegni col tool ⛔ (tasto `9`) e si salvano da sole in `zone_vietate.json` (il corpo del robot non deve mai toccarle, nemmeno a 2 cm: se succede il robot diventa rosso), **linee focali** (snap su una retta), griglia con snap.
- **Simulazione a 30 s** con timer, indicatori (INTAKE, FLYWHEEL, SEARCH, INTAKE GIÙ, SHOOT, BARRA GIÙ) e **grafico velocità/tempo**.
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

1. **Modalità** (barra a sinistra o tasti `1`–`9`): scegli cosa piazzare.
2. **Percorso**: in modalità *Line* / *Curva* / *Ricerca* ogni click aggiunge un punto di arrivo partendo dall'ultimo punto.
3. **Regola i segmenti**: ogni segmento ha un menu a tendina nella barra a sinistra (velocità, heading, azione, align).
4. **Evidenzia** (tasto `7`): scegli il tipo (INTAKE, FLYWHEEL, SPARO, INTAKE GIÙ, BARRA GIÙ) e **trascina sul percorso**.
   Per INTAKE / FLYWHEEL / SPARO puoi anche fare un **click su un nodo** del percorso (start escluso): il robot, arrivato lì, si ferma e l'azione parte dopo il **ritardo** ("dopo s") per la **durata** ("dura s") che imposti nella lista. **BARRA GIÙ** e INTAKE GIÙ si mettono solo a tratto: evidenzia anche attorno ai nodi dove deve restare giù mentre il robot fa altro (così non la abbassi per sbaglio contro un muro).
5. **Parcheggio** (tasto `8`): piazza il punto di emergenza e regola "ultimi s", velocità e heading.
6. **Zone vietate** (tasto `9`): ogni click aggiunge un vertice del poligono; chiudi la zona con doppio click, click sul primo vertice o `Invio` (`Esc` annulla la bozza, tasto destro toglie l'ultimo vertice). Trascina un vertice per modificarlo o trascina dentro la zona per spostarla tutta; tasto destro su una zona la elimina. Con la griglia attiva i vertici si agganciano a griglia e bordi del campo, con `Shift` si allineano all'ultimo vertice. Sono ammessi anche poligoni concavi (non devono autointersecarsi). Si salvano da sole e restano per tutti i percorsi.
7. **Simula** con `Space` o il pulsante ▶ Avvia.
8. **Salva** (`Ctrl+S`) ed **esporta** (`Ctrl+E`).

### Scorciatoie

| Tasto | Azione |
|---|---|
| `1` … `9` | Select, Line, Curva, Barriera, Focale, Ricerca, Evidenzia, Parcheggio, Zone vietate |
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
| `zone_vietate.json` | Zone vietate permanenti, come liste di vertici (si salva da solo quando le modifichi; si carica all'avvio; non fa parte del progetto). I vecchi file con zone circolari vengono letti e convertiti in poligoni a 16 lati. |
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
      "robot_heading": { "start": 90.00, "end": 90.00 },
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
      "robot_heading": { "start": 90.00, "end": 90.00 },
      "action": "move",
      "align": 180.00
    }
  ],
  "mechanisms": [
    { "type": "intake",      "from": { "segment": 0, "t": 0.20 }, "to": { "segment": 0, "t": 0.90 } },
    { "type": "intake_down", "from": { "segment": 0, "t": 0.10 }, "to": { "segment": 0, "t": 0.95 } },
    { "type": "shoot",       "from": { "segment": 1, "t": 0.30 }, "to": { "segment": 1, "t": 0.60 } },
    { "type": "bar_down",    "from": { "segment": 0, "t": 0.05 }, "to": { "segment": 1, "t": 0.95 } },
    { "type": "point", "mechanism": "shoot",  "after_segment": 1, "delay": 0.00, "time": 4.00 },
    { "type": "point", "mechanism": "intake", "after_segment": 1, "delay": 2.00, "time": 2.00 }
  ],
  "park": { "x": 300.00, "y": 60.00, "heading": 90.00, "trigger_time": 4.00, "v": 100.00 }
}
```

Note sul formato:

- `type` del segmento: `line`, `curve` (ha anche `controls`) o `search`.
- `speed`: le **linee** hanno il profilo completo; le **curve** solo `v_start`, `v_max`, `accel` (nessuna frenata); i segmenti **search** non hanno il blocco `speed` ma il blocco `search` (velocità `v` costante).
- `action`: `"move"` oppure `{"wait": s}`. Sparo, intake e flywheel **non** sono azioni del segmento: stanno in `mechanisms`.
- `align`: gradi a cui ruotare sul posto a fine segmento, oppure `null`. Si esegue **prima** dell'azione. La velocità di rotazione non è nel JSON (non è regolabile: la gestisce il follower).
- `mechanisms`: tratti in cui un meccanismo è attivo (`intake`, `flywheel`, `intake_down`, `bar_down`, `shoot`) oppure azioni puntuali (`"type":"point"`): finito il segmento `after_segment` il robot si ferma; `mechanism` (`intake` / `flywheel` / `shoot`) parte dopo `delay` secondi e dura `time` secondi. La sosta dura fino alla fine dell'ultima azione. La barra è sempre alzata tranne dentro i tratti `bar_down`.
- `park`: `null` se il parcheggio d'emergenza è disattivato.
- Le **barriere non sono esportate** (restano solo nel `.hudproject`).

---

## Come ragiona la simulazione (in breve)

- Ogni segmento: il robot parte da `v_start`, accelera fino a `v_max`, dal `brake_point` frena fino a `v_min` e **si ferma sul posto all'arrivo**.
- Tra due segmenti ruota sul posto dall'heading finale del precedente a quello iniziale del successivo (velocità fissa 180°/s, usata solo per le stime).
- A fine segmento l'ordine è: **Align → azioni puntuali del nodo (sparo / intake / flywheel con ritardo e durata) → azione** (Wait). Le azioni puntuali tengono fermo il robot fino alla fine dell'ultima (ritardo + durata).
- Il timer è di **30 s**: scaduto il tempo il robot si ferma dove si trova.
- Se è attivo il parcheggio e mancano meno di *N* secondi con il percorso ancora in corso, il robot va al punto di parcheggio in linea retta, aggirando le zone vietate.

## Limiti noti

- Nessuna fisica delle palline né rilevamento reale con la camera: l'area di ricerca e il cono visivo sono solo indicazioni grafiche.
- Il *tempo limite* della ricerca è un parametro per il robot (nel JSON); nella simulazione cambia solo l'etichetta.
- Le **zone vietate** sono poligoni (lista di vertici) che disegni tu col tool ⛔ e vivono in `zone_vietate.json` nella cartella di lavoro (copialo se vuoi le stesse zone su un altro PC). Se il corpo del robot (45×45 cm, ruotato col suo heading) tocca una zona o sta a 2 cm o meno, in simulazione il robot diventa **rosso** e compare un avviso; i segmenti dove succede sono rossi. I nodi non si possono piazzare troppo vicino alle zone. Il contorno giallo sottile attorno a ogni zona è il limite per il *centro* del robot.
- Il salvataggio usa sempre `progetto.hudproject` nella cartella di lavoro.
