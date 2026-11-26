// Finestra de configuració. Contracte amb Rust a PLA.md.
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

type Config = {
  drecera: string;
  mode: string;
  idioma: string;
  enganxar: boolean;
  inici_amb_windows: boolean;
  drecera_codi: string;
  llenguatge_codi: string;
  micro: string;
};

const formConfig = document.querySelector<HTMLFormElement>("#config")!;
const formClau = document.querySelector<HTMLFormElement>("#clau")!;
const clauEstat = document.querySelector<HTMLParagraphElement>("#clau-estat")!;
const missatge = document.querySelector<HTMLParagraphElement>("#missatge")!;
let actual: Config;

function avisa(text: string, error = false) {
  missatge.textContent = text;
  missatge.classList.toggle("error", error);
}

function camp(nom: string) {
  return formConfig.elements.namedItem(nom) as HTMLInputElement;
}

function omple(config: Config) {
  for (const [nom, valor] of Object.entries(config)) {
    const el = camp(nom);
    if (!el) continue; // `mode` encara no té control (Fase 3)
    if (el.type === "checkbox") el.checked = valor as boolean;
    else el.value = valor as string;
  }
}

function llegeix(): Config {
  return {
    ...actual,
    drecera: camp("drecera").value.trim(),
    idioma: camp("idioma").value,
    micro: camp("micro").value,
    enganxar: camp("enganxar").checked,
    inici_amb_windows: camp("inici_amb_windows").checked,
    drecera_codi: camp("drecera_codi").value.trim(),
    llenguatge_codi: camp("llenguatge_codi").value.trim(),
  };
}

async function refrescaClau() {
  const te = await invoke<boolean>("te_clau_api");
  clauEstat.textContent = te ? "Hi ha una clau desada." : "No hi ha cap clau desada: el mode codi no funcionarà.";
}

async function desaClau(clau: string, fet: string) {
  try {
    await invoke("set_clau_api", { clau });
    formClau.reset();
    avisa(fet);
    await refrescaClau();
  } catch (e) {
    avisa(String(e), true);
  }
}

formConfig.addEventListener("submit", async (e) => {
  e.preventDefault();
  const nova = llegeix();
  try {
    await invoke("set_config", { config: nova });
    actual = nova;
    avisa("Configuració desada.");
  } catch (err) {
    avisa(String(err), true);
  }
});

formClau.addEventListener("submit", (e) => {
  e.preventDefault();
  const clau = (formClau.elements.namedItem("clau") as HTMLInputElement).value;
  if (clau.trim()) desaClau(clau, "Clau desada.");
});

document.querySelector("#esborra-clau")!.addEventListener("click", () => desaClau("", "Clau esborrada."));

// --- Model de veu ---
const modelEstat = document.querySelector<HTMLParagraphElement>("#model-estat")!;
const modelProgres = document.querySelector<HTMLProgressElement>("#model-progres")!;
const baixaModel = document.querySelector<HTMLButtonElement>("#baixa-model")!;
const MB = (bytes: number) => Math.round(bytes / 1e6);

async function refrescaModel() {
  const hi_es = await invoke<boolean>("model_present");
  modelEstat.textContent = hi_es ? "El model ja està baixat." : "Falta el model: sense ell Diga no pot transcriure.";
  baixaModel.hidden = hi_es;
}

listen<{ baixat: number; total: number }>("diga://progres-model", ({ payload: { baixat, total } }) => {
  modelProgres.hidden = false;
  if (total > 0) {
    modelProgres.max = total;
    modelProgres.value = baixat;
    modelEstat.textContent = `Baixant… ${MB(baixat)} de ${MB(total)} MB`;
  } else {
    modelProgres.removeAttribute("value"); // mida desconeguda: barra indeterminada
    modelEstat.textContent = `Baixant… ${MB(baixat)} MB`;
  }
});

baixaModel.addEventListener("click", async () => {
  baixaModel.disabled = true;
  modelEstat.textContent = "Connectant…";
  try {
    await invoke("descarregar_model");
    avisa("Model baixat.");
  } catch (e) {
    avisa(String(e), true);
  } finally {
    baixaModel.disabled = false;
    modelProgres.hidden = true;
    await refrescaModel();
  }
});

// Abans d'omplir: el <select> ha de tenir l'opció del micro desat, encara que ara no hi sigui.
async function omplMicros(desat: string) {
  const select = camp("micro") as unknown as HTMLSelectElement;
  const micros = await invoke<string[]>("llista_micros");
  for (const nom of micros) select.add(new Option(nom, nom));
  // Desendollat: es manté triat perquè no es perdi en desar.
  if (desat && !micros.includes(desat)) select.add(new Option(`${desat} (no connectat)`, desat));
}

actual = await invoke<Config>("get_config");
await omplMicros(actual.micro);
omple(actual);
await refrescaClau();
await refrescaModel();
