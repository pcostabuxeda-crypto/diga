// Rètol flotant «escoltant / transcrivint / endreçant / copiat». Escolta `diga://estat` (contracte a PLA.md).
import { listen } from "@tauri-apps/api/event";
import { currentMonitor, getCurrentWindow, PhysicalPosition, primaryMonitor } from "@tauri-apps/api/window";

type Estat = {
  estat: "escoltant" | "transcrivint" | "endreçant" | "copiat" | "error";
  text?: string;
  error?: string;
  avis?: string; // amb `copiat`: el mode net ha fallat i s'ha copiat el text del Whisper
};

const TEXTOS: Record<Estat["estat"], string> = {
  escoltant: "Escoltant…",
  transcrivint: "Transcrivint…",
  endreçant: "Endreçant…",
  copiat: "Copiat",
  error: "Error",
};
const AMAGA_DESPRES: Partial<Record<Estat["estat"], number>> = { copiat: 1500, error: 4000 };
const AMAGA_AMB_AVIS = 4000; // que es pugui llegir, com un error
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
  const avis = payload.estat === "copiat" ? payload.avis : undefined;
  retol.dataset.estat = avis ? "avis" : payload.estat;
  retol.textContent = avis
    ? `Copiat sense endreçar: ${avis}`
    : payload.estat === "error" && payload.error
      ? payload.error
      : TEXTOS[payload.estat];
  retol.title = avis ?? payload.text ?? "";

  await colocaAbaix();
  await finestra.show();

  const ms = avis ? AMAGA_AMB_AVIS : AMAGA_DESPRES[payload.estat];
  if (ms) temporitzador = window.setTimeout(() => finestra.hide(), ms);
});
