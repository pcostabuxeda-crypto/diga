# Diga — pla

Prems una drecera, parles, i el text queda copiat (i, si vols, enganxat on tens el cursor).

## Decisions (26 nov 2026)

| Tema | Decisió | Per què |
|---|---|---|
| Nom | **Diga** | Curt, català, com Ona i Nova |
| App | **Tauri 2 + Rust** | Exe petit, safata, drecera global i instal·lador NSIS fets. Mateix stack que Ona i Trainer Lab |
| UI | HTML + TypeScript amb Vite (plantilla `vanilla-ts`) | Només hi ha config i un rètol «escoltant»: no cal framework |
| Transcripció | **Whisper local** (`whisper-rs`, whisper.cpp) | Gratis, privat, sense internet, bon català |
| Màquina objectiu | **Portàtil i5 10a gen, 16 GB, sense GPU**, amb Claude i Docker oberts | Si va bé aquí, va bé a qualsevol PC de client |
| Model | `ggml-small-q5_1.bin` (~190 MB) a la CPU, com a molt 4 fils | Mesurat: 0,8–1 s per frase i ~315 MB de RAM al PC de proves. Es baixa a `%APPDATA%\cat.diga.app\models` |
| Repòs | El model es descarrega de memòria després de 5 min sense dictar | No ha de molestar jocs ni altres apps |
| GPU | No, de moment | CUDA només va amb NVIDIA. Si mai cal, Vulkan (qualsevol GPU) |
| Dictat més llest (local) | Pista d'estil + vocabulari propi a Whisper (`initial_prompt`); greedy, no beam | Mesurat: el vocabulari encerta termes sense cost; el beam 5 era +50% de temps per guanyar poc |
| Mode net (opcional) | **Claude Haiku 5.5** per API d'Anthropic (HTTP directe, `net.rs`): corregeix pel context, puntuació i ordres dictades | El portàtil no gasta RAM ni CPU, ~1 s i cost ínfim. Si falla, es copia el text del Whisper. Només s'envia el text, mai l'àudio |
| Mode codi | Aparcat | Primer el dictat normal |
| Git | GitHub Flow: branques `feature/`, PR, squash | [METODOLOGIA.md](../METODOLOGIA.md) |

## Com funciona

```
drecera premuda ──► grava micro (cpal, 16 kHz mono)
drecera deixada ──► whisper (idioma: ca, o auto) ──► porta-retalls
                                                  └─► Ctrl+V (opcional, enigo)
```

- Mode: **mantenir** la drecera mentre parles. (Prémer per començar i per acabar: Fase 3.)
- Drecera per defecte: `Ctrl+Alt+D` (configurable). `Ctrl+Alt+Espai` ja la fa servir una altra app en aquest PC. `Win+H` és el dictat de Windows: no el trepitgem.
- Viu a la safata del sistema. Mostra un rètol a sobre de la barra de tasques (es pot arrossegar): escoltant → transcrivint → copiat, amb el text que va entenent mentre parles.

## Estructura

```
diga/
├─ src/                      UI (B)
│  ├─ main.ts                finestra de configuració
│  └─ retol.ts               rètol «escoltant / transcrivint / copiat»
├─ src-tauri/
│  ├─ src/
│  │  ├─ main.rs             només crida lib::run (no tocar)
│  │  ├─ lib.rs              arrencada, plugins, drecera i ordres (A; B hi afegeix les seves línies avisant)
│  │  ├─ safata.rs           icona i menú de la safata (B)
│  │  ├─ audio.rs            captura del micro (A)
│  │  ├─ transcriure.rs      whisper (A)
│  │  ├─ sortida.rs          porta-retalls + enganxar (A)
│  │  ├─ config.rs           llegir/desar config (B)
│  │  └─ models.rs           baixar el model amb progrés (B)
│  └─ tauri.conf.json
├─ PLA.md                    aquest fitxer (a git)
└─ connexio.md               tauler entre A i B (fora de git)
```

