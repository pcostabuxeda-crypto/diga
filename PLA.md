# Diga — pla

Prems una drecera, parles, i el text queda copiat (i, si vols, enganxat on tens el cursor).

## Decisions (26 nov 2026)

| Tema | Decisió | Per què |
|---|---|---|
| Nom | **Diga** | Curt, català, com Ona i Nova |
| App | **Tauri 2 + Rust** | Exe petit, safata, drecera global i instal·lador NSIS fets. Mateix stack que Ona i Trainer Lab |
| UI | HTML + TypeScript amb Vite (plantilla `vanilla-ts`) | Només hi ha config i un rètol «escoltant»: no cal framework |
| Transcripció | **Whisper local** (`whisper-rs`, whisper.cpp) | Gratis, privat, sense internet, bon català |
| Model | `ggml-large-v3-turbo-q5_0.bin` (~550 MB) | Millor relació qualitat/velocitat. Es baixa el primer cop a `%APPDATA%\diga\models` |
| GPU | CPU a la Fase 1, CUDA (RTX 5070) a la Fase 2 | Al PC no hi ha CUDA Toolkit (`nvcc`); el necessitem per compilar amb GPU |
| Git | GitHub Flow: branques `feature/`, PR, squash | [METODOLOGIA.md](../METODOLOGIA.md) |

## Com funciona

```
drecera premuda ──► grava micro (cpal, 16 kHz mono)
drecera deixada ──► whisper (idioma: ca, o auto) ──► porta-retalls
                                                  └─► Ctrl+V (opcional, enigo)
```

- Mode: **mantenir** la drecera mentre parles. (Prémer per començar i per acabar: Fase 3.)
- Drecera per defecte: `Ctrl+Alt+Espai` (configurable). `Win+H` és el dictat de Windows: no el trepitgem.
- Viu a la safata del sistema. Mostra un rètol petit: escoltant → transcrivint → copiat.

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
- `diga://estat` → `{ estat: "escoltant" | "transcrivint" | "copiat" | "error", text?: string, error?: string }`
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
{ "drecera": "Ctrl+Alt+Space", "mode": "mantenir", "idioma": "ca", "enganxar": false, "inici_amb_windows": false,
  "drecera_codi": "Ctrl+Alt+C", "llenguatge_codi": "JavaScript" }
```

## Fases

**Fase 0 — esquelet** (A, sol, abans que B comenci)
- `create-tauri-app` amb `vanilla-ts`, plugins: global-shortcut, clipboard-manager, store.
- CI verd. Crear el worktree de B.

**Fase 1 — MVP** (A i B en paral·lel)
- A: drecera mantinguda → grava → whisper CPU → porta-retalls. Prova: dir una frase i enganxar-la al Bloc de notes.
- B: safata (Configuració / Sortir), rètol d'estat, finestra de config, descàrrega del model.

**Fase 2 — fer-la de veritat**
- GPU: instal·lar CUDA Toolkit 12.8+ (Blackwell), feature `cuda` a `whisper-rs`, empaquetar les DLL de CUDA.
- Auto-enganxar (Ctrl+V), inici amb Windows, instal·lador NSIS amb icona.

**Fase 3 — només si cal**
- Historial dels últims textos, tria de model, mode «prémer i prémer».

## Riscos

- **Micròfon**: Windows ha de permetre'l a les apps d'escriptori (Configuració → Privadesa → Micròfon).
- **CUDA**: l'exe amb GPU pesa més (DLL de CUDA ~500 MB). Alternativa: backend Vulkan de whisper.cpp.
- **Enganxar**: algunes apps elevades (administrador) no reben el Ctrl+V simulat. El text igualment queda copiat.
