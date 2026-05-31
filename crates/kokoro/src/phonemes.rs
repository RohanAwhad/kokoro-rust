use std::ffi::{c_char, c_int, c_void, CStr, CString};
use std::sync::Once;

use fancy_regex::Regex;

use crate::error::{KokoroError, Result};
use crate::normalize::normalize_text;
use crate::types::Lang;
use crate::vocab::VOCAB_CHARS;

const ESPEAK_AUDIO_OUTPUT_SYNCHRONOUS: c_int = 0x02;
const EE_OK: c_int = 0;

extern "C" {
    fn espeak_Initialize(
        output: c_int,
        buflength: c_int,
        path: *const c_char,
        options: c_int,
    ) -> c_int;

    fn espeak_SetVoiceByName(name: *const c_char) -> c_int;

    fn espeak_TextToPhonemes(
        textptr: *const *const c_void,
        textmode: c_int,
        phonememode: c_int,
    ) -> *const c_char;
}

static ESPEAK_INIT: Once = Once::new();

fn ensure_espeak_init() {
    ESPEAK_INIT.call_once(|| {
        let rate = unsafe {
            espeak_Initialize(ESPEAK_AUDIO_OUTPUT_SYNCHRONOUS, 0, std::ptr::null(), 0)
        };
        if rate < 0 {
            log::error!("espeak_Initialize failed");
        }
    });
}

fn set_espeak_voice(lang: Lang) -> Result<()> {
    let voice_name = match lang {
        Lang::Am => "en-us",
        Lang::Br => "en-gb",
    };
    let c_name = CString::new(voice_name).map_err(|e| KokoroError::Espeak(e.to_string()))?;
    let err = unsafe { espeak_SetVoiceByName(c_name.as_ptr()) };
    if err != EE_OK {
        return Err(KokoroError::Espeak(format!(
            "espeak_SetVoiceByName({voice_name}) failed: {err}"
        )));
    }
    Ok(())
}

pub fn phonemize(text: &str, lang: Lang, normalize: bool) -> Result<String> {
    let text = if normalize {
        normalize_text(text)
    } else {
        text.to_string()
    };

    if text.is_empty() {
        return Ok(String::new());
    }

    ensure_espeak_init();
    set_espeak_voice(lang)?;

    let (chunks, marks) = strip_punctuation(&text);
    if chunks.is_empty() {
        return Ok(String::new());
    }

    let mut phoneme_chunks = Vec::with_capacity(chunks.len());
    for chunk in &chunks {
        if chunk.is_empty() {
            phoneme_chunks.push(String::new());
            continue;
        }
        let ps = raw_phonemize(chunk)?;
        phoneme_chunks.push(postprocess_phonemes(&ps, lang));
    }

    Ok(restore_punctuation(&phoneme_chunks, &marks))
}

fn raw_phonemize(text: &str) -> Result<String> {
    let c_text = CString::new(text).map_err(|e| KokoroError::Espeak(e.to_string()))?;
    let mut text_ptr: *const c_void = c_text.as_ptr() as *const _;

    let mut utterances = Vec::new();
    loop {
        let phonemes_ptr = unsafe {
            espeak_TextToPhonemes(
                &mut text_ptr as *mut *const c_void,
                1,    // espeakCHARS_UTF8
                0x02, // espeakPHONEMES_IPA
            )
        };

        if phonemes_ptr.is_null() {
            break;
        }

        let ps = unsafe { CStr::from_ptr(phonemes_ptr) }
            .to_string_lossy()
            .to_string();

        if ps.is_empty() {
            break;
        }

        utterances.push(ps);

        if text_ptr.is_null() {
            break;
        }
    }

    Ok(utterances.join(" "))
}

const PUNCTUATION_MARKS: &str = ";:,.!?¡¿—…\"«»";

fn strip_punctuation(text: &str) -> (Vec<String>, Vec<String>) {
    let mut chunks = Vec::new();
    let mut marks = Vec::new();

    let mut current = String::new();
    for ch in text.chars() {
        if PUNCTUATION_MARKS.contains(ch) {
            if !current.is_empty() {
                chunks.push(current.clone());
                current.clear();
            }
            marks.push(ch.to_string());
        } else {
            current.push(ch);
        }
    }
    if !current.is_empty() {
        chunks.push(current);
    }

    (chunks, marks)
}

fn restore_punctuation(chunks: &[String], marks: &[String]) -> String {
    if chunks.is_empty() {
        return marks.join(" ");
    }

    let mut result = String::new();
    for (i, chunk) in chunks.iter().enumerate() {
        if i > 0 && !chunk.is_empty() {
            result.push(' ');
        }
        result.push_str(chunk);
        if i < marks.len() {
            result.push_str(&marks[i]);
        }
    }

    if !result.ends_with(' ') && !result.is_empty() {
        result.push(' ');
    }

    result
}

fn postprocess_phonemes(ps: &str, lang: Lang) -> String {
    let mut ps = ps.to_string();

    ps = ps.replace("kəkˈoːɹoʊ", "kˈoʊkəɹoʊ");
    ps = ps.replace("kəkˈɔːɹəʊ", "kˈəʊkəɹəʊ");

    ps = ps.replace('ʲ', "j");
    ps = ps.replace('r', "ɹ");
    ps = ps.replace('x', "k");
    ps = ps.replace('ɬ', "l");

    let re_hundred = Regex::new(r"(?<=[a-zɹː])(?=hˈʌndɹɪd)").unwrap();
    ps = re_hundred.replace_all(&ps, " ").to_string();

    let re_z_end = Regex::new(r#" z(?=[;:,.!?¡¿—…"«»\u{201d} ]|$)"#).unwrap();
    ps = re_z_end.replace_all(&ps, "z").to_string();

    if lang == Lang::Am {
        let re_ninety = Regex::new(r"(?<=nˈaɪn)ti(?!ː)").unwrap();
        ps = re_ninety.replace_all(&ps, "di").to_string();
    }

    let vocab_ref = &*VOCAB_CHARS;
    ps = ps.chars().filter(|c| vocab_ref.contains(*c)).collect();

    ps.trim().to_string()
}
