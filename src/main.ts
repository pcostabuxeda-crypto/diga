// Finestra de configuració. Contracte amb Rust a PLA.md.
import { invoke } from "@tauri-apps/api/core";

type Config = {
  drecera: string;
  mode: string;
  idioma: string;
  enganxar: boolean;
  inici_amb_windows: boolean;
  drecera_codi: string;
  llenguatge_codi: string;
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

actual = await invoke<Config>("get_config");
omple(actual);
await refrescaClau();
