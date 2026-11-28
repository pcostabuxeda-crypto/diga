use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use tauri::path::BaseDirectory;
use tauri::{AppHandle, Manager};
use whisper_rs::{
    FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters, WhisperVadContext,
    WhisperVadContextParams, WhisperVadParams,
};

use crate::audio::HZ_WHISPER;

/// Nom del model. `models.rs` (B) el baixa a `cami_model`.
// Petit perquè vagi a la CPU d'un portàtil (i5 10a gen) amb altres apps obertes.
pub const MODEL: &str = "ggml-small-q5_1.bin";

pub fn cami_model(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    Ok(dir.join("models").join(MODEL))
}

/// Model Silero de detecció de veu (VAD). Va dins l'app: fa 885 KB.
const VAD: &str = "resources/ggml-silero-v5.1.2.bin";

pub fn cami_vad(app: &AppHandle) -> Result<PathBuf, String> {
    app.path().resolve(VAD, BaseDirectory::Resource).map_err(|e| e.to_string())
}

/// Temps sense dictar abans d'alliberar el model i la memòria: Diga no ha de molestar la resta d'apps.
const REPOS: Duration = Duration::from_secs(5 * 60);

struct Carregat {
    ctx: WhisperContext,
    /// `None` si no s'ha pogut carregar: llavors es transcriu tot l'àudio, com abans.
    vad: Option<WhisperVadContext>,
    ultim_us: Instant,
}

static CARREGAT: Mutex<Option<Carregat>> = Mutex::new(None);

/// Text que Whisper pren com a «el que s'ha dit abans»: li marca l'estil (puntuació,
/// majúscules) i li fa conèixer les paraules pròpies de l'usuari.
pub fn pista(idioma: &str, vocabulari: &str) -> String {
    let estil = match idioma {
        "ca" => "Bon dia. Aquesta és una transcripció en català, amb comes, punts i majúscules ben posats.",
        "es" => "Buenos días. Esta es una transcripción en castellano, con comas, puntos y mayúsculas bien puestos.",
        "en" => "Hello. This is an English transcription, with proper commas, periods and capital letters.",
        _ => "",
    };
    let paraules: Vec<&str> = vocabulari
        .split([',', '\n'])
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .collect();
    if paraules.is_empty() {
        return estil.to_string();
    }
    format!("{estil} {}.", paraules.join(", ")).trim().to_string()
}

pub fn transcriure<F: FnMut() -> bool + 'static>(
    cami: &Path,
    cami_vad: &Path,
    audio: &[f32],
    idioma: &str,
    vocabulari: &str,
    parar: F,
) -> Result<String, String> {
    // Menys de 0,3 s és una pulsació sense voler.
    if audio.len() < HZ_WHISPER as usize * 3 / 10 {
        return Err("Massa curt: mantén la drecera mentre parles".into());
    }
    // Micro silenciat o sense permís: un missatge més clar que el del VAD.
    if crate::audio::volum(audio) < 0.001 {
        return Err("No s'ha sentit res: mira que el micro no estigui silenciat".into());
    }

    let mut carregat = CARREGAT.lock().unwrap_or_else(|e| e.into_inner());
    if carregat.is_none() {
        if !cami.exists() {
            return Err("Falta el model de veu. Baixa'l des de la configuració.".into());
        }
        let mut params = WhisperContextParameters::default();
        params.flash_attn(true); // menys memòria i més ràpid
        let ctx = WhisperContext::new_with_params(cami, params)
            .map_err(|e| format!("No s'ha pogut carregar el model: {e}"))?;
        let vad = WhisperVadContext::new(&cami_vad.to_string_lossy(), WhisperVadContextParams::new())
            .inspect_err(|e| eprintln!("diga: sense VAD ({}): {e}", cami_vad.display()))
            .ok();
        *carregat = Some(Carregat { ctx, vad, ultim_us: Instant::now() });
        vigilar_repos();
    }
    let c = carregat.as_mut().unwrap();

    // Amb silenci o soroll, Whisper s'inventa text: només li passem els trossos amb veu.
    let mut audio = match c.vad.as_mut() {
        None => audio.to_vec(),
        Some(vad) => {
            let trams: Vec<(f32, f32)> = vad
                .segments_from_samples(WhisperVadParams::new(), audio)
                .map_err(|e| format!("VAD: {e}"))?
                .map(|t| (t.start, t.end))
                .collect();
            let veu = nomes_veu(audio, &trams);
            if veu.is_empty() {
                return Err("No s'ha sentit cap paraula".into());
            }
            veu
        }
    };
    // Whisper vol com a mínim 1 s: s'omple amb silenci.
    audio.resize(audio.len().max(HZ_WHISPER as usize * 11 / 10), 0.0);

    let mut state = c.ctx.create_state().map_err(|e| e.to_string())?;
    // Greedy: el beam search (5) era un 50% més lent per guanyar molt poc (mesurat).
    let mut p = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
    p.set_language(Some(idioma));
    let pista = pista(idioma, vocabulari);
    if !pista.is_empty() {
        p.set_initial_prompt(&pista);
    }
    // Com a molt 4 fils: la resta de la CPU queda per a les altres apps.
    p.set_n_threads(std::thread::available_parallelism().map_or(4, |n| n.get().min(4) as i32));
    // Whisper processa 30 s d'àudio encara que en diguis 5: retallar la finestra
    // (50 posicions per segon + marge) fa la passada molt més ràpida.
    let segons = audio.len() as f32 / HZ_WHISPER as f32;
    p.set_audio_ctx(((segons * 50.0) as i32 + 64).min(1500));
    p.set_no_context(true);
    p.set_suppress_nst(true); // sense «# (mòbil)», «[Música]» i similars
    p.set_print_special(false);
    p.set_print_progress(false);
    p.set_print_realtime(false);
    p.set_print_timestamps(false);
    p.set_abort_callback_safe::<_, F>(Some(parar));
    state.full(p, &audio).map_err(|e| e.to_string())?;
    c.ultim_us = Instant::now();

    let text: String = state.as_iter().map(|s| s.to_string()).collect();
    let text = text.trim().to_string();
    let text = crate::text::aplicar_vocabulari(&text, vocabulari);
    let text = crate::text::netejar_falques(&text, idioma);
    Ok(text)
}

