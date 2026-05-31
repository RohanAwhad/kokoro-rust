use std::collections::HashMap;
use std::sync::LazyLock;

pub(crate) static VOCAB_CHARS: LazyLock<String> = LazyLock::new(|| {
    let pad = "$";
    let punctuation = ";:,.!?¡¿—…\"«»\"\" ";
    let letters = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";
    let letters_ipa = "ɑɐɒæɓʙβɔɕçɗɖðʤəɘɚɛɜɝɞɟʄɡɠɢʛɦɧħɥʜɨɪʝɭɬɫɮʟɱɯɰŋɳɲɴøɵɸθœɶʘɹɺɾɻʀʁɽʂʃʈʧʉʊʋⱱʌɣɤʍχʎʏʑʐʒʔʡʕʢǀǁǂǃˈˌːˑʼʴʰʱʲʷˠˤ˞↓↑→↗↘'̩'ᵻ";

    let mut s = String::new();
    s.push_str(pad);
    s.push_str(punctuation);
    s.push_str(letters);
    s.push_str(letters_ipa);
    s
});

static VOCAB: LazyLock<HashMap<char, u32>> = LazyLock::new(build_vocab);

fn build_vocab() -> HashMap<char, u32> {
    let symbols: Vec<char> = VOCAB_CHARS.chars().collect();
    let mut dict = HashMap::new();
    for (i, &ch) in symbols.iter().enumerate() {
        dict.insert(ch, i as u32);
    }
    dict
}

pub fn tokenize(phonemes: &str) -> Vec<u32> {
    let vocab = &*VOCAB;
    phonemes.chars()
        .filter_map(|c| vocab.get(&c))
        .copied()
        .collect()
}
