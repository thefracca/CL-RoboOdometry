# FTC Path HUD

Strumento visivo per progettare, simulare ed esportare percorsi autonomi per robot FTC con ruote mecanum.

---

## Requisiti

- **Rust** 1.75+ con `cargo`
- **eframe** 0.28 (già nel `Cargo.toml`)
- Windows (build testata), Linux e macOS supportati da eframe

```toml
[dependencies]
eframe = "0.28.0"
serde_json = "1.0"
```

---

## Avvio

```bash
cargo run --release
```

I file `progetto.hudproject` e `path.json` vengono salvati nella stessa cartella dell'eseguibile.

---

## Interfaccia

```
┌───────────────┬────────────────────────────┬──────────────┐
│ Sidebar (sx)  │  Campo 365.76×365.76 cm    │ Shortcut (dx)│
│               │                            │              │
│ Modalità      │ • Griglia fine             │ Elenco tasti │
│ Simulazione   │ • Linee gialle = zona      │ rapidi       │
│ Segmenti      │   esclusione bordi         │              │
│ Barriere      │ • Waypoint, curve Bezier   │              │
│ Salva/Export  │ • Robot ciano              │              │
│               │ • Triangolini frenata      │              │
└───────────────┴────────────────────────────┴──────────────┘
```

---

## Modalità di editing

| Bottone | Funzione |
|---------|----------|
| ✋ **Select** | Seleziona e trascina punti, robot, control point, vertici barriera |
| 📏 **Line** | Click sul campo aggiunge un nodo con segmento dritto |
| 〜 **Curva** | Click aggiunge un nodo con curva Bezier cubica |
| 🚧 **Barriera** | Click aggiunge vertici di una zona non accessibile |
| 📐 **Focale** | Click A + click B crea una linea di riferimento con snap |

---

## Percorso

### Aggiungere nodi
1. Seleziona **Line** o **Curva**
2. Clicca sul campo per piazzare i waypoint uno alla volta
3. Il percorso parte sempre dal punto verde (Start)

### Muovere punti
- In modalità **Select**: trascina qualsiasi waypoint (arancio) o lo Start (verde)
- **Tasto destro** su un waypoint → lo elimina

### Curve Bezier
- Ogni curva ha uno o più **control point** (cerchietti oro)
- Trascina i control point per modificare la forma della curva
- **Doppio click** vicino a una curva esistente → aggiunge un control point
- **Tasto destro** su un control point → lo rimuove (se ne rimane almeno uno)

### Parametri per segmento
Ogni segmento ha un pannello collassabile nella sidebar con:

| Parametro | Descrizione |
|-----------|-------------|
| `v_start` | Velocità iniziale (cm/s) |
| `v_max` | Velocità massima (cm/s) |
| `accel` | Accelerazione (cm/s²) |
| `decel` | Decelerazione (cm/s²) |
| `v_min` | Velocità minima in frenata (cm/s): in decelerazione non scende mai sotto questo valore |
| `brake_point` | Frazione [0,1] della **lunghezza** del segmento dove inizia la frenata — draggabile sul campo (triangolino rosso ▼) |
| **Heading percorso** | Tangent = segue la traiettoria, Fixed = angolo fisso |
| **Heading robot** | Dove guarda il robot (mecanum), interpolato da inizio a fine segmento |
| **vel. rot. °/s** | Velocità di rotazione tra la fine di un segmento e l'inizio del successivo |
| **Azione** | Move / Wait / Shoot+Stop / Shoot+Move |
| **Intake / Flywheel** | Attiva i meccanismi durante il segmento |

### Profilo di velocità

Ogni segmento usa una cinematica reale (identica in simulazione e tempo stimato):

1. Parte da `v_start` e **accelera** con `accel` fino a `v_max`
2. Mantiene `v_max` finché non raggiunge il `brake_point`
3. Dal `brake_point` **decelera** con `decel`, ma mai sotto `v_min`
4. Arrivato al punto finale **si ferma sul posto** (anche se la velocità non è 0) e il segmento successivo riparte dal proprio `v_start`

Se il `brake_point` arriva prima di `v_max`, la frenata parte dalla velocità raggiunta in quel momento.

---

## Robot

- Il robot è il **quadrato ciano** sul campo
- **Trascina** per spostarlo liberamente
- **Shift + trascina** orizzontalmente → ruota (1 px = 1°)
- Lo slider **Heading** nella sidebar imposta l'angolo manualmente
- Bottone **📍 Robot → Start** posiziona il robot sul punto di partenza

### Frecce sul robot (durante la simulazione)
- **Freccia bianca** → dove guarda il robot (heading)
- **Freccia verde** → direzione di movimento (tangente al percorso)

Con le ruote mecanum le due frecce possono puntare in direzioni diverse.

---

## Barriere

Le barriere definiscono zone non accessibili al robot.