/// Uneix els trams amb veu (en centisegons, com els dona el VAD) amb 0,1 s de silenci
/// entre ells, perquè Whisper no hi enganxi paraules.
fn nomes_veu(audio: &[f32], trams: &[(f32, f32)]) -> Vec<f32> {
    let mostra = |cs: f32| ((cs * HZ_WHISPER as f32 / 100.0) as usize).min(audio.len());
    let mut veu = Vec::new();
    for &(inici, fi) in trams {
        let (a, b) = (mostra(inici), mostra(fi));
        if a >= b {
            continue;
        }
        if !veu.is_empty() {
            veu.resize(veu.len() + HZ_WHISPER as usize / 10, 0.0);
        }
        veu.extend_from_slice(&audio[a..b]);
    }
    veu
}

/// Allibera el model quan fa `REPOS` que no es fa servir. El fil s'acaba en alliberar-lo.
fn vigilar_repos() {
    std::thread::spawn(|| loop {
        std::thread::sleep(Duration::from_secs(30));
        let mut carregat = CARREGAT.lock().unwrap_or_else(|e| e.into_inner());
        if carregat.as_ref().is_none_or(|c| c.ultim_us.elapsed() >= REPOS) {
            *carregat = None;
            return;
        }
    });
}

#[cfg(test)]
mod tests {
    #[test]
    fn nomes_veu_uneix_els_trams_amb_silenci() {
        let audio: Vec<f32> = (0..32_000).map(|i| i as f32).collect(); // 2 s
        // De 0,1 a 0,2 s i de 1,5 s fins més enllà del final.
        let veu = super::nomes_veu(&audio, &[(10.0, 20.0), (150.0, 999.0)]);
        assert_eq!(veu.len(), 1_600 + 1_600 + 8_000);
        assert_eq!(veu[0], 1_600.0);
        assert_eq!(veu[1_600], 0.0);
        assert_eq!(veu[3_200], 24_000.0);
        assert!(super::nomes_veu(&audio, &[]).is_empty());
    }

    #[test]
    fn la_pista_porta_estil_i_vocabulari() {
        let p = super::pista("ca", " Docker, Claude\npcosta-client ,, ");
        assert!(p.starts_with("Bon dia."));
        assert!(p.ends_with(" Docker, Claude, pcosta-client."));
        assert_eq!(super::pista("auto", ""), "");
        assert_eq!(super::pista("auto", "Diga"), "Diga.");
    }

    /// Prova amb el model de veritat (WAV 16 kHz mono 16 bits):
    /// `DIGA_MODEL=… DIGA_WAV=… [DIGA_IDIOMA=es] [DIGA_VAD=…] cargo test -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn transcriu_un_wav() {
        let model = std::env::var("DIGA_MODEL").unwrap();
        let wav = std::fs::read(std::env::var("DIGA_WAV").unwrap()).unwrap();
        let audio: Vec<f32> = wav[44..]
            .chunks_exact(2)
            .map(|b| i16::from_le_bytes([b[0], b[1]]) as f32 / 32768.0)
            .collect();
        let idioma = std::env::var("DIGA_IDIOMA").unwrap_or("ca".into());
        let vocabulari = std::env::var("DIGA_VOCABULARI").unwrap_or_default();
        let vad = std::env::var("DIGA_VAD").unwrap_or("resources/ggml-silero-v5.1.2.bin".into());
        // Dues passades: la primera inclou carregar el model.
        let mut text = String::new();
        for _ in 0..2 {
            let inici = std::time::Instant::now();
            text = super::transcriure(model.as_ref(), vad.as_ref(), &audio, &idioma, &vocabulari, || false).unwrap();
            println!("[{:.1} s] {text}", inici.elapsed().as_secs_f32());
        }
        assert!(!text.is_empty());
    }
}
