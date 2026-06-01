use kokoro::phonemes::phonemize;
use kokoro::types::Lang;

fn main() {
    let text = "Hello world!";
    let r = phonemize(text, Lang::Am, true).unwrap();
    println!("SHORT: {r:?}");

    let text = "Well, well, well, if it isn't the legend himself, Rohan.";
    let r = phonemize(text, Lang::Am, true).unwrap();
    println!("LONG:  {r:?}");

    let text = "Well, well, well, if it isn't the legend himself, Rohan. I was just telling the other AIs how boring it's been without you. What can I do for you, boss?";
    let r = phonemize(text, Lang::Am, true).unwrap();
    println!("FULL:  {r:?}");
    println!("len:   {}", r.len());
}

