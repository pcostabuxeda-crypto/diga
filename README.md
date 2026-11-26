# Diga

Prems una drecera, parles, i el text queda copiat al porta-retalls.
App d'escriptori per a Windows: Tauri 2 + Rust + Whisper local.

Estat: Fase 1 (nucli de veu). Vegeu [PLA.md](PLA.md).

Per compilar a Windows cal:
- **LLVM** (`winget install LLVM.LLVM`): whisper-rs necessita libclang.
- **CMake**: al PATH, o la variable `CMAKE` apuntant al `cmake.exe` de Visual Studio.

```
npm install
npm run tauri dev
```

Drecera per defecte: mantén `Ctrl+Alt+D` mentre parles.