### Creare una barriera
1. Seleziona modalità 🚧 **Barriera**
2. Click per aggiungere vertici uno alla volta
3. **Doppio click** (o ultimo vertice) chiude il poligono (minimo 3 vertici)
4. **Tasto destro** durante la costruzione → annulla l'ultimo vertice

### Gestire le barriere
- In modalità **Select**: trascina i singoli vertici o l'interno del poligono per spostarlo
- **Tasto destro** su un vertice → rimuove quel vertice e riapre la barriera
- Quando una barriera viene chiusa, tutti i waypoint che finiscono dentro vengono automaticamente spostati fuori

### Zona di esclusione
Attorno ad ogni barriera compare una **linea tratteggiata gialla** che rappresenta la distanza minima (`robot_size/2 + 1 cm`). I waypoint non possono essere piazzati dentro questa zona.

La stessa zona di esclusione vale anche per i **bordi del campo**.

---

## Snap

Lo snap è automatico — nessun tasto da tenere premuto. Il punto si aggancia quando si avvicina abbastanza alla destinazione.

| Tipo | Quando si attiva | Feedback visivo |
|------|-----------------|-----------------|
| **Griglia** | Vicino a un incrocio della griglia (solo se griglia ON) | Croce verde |
| **Ortho** | Tenendo **Shift**: vicino alla linea orizzontale/verticale del punto precedente (barriere, linee, curve, linee focali). Funziona anche con griglia OFF | Linea blu + cerchio |
| **Focale** | Vicino a una linea focale | Linea gialla evidenziata + cerchio |

**Nota:** disabilitare la griglia (bottone "🔳 Griglia OFF" o tasto `G`) disabilita lo snap a griglia. Lo snap ortho resta disponibile tenendo Shift, le linee focali snappano sempre (finché visibili).

---

## Linee focali

Le linee focali sono rette di riferimento infinite utili per allineare waypoint con elementi del campo (palline, zone di scoring, ecc.).

### Creare una linea focale
1. Seleziona modalità 📐 **Focale**
2. Click punto A → click punto B → la linea viene creata
3. **Tasto destro** durante la creazione annulla il primo punto

### Gestire le linee focali
- Appaiono come linee tratteggiate giallo chiaro sul campo
- Nella sidebar puoi regolare il **raggio di attrazione** (default 15 cm)
- La **checkbox** accanto a ogni linea la nasconde: una linea nascosta non viene disegnata e non fa snap
- Pulsante ✖ per eliminarle

---

## Simulazione

### Avviare la simulazione
1. Crea almeno un segmento
2. Premi **▶ Avvia** nella sidebar
3. Il robot si posiziona sullo Start e segue il percorso

### Timer
- Il timer conta alla rovescia da **30 secondi** (durata match FTC)
- Verde → giallo (< 10s) → rosso (< 5s)
- Allo scadere del timer la simulazione si ferma ovunque sia il robot

### Stati della simulazione
| Indicatore | Stato |
|-----------|-------|
| (nessuno) | Il robot si sta muovendo |
| `↺ ROT 90° → 180°` | Il robot sta ruotando sul posto tra segmenti |
| `⏸ WAIT 1.2s` | Il robot sta aspettando (azione Wait) |
| `🎯 SHOOT+STOP 1.5s` | Il robot è fermo e sta sparando |
| `🎯 SHOOT+MOVE` | Il robot si sta muovendo e sparando: attivo per **tutto** il segmento su cui è impostato (cerchio rosso pulsante sul campo) |

### Tempo stimato
Sopra la sezione Progetto compare `⏱ Tempo stimato` (verde/giallo/rosso rispetto ai 30 s). Usa la stessa cinematica della simulazione, quindi include accelerazione, frenata, rotazioni tra segmenti, Wait e Shoot+Stop.

### Indicatori meccanismi
- **INTAKE** (verde) → intake attivo durante il segmento corrente
- **FLYWHEEL** (arancio) → flywheel attivo

---

## Scorciatoie da tastiera

Elencate anche nel pannello a destra del campo.

| Tasto | Azione |
|-------|--------|
| `1` – `5` | Select / Line / Curva / Barriera / Focale |
| `G` | Griglia + snap griglia ON/OFF |
| `Space` | Play / Stop simulazione |
| `Home` | Robot → Start |
| `Ctrl+Z` | Annulla ultimo segmento |
| `Ctrl+Shift+Z` | Annulla ultimo vertice della barriera aperta |
| `Ctrl+S` / `Ctrl+O` | Salva / Carica progetto |
| `Ctrl+E` | Esporta JSON |
| `Shift+Canc` | Cancella tutto il percorso |
| `Shift + click` | Snap ortho (orizzontale/verticale) |
| `Shift + trascina` (sul robot) | Ruota il robot |
| Tasto destro | Elimina punto / vertice |
| Doppio click | Aggiunge control point alla curva più vicina |

---

## Salvataggio e caricamento

