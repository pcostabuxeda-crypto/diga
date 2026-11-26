use std::sync::{Arc, Mutex, mpsc};
use std::thread::JoinHandle;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, Sample, SampleFormat, SizedSample};

pub const HZ_WHISPER: u32 = 16_000;

/// Gravació en curs. El `Stream` de cpal no és `Send`: viu dins el seu fil.
pub struct Gravacio {
    aturar: mpsc::Sender<()>,
    fil: JoinHandle<Vec<f32>>,
}

pub fn comencar() -> Result<Gravacio, String> {
    let (aturar, rebre_aturar) = mpsc::channel();
    let (llest, rebre_llest) = mpsc::channel();
    let fil = std::thread::spawn(move || {
        let (stream, mostres, hz, canals) = match obrir() {
            Ok(v) => {
                let _ = llest.send(Ok(()));
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
    rebre_llest
        .recv()
        .map_err(|_| "El fil d'àudio ha caigut".to_string())??;
    Ok(Gravacio { aturar, fil })
}

impl Gravacio {
    /// Atura el micro i torna l'àudio a 16 kHz mono, com el vol Whisper.
    pub fn aturar(self) -> Result<Vec<f32>, String> {
        let _ = self.aturar.send(());
        self.fil.join().map_err(|_| "El fil d'àudio ha petat".to_string())
    }
}

type Obert = (cpal::Stream, Arc<Mutex<Vec<f32>>>, u32, u16);

fn obrir() -> Result<Obert, String> {
    let device = cpal::default_host()
        .default_input_device()
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
    fn ja_a_16k_no_canvia() {
        assert_eq!(a_16k_mono(&[0.1, -0.1], 1, 16_000), vec![0.1, -0.1]);
    }
}
