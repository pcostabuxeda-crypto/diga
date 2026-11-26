// Rètol flotant «escoltant / transcrivint / copiat». Escolta `diga://estat` (contracte a PLA.md).
import { listen } from "@tauri-apps/api/event";
import { currentMonitor, getCurrentWindow, PhysicalPosition, primaryMonitor } from "@tauri-apps/api/window";

type Estat = {
  estat: "escoltant" | "transcrivint" | "copiat" | "error";
  text?: string;
  error?: string;
};

const TEXTOS: Record<Estat["estat"], string> = {
  escoltant: "Escoltant…",
  transcrivint: "Transcrivint…",
  copiat: "Copiat",
  error: "Error",
};
const AMAGA_DESPRES: Partial<Record<Estat["estat"], number>> = { copiat: 1500, error: 4000 };
const MARGE_INFERIOR = 24; // px lògics per sobre de la barra de tasques

const finestra = getCurrentWindow();
const retol = document.querySelector<HTMLDivElement>("#retol")!;
let temporitzador: number | undefined;

// A baix al mig de l'àrea de treball (sense la barra de tasques).
async function colocaAbaix() {
  const monitor = (await currentMonitor()) ?? (await primaryMonitor());
  if (!monitor) return;
  const { position, size } = monitor.workArea;
  const mida = await finestra.outerSize();
  const x = position.x + (size.width - mida.width) / 2;
  const y = position.y + size.height - mida.height - MARGE_INFERIOR * monitor.scaleFactor;
  await finestra.setPosition(new PhysicalPosition(Math.round(x), Math.round(y)));
}

listen<Estat>("diga://estat", async ({ payload }) => {
  clearTimeout(temporitzador);
  retol.dataset.estat = payload.estat;
  retol.textContent = payload.estat === "error" && payload.error ? payload.error : TEXTOS[payload.estat];
  retol.title = payload.text ?? "";

  await colocaAbaix();
  await finestra.show();

  const ms = AMAGA_DESPRES[payload.estat];
  if (ms) temporitzador = window.setTimeout(() => finestra.hide(), ms);
});
