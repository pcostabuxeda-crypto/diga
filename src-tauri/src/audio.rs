use std::sync::{Arc, Mutex, Weak, mpsc};
use std::thread::JoinHandle;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, Sample, SampleFormat, SizedSample};

pub const HZ_WHISPER: u32 = 16_000;

/// Gravació en curs. El `Stream` de cpal no és `Send`: viu dins el seu fil.
pub struct Gravacio {
    aturar: mpsc::Sender<()>,
    fil: JoinHandle<Vec<f32>>,
    en_curs: EnCurs,
}

/// L'àudio mentre es grava, per anar-lo transcrivint. Mor quan s'atura la gravació.
#[derive(Clone)]
pub struct EnCurs {
    mostres: Weak<Mutex<Vec<f32>>>,
    hz: u32,
    canals: u16,
}

impl EnCurs {
    pub fn viu(&self) -> bool {
        self.mostres.strong_count() > 0
    }

    /// El que s'ha gravat fins ara, a 16 kHz mono. `None` si ja s'ha aturat.
    pub fn audio(&self) -> Option<Vec<f32>> {
        let m = self.mostres.upgrade()?;
        let v = m.lock().ok()?.clone();
        Some(a_16k_mono(&v, self.canals, self.hz))
    }
}

/// Noms dels micròfons connectats, per triar-ne un a la configuració.
#[tauri::command]
pub fn llista_micros() -> Vec<String> {
    cpal::default_host()
        .input_devices()
        .map(|ds| ds.filter_map(|d| nom(&d)).collect())
        .unwrap_or_default()
}

fn nom(d: &cpal::Device) -> Option<String> {
    d.description().ok().map(|x| x.name().to_string())
}

/// `micro` buit = el micro per defecte de Windows. Si el triat no hi és (desconnectat),
/// també es fa servir el de Windows.
pub fn comencar(micro: String) -> Result<Gravacio, String> {
    let (aturar, rebre_aturar) = mpsc::channel();
    let (llest, rebre_llest) = mpsc::channel();
    let fil = std::thread::spawn(move || {
        let (stream, mostres, hz, canals) = match obrir(&micro) {
            Ok(v) => {
                let _ = llest.send(Ok(EnCurs { mostres: Arc::downgrade(&v.1), hz: v.2, canals: v.3 }));
                v
            }
            Err(e) => {
                let _ = llest.send(Err(e));
                return Vec::new();
            }
        };
        let _ = rebre_aturar.recv();
        drop(stream);
        let mostres = std::mem::take(&mut *mostres.lock().unwrap());
        a_16k_mono(&mostres, canals, hz)
    });
    let en_curs = rebre_llest
        .recv()
        .map_err(|_| "El fil d'àudio ha caigut".to_string())??;
    Ok(Gravacio { aturar, fil, en_curs })
}

impl Gravacio {
    pub fn en_curs(&self) -> EnCurs {
        self.en_curs.clone()
    }

    /// Atura el micro i torna l'àudio a 16 kHz mono, com el vol Whisper.
    pub fn aturar(self) -> Result<Vec<f32>, String> {
        let _ = self.aturar.send(());
        self.fil.join().map_err(|_| "El fil d'àudio ha petat".to_string())
    }
}

type Obert = (cpal::Stream, Arc<Mutex<Vec<f32>>>, u32, u16);

