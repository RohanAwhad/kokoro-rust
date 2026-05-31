use std::sync::LazyLock;

use fancy_regex::Regex;

fn split_num(caps: &fancy_regex::Captures) -> String {
    let num = caps.get(0).unwrap().as_str();
    if num.contains('.') {
        return num.to_string();
    }
    if num.contains(':') {
        let parts: Vec<&str> = num.split(':').collect();
        let h: i32 = parts[0].parse().unwrap_or(0);
        let m: i32 = parts[1].parse().unwrap_or(0);
        if m == 0 {
            return format!("{h} o'clock");
        } else if m < 10 {
            return format!("{h} oh {m}");
        }
        return format!("{h} {m}");
    }
    let year: i32 = num[..4].parse().unwrap_or(0);
    if year < 1100 || year % 1000 < 10 {
        return num.to_string();
    }
    let left = &num[..2];
    let right: i32 = num[2..4].parse().unwrap_or(0);
    let s = if num.ends_with('s') { "s" } else { "" };
    if (100..=999).contains(&(year % 1000)) {
        if right == 0 {
            return format!("{left} hundred{s}");
        } else if right < 10 {
            return format!("{left} oh {right}{s}");
        }
    }
    format!("{left} {right}{s}")
}

fn flip_money(caps: &fancy_regex::Captures) -> String {
    let m = caps.get(0).unwrap().as_str();
    let bill = if m.starts_with('$') { "dollar" } else { "pound" };
    let rest = &m[1..];
    if rest.chars().last().map_or(false, |c| c.is_alphabetic()) {
        return format!("{rest} {bill}s");
    }
    if !rest.contains('.') {
        let s = if rest == "1" { "" } else { "s" };
        return format!("{rest} {bill}{s}");
    }
    let parts: Vec<&str> = rest.split('.').collect();
    let b = parts[0];
    let c_str = parts[1];
    let s = if b == "1" { "" } else { "s" };
    let c: i32 = format!("{:0<2}", c_str).parse().unwrap_or(0);
    let coins = if m.starts_with('$') {
        if c == 1 { "cent" } else { "cents" }
    } else {
        if c == 1 { "penny" } else { "pence" }
    };
    format!("{b} {bill}{s} and {c} {coins}")
}

fn point_num(caps: &fancy_regex::Captures) -> String {
    let num = caps.get(0).unwrap().as_str();
    let parts: Vec<&str> = num.split('.').collect();
    let a = parts[0];
    let b: String = parts[1].chars().map(|c| format!(" {c}")).collect::<String>();
    format!("{a} point{b}")
}

fn abbreviation_dots(caps: &fancy_regex::Captures) -> String {
    caps.get(0).unwrap().as_str().replace('.', "-")
}

pub fn normalize_text(text: &str) -> String {
    static RE_NONSPACE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"[^\S \n]").unwrap());
    static RE_MULTISPACE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"  +").unwrap());
    static RE_NEWLINE_SPACES: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(?<=\n) +(?=\n)").unwrap());
    static RE_DR: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"\bD[Rr]\.(?= [A-Z])").unwrap());
    static RE_MR: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"\b(?:Mr\.|MR\.(?= [A-Z]))").unwrap());
    static RE_MS: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"\b(?:Ms\.|MS\.(?= [A-Z]))").unwrap());
    static RE_MRS: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"\b(?:Mrs\.|MRS\.(?= [A-Z]))").unwrap());
    static RE_ETC: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"\betc\.(?! [A-Z])").unwrap());
    static RE_YEAH: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(?i)\b(y)eah?\b").unwrap());
    static RE_NUM: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"\d*\.\d+|\b\d{4}s?\b|(?<!:)\b(?:[1-9]|1[0-2]):[0-5]\d\b(?!:)").unwrap());
    static RE_DIGIT_COMMA: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(?<=\d),(?=\d)").unwrap());
    static RE_MONEY: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(?i)[$£]\d+(?:\.\d+)?(?: hundred| thousand| (?:[bm]|tr)illion)*\b|[$£]\d+\.\d\d?\b").unwrap());
    static RE_DECIMAL: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"\d*\.\d+").unwrap());
    static RE_DIGIT_DASH: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(?<=\d)-(?=\d)").unwrap());
    static RE_DIGIT_S: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(?<=\d)S").unwrap());
    static RE_POSSESSIVE_S: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(?<=[BCDFGHJ-NP-TV-Z])'?s\b").unwrap());
    static RE_XS: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(?<=X')S\b").unwrap());
    static RE_ABBREV_DOTS: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(?:[A-Za-z]\.){2,} [a-z]").unwrap());
    static RE_INITIAL_DOT: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(?i)(?<=[A-Z])\.(?=[A-Z])").unwrap());

    let mut text = text.to_string();

    text = text.replace('\u{2018}', "'").replace('\u{2019}', "'");
    text = text.replace('\u{00ab}', "\u{201c}").replace('\u{00bb}', "\u{201d}");
    text = text.replace('\u{201c}', "\"").replace('\u{201d}', "\"");
    text = text.replace('(', "\u{00ab}").replace(')', "\u{00bb}");

    let cjk_map = [
        ('\u{3001}', ", "),
        ('\u{3002}', ". "),
        ('\u{ff01}', "! "),
        ('\u{ff0c}', ", "),
        ('\u{ff1a}', ": "),
        ('\u{ff1b}', "; "),
        ('\u{ff1f}', "? "),
    ];
    for (a, b) in cjk_map {
        text = text.replace(a, b);
    }

    text = RE_NONSPACE.replace_all(&text, " ").to_string();
    text = RE_MULTISPACE.replace_all(&text, " ").to_string();
    text = RE_NEWLINE_SPACES.replace_all(&text, "").to_string();
    text = RE_DR.replace_all(&text, "Doctor").to_string();
    text = RE_MR.replace_all(&text, "Mister").to_string();
    text = RE_MS.replace_all(&text, "Miss").to_string();
    text = RE_MRS.replace_all(&text, "Mrs").to_string();
    text = RE_ETC.replace_all(&text, "etc").to_string();
    text = RE_YEAH.replace_all(&text, "${1}e'a").to_string();
    text = RE_NUM.replace_all(&text, split_num).to_string();
    text = RE_DIGIT_COMMA.replace_all(&text, "").to_string();
    text = RE_MONEY.replace_all(&text, flip_money).to_string();
    text = RE_DECIMAL.replace_all(&text, point_num).to_string();
    text = RE_DIGIT_DASH.replace_all(&text, " to ").to_string();
    text = RE_DIGIT_S.replace_all(&text, " S").to_string();
    text = RE_POSSESSIVE_S.replace_all(&text, "'S").to_string();
    text = RE_XS.replace_all(&text, "s").to_string();
    text = RE_ABBREV_DOTS.replace_all(&text, abbreviation_dots).to_string();
    text = RE_INITIAL_DOT.replace_all(&text, "-").to_string();

    text.trim().to_string()
}
