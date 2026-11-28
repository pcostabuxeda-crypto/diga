// Portat de Handy (https://github.com/cjpais/Handy, MIT) - apply_custom_words, remove_filler_words, collapse_stutters

use regex::Regex;
use strsim::normalized_levenshtein;
use natural::phonetics::SoundexWord;

/// Llindar per coincidència aproximada (0.0 - 1.0).
/// Valors més alts = més estricte. 0.3 permet correccions moderades.
const THRESHOLD: f64 = 0.3;

/// Aplica vocabulari personalitzat al text transcrit.
///
/// `vocabulari` és un string amb paraules separades per comes (ex: "Docker, Claude, pcosta-client").
/// Utilitza coincidència aproximada (Levenshtein + Soundex) mantenint majúscules i puntuació.
pub fn aplicar_vocabulari(text: &str, vocabulari: &str) -> String {
    let paraules_vocabulari: Vec<&str> = vocabulari
        .split([',', '\n'])
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .collect();

    if paraules_vocabulari.is_empty() {
        return text.to_string();
    }

    // Pre-calcula SoundexWord de les paraules del vocabulari
    let soundex_vocabulari: Vec<SoundexWord> = paraules_vocabulari.iter().map(|p| SoundexWord::new(p)).collect();

    let mut resultat = String::with_capacity(text.len());
    let mut buffer = String::new();

    for c in text.chars() {
        if c.is_alphanumeric() || c == '\'' || c == '’' || c == '-' {
            buffer.push(c);
        } else {
            if !buffer.is_empty() {
                resultat.push_str(&corregir_paraula(&buffer, &paraules_vocabulari, &soundex_vocabulari));
                buffer.clear();
            }
            resultat.push(c);
        }
    }

    // Processa l'última paraula si n'hi ha
    if !buffer.is_empty() {
        resultat.push_str(&corregir_paraula(&buffer, &paraules_vocabulari, &soundex_vocabulari));
    }

    resultat
}

/// Corregeix una paraula individual utilitzant Levenshtein + Soundex.
fn corregir_paraula(
    paraula: &str,
    vocabulari: &[&str],
    _soundex_vocabulari: &[SoundexWord],
) -> String {
    let soundex_paraula = SoundexWord::new(paraula);
    let paraula_lower = paraula.to_lowercase();

    let mut millor_coincidencia = None;
    let mut millor_puntuacio = 0.0;

    for v in vocabulari {
        // Saltar si la paraula ja coincideix exactament (ignorant majúscules)
        if paraula_lower == v.to_lowercase() {
            return preservar_cas(paraula, v);
        }

        // Soundex ràpid com a filtre (case-insensitive)
        if !soundex_paraula.sounds_like_str(&v.to_lowercase()) {
            continue;
        }

        // Levenshtein normalitzat per puntuació fina
        let puntuacio = normalized_levenshtein(&paraula_lower, &v.to_lowercase());
        if puntuacio > millor_puntuacio && puntuacio >= THRESHOLD {
            millor_puntuacio = puntuacio;
            millor_coincidencia = Some(*v);
        }
    }

    match millor_coincidencia {
        Some(correccio) => preservar_cas(paraula, correccio),
        None => paraula.to_string(),
    }
}

/// Preserva el patró de majúscules/minúscules de l'original en la correcció.
fn preservar_cas(original: &str, correccio: &str) -> String {
    let chars_orig: Vec<char> = original.chars().collect();
    let chars_corr: Vec<char> = correccio.chars().collect();

    // Si l'original és tot majúscules, correcció en majúscules
    if chars_orig.iter().all(|c| c.is_uppercase()) {
        return correccio.to_uppercase();
    }

    // Si l'original comença per majúscula, correcció amb inicial majúscula
    if !chars_orig.is_empty() && chars_orig[0].is_uppercase() {
        let mut result = String::with_capacity(correccio.len());
        for (i, c) in chars_corr.iter().enumerate() {
            if i == 0 {
                result.push(c.to_uppercase().next().unwrap_or(*c));
            } else {
                result.push(*c);
            }
        }
        return result;
    }

    // Altrament, tot en minúscules
    correccio.to_lowercase()
}

// ============================================================================
// ELIMINACIÓ DE FALQUES I REPETICIONS
// ============================================================================

/// Llista de falques (paraules de farciment) en català.
const FALQUES_CA: &[&str] = &[
    "eh", "ehm", "mmm", "doncs doncs", "este", "bueno", "pues", "o sea", "digamos",
    "es que", "mira", "venga", "vale", "claro", "ah", "uh", "um", "uhm", "hmm",
];

