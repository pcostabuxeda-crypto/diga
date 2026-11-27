// Rètol flotant «escoltant / transcrivint / endreçant / copiat» amb el text que es va entenent.
// Es pot arrossegar. Escolta `diga://estat` (contracte a PLA.md).
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
const estat = document.querySelector<HTMLDivElement>("#estat")!;
const text = document.querySelector<HTMLDivElement>("#text")!;
let temporitzador: number | undefined;
let colocat = false; // només la primera vegada: després es queda on l'hagis arrossegat

// Whisper torna tot el text cada cop: les paraules noves s'escriuen una a una perquè
// sembli que surten mentre parles. El que Whisper corregeix es canvia de cop.
const PAS_PARAULA = 60; // ms entre paraules
const sensePresses = matchMedia("(prefers-reduced-motion: reduce)");
let paraules: string[] = []; // les que es veuen
let objectiu: string[] = [];
let maquina: number | undefined;

function posaText(nou: string, deCop: boolean) {
  objectiu = nou.split(/\s+/).filter(Boolean);
  let iguals = 0;
  while (iguals < paraules.length && paraules[iguals] === objectiu[iguals]) iguals++;
  paraules = deCop || sensePresses.matches ? [...objectiu] : paraules.slice(0, iguals);
  pinta();
  clearInterval(maquina);
  if (paraules.length < objectiu.length) {
    maquina = window.setInterval(() => {
      paraules.push(objectiu[paraules.length]);
      pinta();
      if (paraules.length >= objectiu.length) clearInterval(maquina);
    }, PAS_PARAULA);
  }
}

function pinta() {
  text.textContent = paraules.join(" ");
  text.scrollTop = text.scrollHeight; // les últimes línies
}

retol.addEventListener("mousedown", (e) => {
  if (e.button === 0) finestra.startDragging();
});

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
  estat.textContent = avis
    ? `Copiat sense endreçar: ${avis}`
    : payload.estat === "error" && payload.error
      ? payload.error
      : TEXTOS[payload.estat];
  retol.title = avis ?? payload.text ?? "";
  // Transcrivint i endreçant mantenen el text d'abans; un dictat nou o un error el buiden.
  // El text final (copiat) surt sencer de cop.
  if (payload.text !== undefined) posaText(payload.text, payload.estat !== "escoltant");
  else if (payload.estat === "escoltant" || payload.estat === "error") posaText("", true);

  if (!colocat) {
    await colocaAbaix();
    colocat = true;
  }
  await finestra.show();

  const ms = avis ? AMAGA_AMB_AVIS : AMAGA_DESPRES[payload.estat];
  if (ms) temporitzador = window.setTimeout(() => finestra.hide(), ms);
});