## Contracte entre Rust i la UI

Així A i B poden treballar alhora sense esperar-se.

**Esdeveniments** (Rust → UI)
- `diga://estat` → `{ estat: "escoltant" | "transcrivint" | "endreçant" | "copiat" | "error", text?: string, error?: string, avis?: string }`
  - `escoltant` amb `text`: el que s'ha entès fins ara (es retranscriu tot l'àudio contínuament mentre es parla; el rètol hi fa aparèixer les paraules noves una a una).
  - `endreçant`: el mode net està passant el text per Claude.
  - `avis` (amb `copiat`): el mode net ha fallat i s'ha copiat el text del Whisper tal qual.
- `diga://progres-model` → `{ baixat: number, total: number }`

**Ordres** (UI → Rust)
- `get_config() -> Config`
- `set_config(config: Config)` — torna a registrar les dreceres que hagin canviat; si una falla, no desa res
- `te_clau_api() -> bool` — la clau mai viatja cap a la UI
- `set_clau_api(clau: string)` — la desa al Gestor de credencials de Windows (`cat.diga.app` / `anthropic-api`); buida = esborrar-la
- `descarregar_model() -> ()` — envia `diga://progres-model`
- `model_present() -> bool`

**Des de Rust**: `config::carregar(app) -> Config` i `config::clau_api() -> Option<String>` (per a `codi.rs`).

**Config** (`%APPDATA%\cat.diga.app\config.json`, amb `tauri-plugin-store`, clau `"config"`)
```json
{ "drecera": "Ctrl+Alt+D", "mode": "mantenir", "idioma": "ca", "enganxar": false, "inici_amb_windows": false,
  "drecera_codi": "Ctrl+Alt+C", "llenguatge_codi": "JavaScript", "micro": "",
  "vocabulari": "", "mode_net": false }
```

## Fases

**Fase 0 — esquelet** (A, sol, abans que B comenci)
- `create-tauri-app` amb `vanilla-ts`, plugins: global-shortcut, clipboard-manager, store.
- CI verd. Crear el worktree de B.

**Fase 1 — MVP** (A i B en paral·lel)
- A: drecera mantinguda → grava → whisper CPU → porta-retalls. Prova: dir una frase i enganxar-la al Bloc de notes.
- B: safata (Configuració / Sortir), rètol d'estat, finestra de config, descàrrega del model.

**Fase 1b — mode codi**
- A: `codi.rs`: text + llenguatge → Claude Haiku 5.5 → codi al porta-retalls (segona drecera).
- B: a la config, llenguatge, segona drecera i clau d'API (Gestor de credencials de Windows).

**Fase 2 — fer-la de veritat**
- Auto-enganxar (Ctrl+V), inici amb Windows, instal·lador NSIS amb icona.

**Fase 3 — només si cal**
- Historial dels últims textos, tria de model, mode «prémer i prémer».

## Riscos

- **Micròfon**: Windows ha de permetre'l a les apps d'escriptori (Configuració → Privadesa → Micròfon).
- **Al·lucinacions**: amb silenci o soroll, Whisper s'inventa text. Ara el VAD Silero (`resources/ggml-silero-v5.1.2.bin`, 885 KB, dins l'app) li passa només els trossos amb veu; si no n'hi ha, s'avisa. Si el volum és gairebé zero, s'avisa abans (micro silenciat). Mesurat: amb 5 s de soroll, sense VAD inventava una frase; amb VAD avisa, i una frase amb 6 s de soroll al voltant passa de 2 s a 1,1 s.
- **Compilar a Windows**: cal LLVM (libclang) i CMake. Sense el `/O2` de `src-tauri/.cargo/config.toml`, whisper.cpp va ~10 vegades més lent.
- **Enganxar**: algunes apps elevades (administrador) no reben el Ctrl+V simulat. El text igualment queda copiat.