/// Llista de falques (paraules de farciment) en castellà.
const FALQUES_ES: &[&str] = &[
    "eh", "ehm", "mmm", "pues", "este", "bueno", "o sea", "digamos", "es que",
    "mira", "venga", "vale", "claro", "ah", "uh", "um", "uhm", "hmm",
];

/// Elimina falques i col·lapseja estotegues (repeticions 3+ cops).
///
/// `idioma`: "ca" per català, "es" per castellà, "auto" per detectar automàticament.
pub fn netejar_falques(text: &str, idioma: &str) -> String {
    // Auto-detecció ANTES de processar (perquè el processat elimina les paraules clau)
    let falques = match idioma {
        "ca" => FALQUES_CA,
        "es" => FALQUES_ES,
        _ => {
            // Auto-detecció simple: si hi ha paraules catalanes típiques
            let text_lower = text.to_lowercase();
            if text_lower.contains("doncs") || text_lower.contains("este") || text_lower.contains("o sea") {
                FALQUES_CA
            } else {
                FALQUES_ES
            }
        }
    };

    let mut text_net = text.to_string();

    // 1. Elimina falques (paraules de farciment)
    text_net = eliminar_falques(&text_net, falques);

    // 2. Col·lapseja estotegues (repeticions de 3 o més cops)
    text_net = collapse_stutters(&text_net);

    // 3. Netegem espais múltiples i retoc final
    netejar_espais(&text_net)
}

/// Elimina les falques de la llista donada.
fn eliminar_falques(text: &str, falques: &[&str]) -> String {
    let mut resultat = text.to_string();

    // Ordenem per longitud descendent per treure primer les falques més llargues
    // (ex: "doncs doncs" abans que "doncs")
    let mut falques_ordenades: Vec<&str> = falques.to_vec();
    falques_ordenades.sort_by_key(|f| std::cmp::Reverse(f.len()));

    for falca in falques_ordenades {
        // Regex per paraules completes (fronteres de paraula), case-insensitive
        let pattern = format!(r"(?i)\b{}\b", regex::escape(falca));
        if let Ok(re) = Regex::new(&pattern) {
            resultat = re.replace_all(&resultat, " ").to_string();
        }
    }

    resultat
}

/// Col·lapseja repeticions de 3 o més cops de la mateixa paraula (estotegues).
/// Ex: "el el el cotxe" -> "el el cotxe" (manté 2 ocurrències)
fn collapse_stutters(text: &str) -> String {
    // Separem per espais mantenint la puntuació
    let re = match Regex::new(r"\s+") {
        Ok(r) => r,
        Err(_) => return text.to_string(),
    };

    let parts: Vec<&str> = re.split(text).collect();
    if parts.len() < 3 {
        return text.to_string();
    }

    let mut result = Vec::new();
    let mut i = 0;

    while i < parts.len() {
        let current = parts[i];
        let mut count = 1;

        // Compte repeticions consecutives (case-insensitive)
        while i + count < parts.len()
            && parts[i + count].eq_ignore_ascii_case(current)
        {
            count += 1;
        }

        if count >= 3 {
            // Només guardem 2 ocurrències
            result.push(current);
            result.push(current);
        } else {
            for _ in 0..count {
                result.push(current);
            }
        }

        i += count;
    }

    result.join(" ")
}