fn obrir(micro: &str) -> Result<Obert, String> {
    let host = cpal::default_host();
    let triat = match micro {
        "" => None,
        _ => host
            .input_devices()
            .ok()
            .and_then(|mut ds| ds.find(|d| nom(d).as_deref() == Some(micro))),
    };
    if triat.is_none() && !micro.is_empty() {
        eprintln!("diga: no trobo el micro «{micro}», faig servir el de Windows");
    }
    let device = triat
        .or_else(|| host.default_input_device())
        .ok_or("No hi ha cap micròfon")?;
    let config = device.default_input_config().map_err(|e| e.to_string())?;
    let (hz, canals, format) = (config.sample_rate(), config.channels(), config.sample_format());
    let config: cpal::StreamConfig = config.into();
    let mostres = Arc::new(Mutex::new(Vec::new()));

    let stream = match format {
        SampleFormat::F32 => escoltar::<f32>(&device, config, &mostres),
        SampleFormat::I16 => escoltar::<i16>(&device, config, &mostres),
        SampleFormat::I32 => escoltar::<i32>(&device, config, &mostres),
        SampleFormat::U16 => escoltar::<u16>(&device, config, &mostres),
        f => return Err(format!("Format de micro no suportat: {f}")),
    }?;
    stream.play().map_err(|e| e.to_string())?;
    Ok((stream, mostres, hz, canals))
}

fn escoltar<T>(
    device: &cpal::Device,
    config: cpal::StreamConfig,
    mostres: &Arc<Mutex<Vec<f32>>>,
) -> Result<cpal::Stream, String>
where
    T: SizedSample,
    f32: FromSample<T>,
{
    let m = mostres.clone();
    device
        .build_input_stream(
            config,
            move |d: &[T], _: &_| {
                if let Ok(mut v) = m.lock() {
                    v.extend(d.iter().map(|&s| f32::from_sample(s)));
                }
            },
            |e| eprintln!("micro: {e}"),
            None,
        )
        .map_err(|e| e.to_string())
}

/// Volum mitjà (RMS) de l'àudio, per saber si hi ha veu.
pub fn volum(audio: &[f32]) -> f32 {
    if audio.is_empty() {
        return 0.0;
    }
    (audio.iter().map(|s| s * s).sum::<f32>() / audio.len() as f32).sqrt()
}

/// Barreja els canals i passa a 16 kHz.
// shortcut: interpolació lineal sense filtre passa-baix (pot fer aliasing); si la qualitat en pateix, posar-hi rubato.
pub fn a_16k_mono(mostres: &[f32], canals: u16, hz: u32) -> Vec<f32> {
    let c = canals.max(1) as usize;
    let mono: Vec<f32> = mostres
        .chunks(c)
        .map(|f| f.iter().sum::<f32>() / f.len() as f32)
        .collect();
    if hz == HZ_WHISPER || mono.is_empty() {
        return mono;
    }
    let pas = hz as f64 / HZ_WHISPER as f64;
    let n = (mono.len() as f64 / pas) as usize;
    (0..n)
        .map(|i| {
            let x = i as f64 * pas;
            let j = x as usize;
            let a = mono[j];
            let b = *mono.get(j + 1).unwrap_or(&a);
            a + (b - a) * (x - j as f64) as f32
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn estereo_48k_a_mono_16k() {
        // 1 s d'estèreo a 48 kHz: esquerra 0.2, dreta 0.4
        let entrada: Vec<f32> = (0..48_000).flat_map(|_| [0.2, 0.4]).collect();
        let sortida = a_16k_mono(&entrada, 2, 48_000);
        assert_eq!(sortida.len(), 16_000);
        assert!(sortida.iter().all(|&s| (s - 0.3).abs() < 1e-6));
    }

    #[test]
    fn volum_del_silenci_i_de_la_veu() {
        assert_eq!(volum(&[0.0; 100]), 0.0);
        assert!((volum(&[0.5, -0.5]) - 0.5).abs() < 1e-6);
    }

    #[test]
    fn en_curs_mor_amb_la_gravacio() {
        let mostres = Arc::new(Mutex::new(vec![0.5; 32_000]));
        let e = EnCurs { mostres: Arc::downgrade(&mostres), hz: 32_000, canals: 2 };
        assert!(e.viu());
        assert_eq!(e.audio().unwrap().len(), 8_000);
        drop(mostres);
        assert!(!e.viu() && e.audio().is_none());
    }

    #[test]
    fn ja_a_16k_no_canvia() {
        assert_eq!(a_16k_mono(&[0.1, -0.1], 1, 16_000), vec![0.1, -0.1]);
    }
}
