# Diga

Prems una drecera, parles, i el text queda copiat al porta-retalls.
App d'escriptori per a Windows: Tauri 2 + Rust + Whisper local.

Estat: Fase 2 (text en viu, VAD, enganxar, inici amb Windows i instal·lador). Vegeu [PLA.md](PLA.md).

Per compilar a Windows cal:
- **LLVM** (`winget install LLVM.LLVM`): whisper-rs necessita libclang.
- **CMake**: al PATH, o la variable `CMAKE` apuntant al `cmake.exe` de Visual Studio.

```
npm install
npm run tauri dev
```

Instal·lador (per a l'usuari, sense permisos d'administrador, en català):

```
npm run tauri build
```

Surt a `src-tauri/target/release/bundle/nsis/Diga_<versió>_x64-setup.exe` (~3 MB). El model de veu es baixa en obrir l'app el primer cop.
La icona es genera des de `src-tauri/app-icon.svg` amb `npx tauri icon src-tauri/app-icon.svg` (després esborra `icons/android`, `icons/ios` i `icons/64x64.png`).

Drecera per defecte: mantén `Ctrl+Alt+D` mentre parles.