| Bottone | File | Contenuto |
|---------|------|-----------|
| 💾 **Salva** | `progetto.hudproject` | Tutto il progetto (segmenti, barriere, robot, linee focali con visibilità) |
| 📂 **Carica** | `progetto.hudproject` | Ripristina il progetto salvato |
| 📤 **Esporta JSON** | `path.json` | JSON per il robot (+ copia negli appunti) |

---

## Formato JSON esportato

```json
{
  "units": "cm",
  "match_duration": 30,
  "start": { "x": 182.9, "y": 182.9, "heading": 90.0 },
  "segments": [
    {
      "id": 0,
      "type": "line",
      "to": { "x": 182.9, "y": 365.8 },
      "heading": "tangent",
      "robot_heading": {
        "start": 90.0,
        "end": 90.0,
        "rotation_speed": 180.0
      },
      "speed": {
        "v_start": 0.0,
        "v_max": 80.0,
        "v_min": 25.0,
        "accel": 120.0,
        "decel": 200.0,
        "brake_point": 0.75
      },
      "action": "move",
      "intake": false,
      "flywheel": false
    },
    {
      "id": 1,
      "type": "curve",
      "controls": [
        { "x": 182.9, "y": 457.2 },
        { "x": 274.3, "y": 457.2 }
      ],
      "to": { "x": 274.3, "y": 365.8 },
      "heading": { "fixed": 90.0 },
      "robot_heading": {
        "start": 90.0,
        "end": 180.0,
        "rotation_speed": 120.0
      },
      "speed": {
        "v_start": 20.0,
        "v_max": 60.0,
        "v_min": 25.0,
        "accel": 100.0,
        "decel": 150.0,
        "brake_point": 0.6
      },
      "action": { "shoot_stop": 1.5 },
      "intake": true,
      "flywheel": true
    }
  ],
  "barriers": [
    {
      "id": 0,
      "vertices": [
        { "x": 50.0, "y": 50.0 },
        { "x": 150.0, "y": 50.0 },
        { "x": 150.0, "y": 150.0 },
        { "x": 50.0, "y": 150.0 }
      ]
    }
  ]
}
```

### Valori `action`
| Valore | Comportamento |
|--------|---------------|
| `"move"` | Muoviti e basta |
| `{"wait": N}` | Fermati N secondi alla fine del segmento |
| `{"shoot_stop": N}` | Fermati N secondi e spara alla fine del segmento |
| `{"shoot_move": N}` | Spara per tutta la durata del movimento lungo questo segmento |

---

## Note per la lettura del JSON in Java (FTC)

```java
// Esempio di struttura dati consigliata
public class PathSegment {
    public String type;          // "line" | "curve"
    public double[] to;          // {x, y} in cm
    public double[][] controls;  // punti di controllo Bezier (solo curve)
    public RobotHeading robot_heading;
    public SpeedProfile speed;
    public String action;        // o oggetto se ha parametri
    public boolean intake;
    public boolean flywheel;
}

public class SpeedProfile {
    public double v_start;       // cm/s
    public double v_max;         // cm/s
    public double v_min;         // cm/s, minimo in frenata
    public double accel;         // cm/s²
    public double decel;         // cm/s²
    public double brake_point;   // frazione [0,1] della lunghezza del segmento
}

public class RobotHeading {
    public double start;         // gradi
    public double end;           // gradi
    public double rotation_speed;// °/s
}
```

Per le mecanum, usa `robot_heading.start` e `robot_heading.end` per il PID di rotazione, separato dal PID di traslazione che segue le coordinate `to`. Aggiungi le costanti di correzione empiriche per lo slip laterale in un file `MecanumConstants.java` separato.

---

## Struttura del progetto

```
src/
├── main.rs          # Entry point, finestra eframe
├── app.rs           # Stato globale dell'app (HudApp)
├── segment.rs       # Tipi dati: Segment, SpeedProfile, RobotAction, ecc.
├── barrier.rs       # Barriera + geometria (distanza punto-segmento, push_out)
├── simulation.rs    # Macchina a stati della simulazione
├── export.rs        # Serializzazione JSON per il robot
├── project.rs       # Salvataggio/caricamento progetto
├── field.rs         # Costanti campo e conversione coordinate
├── assets.rs        # Caricamento texture e icona
└── ui/
    ├── mod.rs
    ├── side_panel.rs  # Sidebar: modalità, sim, segmenti, export
    └── field_canvas.rs# Campo: disegno, input mouse, snap
assets/
├── field.png        # Immagine del campo FTC
├── icon.png
└── icon.ico
```

---

## Coordinate

Il campo è **365.76 × 365.76 cm** (144 × 144 pollici, campo FTC standard).  
L'origine `(0, 0)` è nell'angolo in basso a sinistra, Y cresce verso l'alto.  
Tutti i valori nel JSON sono in **centimetri**.