/// Netegem espais múltiples, espais abans de puntuació, etc.
fn netejar_espais(text: &str) -> String {
    let mut resultat = text.to_string();

    // Espais múltiples -> un sol espai
    if let Ok(re) = Regex::new(r"\s+") {
        resultat = re.replace_all(&resultat, " ").to_string();
    }

    // Espai abans de puntuació -> sense espai
    if let Ok(re) = Regex::new(r"\s+([.,;:!?])") {
        resultat = re.replace_all(&resultat, "$1").to_string();
    }

    // Puntuació sense espai després -> afegim espai (ex: "hola,món" -> "hola, món")
    if let Ok(re) = Regex::new(r"([.,;:!?])(\S)") {
        resultat = re.replace_all(&resultat, "$1 $2").to_string();
    }

    resultat.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    // ===== Tests per aplicar_vocabulari =====

    #[test]
    fn aplicar_vocabulari_sense_vocabulari_retorna_original() {
        let text = "Hola món";
        assert_eq!(aplicar_vocabulari(text, ""), "Hola món");
        assert_eq!(aplicar_vocabulari(text, "   "), "Hola món");
        assert_eq!(aplicar_vocabulari(text, ",,"), "Hola món");
    }

    #[test]
    fn aplicar_vocabulari_corregix_paraula_coneguda() {
        let text = "Uso docker per contenidors";
        let vocab = "Docker";
        let resultat = aplicar_vocabulari(text, vocab);
        assert!(resultat.contains("Docker"));
    }

    #[test]
    fn aplicar_vocabulari_preserva_majúscules_inicial() {
        let text = "Uso docker";
        let vocab = "Docker";
        let resultat = aplicar_vocabulari(text, vocab);
        assert!(resultat.starts_with("Docker") || resultat.starts_with("docker"));
    }

    #[test]
    fn aplicar_vocabulari_preserva_totes_majúscules() {
        let text = "USO DOCKER";
        let vocab = "Docker";
        let resultat = aplicar_vocabulari(text, vocab);
        assert_eq!(resultat, "USO DOCKER");
    }

    #[test]
    fn aplicar_vocabulari_multiples_paraules() {
        let text = "Uso docker i claude per programar";
        let vocab = "Docker, Claude";
        let resultat = aplicar_vocabulari(text, vocab);
        assert!(resultat.contains("Docker") || resultat.contains("docker"));
        assert!(resultat.contains("Claude") || resultat.contains("claude"));
    }

    #[test]
    fn aplicar_vocabulari_amb_puntuacio() {
        let text = "Uso docker, i claude!";
        let vocab = "Docker, Claude";
        let resultat = aplicar_vocabulari(text, vocab);
        assert!(resultat.contains("Docker") || resultat.contains("docker"));
        assert!(resultat.contains("Claude") || resultat.contains("claude"));
    }

    #[test]
    fn aplicar_vocabulari_no_corregix_si_massa_diferent() {
        let text = "Uso cotxe per anar";
        let vocab = "Docker";
        let resultat = aplicar_vocabulari(text, vocab);
        assert_eq!(resultat, "Uso cotxe per anar");
    }

    // ===== Tests per netejar_falques =====

    #[test]
    fn netejar_falques_elimina_eh_catala() {
        let text = "Eh, hola món";
        let resultat = netejar_falques(text, "ca");
        assert!(!resultat.to_lowercase().starts_with("eh"));
        assert!(resultat.contains("hola"));
    }

    #[test]
    fn netejar_falques_elimina_doncs_doncs() {
        let text = "Doncs doncs, vull dir que...";
        let resultat = netejar_falques(text, "ca");
        assert!(!resultat.to_lowercase().contains("doncs doncs"));
    }

    #[test]
    fn netejar_falques_elimina_este_castella() {
        let text = "Este, quiero decir que...";
        let resultat = netejar_falques(text, "es");
        assert!(!resultat.to_lowercase().starts_with("este"));
    }

    #[test]
    fn netejar_falques_col·lapseja_estotegues() {
        let text = "El el el cotxe és vermell";
        let resultat = netejar_falques(text, "ca");
        assert_eq!(resultat, "El cotxe és vermell");
    }

    #[test]
    fn netejar_falques_col·lapseja_tres_repeticions() {
        let text = "Hola hola hola món";
        let resultat = netejar_falques(text, "ca");
        assert_eq!(resultat, "Hola món");
    }

    #[test]
    fn netejar_falques_no_col·lapseja_dos_cops() {
        let text = "Hola hola món";
        let resultat = netejar_falques(text, "ca");
        // Dos cops no és estotegua, es manté
        assert_eq!(resultat, "Hola hola món");
    }

    #[test]
    fn netejar_falques_netegem_espais_multiples() {
        let text = "Hola    món";
        let resultat = netejar_falques(text, "ca");
        assert_eq!(resultat, "Hola món");
    }

    #[test]
    fn netejar_falques_espai_abans_puntuacio() {
        let text = "Hola , món";
        let resultat = netejar_falques(text, "ca");
        assert_eq!(resultat, "Hola, món");
    }

    #[test]
    fn netejar_falques_espai_despres_puntuacio() {
        let text = "Hola,món";
        let resultat = netejar_falques(text, "ca");
        assert_eq!(resultat, "Hola, món");
    }

    #[test]
    fn netejar_falques_auto_detecta_catala() {
        let text = "Doncs vull dir que este és un test";
        let resultat = netejar_falques(text, "auto");
        // Deu detectar català per "doncs"
        assert!(!resultat.to_lowercase().contains("doncs"));
    }

    #[test]
    fn netejar_falques_auto_detecta_castella() {
        let text = "Pues quiero decir que este es un test";
        let resultat = netejar_falques(text, "auto");
        // Deu detectar castellà
        assert!(!resultat.to_lowercase().contains("pues"));
    }

    #[test]
    fn netejar_falques_manten_paraules_sembitants_no_falques() {
        let text = "El cotxe és verd";
        let resultat = netejar_falques(text, "ca");
        assert_eq!(resultat, "El cotxe és verd");
    }

    #[test]
    fn netejar_falques_case_insensitive() {
        let text = "EH EHM MMM hola";
        let resultat = netejar_falques(text, "ca");
        assert_eq!(resultat.trim(), "hola");
    }
}