use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use tauri::{AppHandle, Manager};
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

use crate::audio::HZ_WHISPER;

/// Nom del model. `models.rs` (B) el baixa a `cami_model`.
// Petit perquè vagi a la CPU d'un portàtil (i5 10a gen) amb altres apps obertes.
pub const MODEL: &str = "ggml-small-q5_1.bin";

pub fn cami_model(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    Ok(dir.join("models").join(MODEL))
}

/// Temps sense dictar abans d'alliberar el model i la memòria: Diga no ha de molestar la resta d'apps.
const REPOS: Duration = Duration::from_secs(5 * 60);

struct Carregat {
    ctx: WhisperContext,
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
    audio: &[f32],
    idioma: &str,
    vocabulari: &str,
    parar: F,
) -> Result<String, String> {
    // Menys de 0,3 s és una pulsació sense voler.
    if audio.len() < HZ_WHISPER as usize * 3 / 10 {
        return Err("Massa curt: mantén la drecera mentre parles".into());
    }
    // Amb silenci, Whisper s'inventa text: millor avisar.
    // shortcut: llindar fix pensat per a micro silenciat o sense permís; si amb ús real
    // encara al·lucina amb soroll de fons, calibrar-lo o posar-hi VAD (whisper-rs en porta).
    if crate::audio::volum(audio) < 0.001 {
        return Err("No s'ha sentit res: mira que el micro no estigui silenciat".into());
    }
    // Whisper vol com a mínim 1 s: s'omple amb silenci.
    let mut audio = audio.to_vec();
    audio.resize(audio.len().max(HZ_WHISPER as usize * 11 / 10), 0.0);

    let mut carregat = CARREGAT.lock().unwrap_or_else(|e| e.into_inner());
    if carregat.is_none() {
        if !cami.exists() {
            return Err("Falta el model de veu. Baixa'l des de la configuració.".into());
        }
        let mut params = WhisperContextParameters::default();
        params.flash_attn(true); // menys memòria i més ràpid
        let ctx = WhisperContext::new_with_params(cami, params)
            .map_err(|e| format!("No s'ha pogut carregar el model: {e}"))?;
        *carregat = Some(Carregat { ctx, ultim_us: Instant::now() });
        vigilar_repos();
    }
    let c = carregat.as_mut().unwrap();

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
    Ok(text.trim().to_string())
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
    fn la_pista_porta_estil_i_vocabulari() {
        let p = super::pista("ca", " Docker, Claude\npcosta-client ,, ");
        assert!(p.starts_with("Bon dia."));
        assert!(p.ends_with(" Docker, Claude, pcosta-client."));
        assert_eq!(super::pista("auto", ""), "");
        assert_eq!(super::pista("auto", "Diga"), "Diga.");
    }

    /// Prova amb el model de veritat (WAV 16 kHz mono 16 bits):
    /// `DIGA_MODEL=… DIGA_WAV=… [DIGA_IDIOMA=es] cargo test -- --ignored --nocapture`
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
        // Dues passades: la primera inclou carregar el model.
        let mut text = String::new();
        for _ in 0..2 {
            let inici = std::time::Instant::now();
            text = super::transcriure(model.as_ref(), &audio, &idioma, &vocabulari, || false).unwrap();
            println!("[{:.1} s] {text}", inici.elapsed().as_secs_f32());
        }
        assert!(!text.is_empty());
    }
}
