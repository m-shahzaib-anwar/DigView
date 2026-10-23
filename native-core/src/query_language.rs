//! Bounded local keyword assistance, not general translation or language detection.
use std::{collections::HashMap, sync::OnceLock};

fn normalized(word: &str) -> String {
    word.chars().filter_map(|c| match c {
        '\u{0640}' | '\u{064b}'..='\u{065f}' | '\u{0670}' => None,
        '\u{0643}' => Some('ک'), '\u{064a}' | '\u{0649}' => Some('ی'),
        other => Some(other),
    }).collect::<String>().to_lowercase()
}
fn fillers(word: &str) -> bool {
    matches!(word,"kya"|"ke"|"ki"|"ka"|"k"|"mein"|"main"|"men"|"se"|"ko"|"par"|"pe"|"sakte"|"sakti"|"sakta"|"hain"|"hai"|"ho"|"kar"|"karo"|"kr"|"kry"|"krna"|"karna"|"kaise"|"kaisay"|"kese"|"kab"|"mujhe"|"mjy"|"mera"|"meri"|"mere"|"liye"|"ga"|"gi"|"ge"|"aur"|"or"|"کیا"|"کے"|"کی"|"کا"|"میں"|"سے"|"کو"|"پر"|"سکتے"|"سکتی"|"سکتا"|"ہیں"|"ہے"|"ہو"|"کر"|"کرنا"|"کیسے"|"کب"|"مجھے"|"میرا"|"میری"|"میرے"|"لیے"|"گا"|"گی"|"گے"|"اور")
}
fn aliases() -> &'static HashMap<String, &'static str> {
    static WORDS: OnceLock<HashMap<String, &'static str>> = OnceLock::new();
    WORDS.get_or_init(|| {
        let mut words=HashMap::new();
        for row in include_str!("search_aliases.tsv").lines().filter(|r|!r.starts_with('#')) {
            let columns=row.split('\t').collect::<Vec<_>>();
            if columns.len()!=3 {continue;}
            for alias in columns[1..].iter().flat_map(|v|v.split(',')) {
                words.insert(normalized(alias),columns[0]);
            }
        }
        words
    })
}
/// Script check alone is not language detection.
pub fn arabic_script(text: &str) -> bool {
    text.chars().any(|c|c.is_alphabetic() && matches!(c,'\u{0600}'..='\u{06ff}'|'\u{0750}'..='\u{077f}'|'\u{08a0}'..='\u{08ff}'|'\u{fb50}'..='\u{fdff}'|'\u{fe70}'..='\u{feff}'))
}
/// Conservative Urdu hints keep unrelated Arabic/Persian queries on their existing model path.
pub fn urdu_wording(text: &str) -> bool {
    arabic_script(text) && (text.chars().any(|c|matches!(c,'ٹ'|'ڈ'|'ڑ'|'ں'|'ے'|'ھ')) ||
        text.split_whitespace().any(|word| {
            let word=normalized(word.trim_matches(|c:char|!c.is_alphanumeric()));
            arabic_script(&word) && fillers(&word)
        }))
}
/// Keep unknown words, numbers and negation. Never interpret filenames as aliases.
/// Latin queries need two non-English signals to avoid changing normal English.
pub fn assist(text: &str) -> Option<String> {
    let text=crate::semantic::bounded_text(text,1200);
    let words=text.split_whitespace().take(80).map(|w|w.trim_matches(|c:char|!c.is_alphanumeric() && c!='.' && c!='_' && c!='-')).filter(|w|!w.is_empty()).collect::<Vec<_>>();
    let mut signals=0usize;let mut known=0usize;let mut content=0usize;
    let mut script=false;let mut output=Vec::new();
    for word in words {
        let normalized=normalized(word);
        script|=normalized.chars().any(|c|matches!(c,'\u{0600}'..='\u{06ff}'));
        if fillers(&normalized) {signals+=1;continue;}
        content+=1;
        if let Some(english)=aliases().get(&normalized) {
            known+=1;
            if normalized!=*english {signals+=1;}
            output.push((*english).to_string());
        } else {output.push(word.to_string());}
    }
    if known==0 || known*2<content || (!script && signals<2) {return None;}
    let assisted=output.join(" ");
    if assisted==text {None} else {Some(assisted)}
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn assists_both_scripts_without_demo_names_or_phrase_rules() {
        assert_eq!(assist("کیا کرائے کے گھر میں بلی رکھ سکتے ہیں؟").as_deref(),Some("rent home cat keep"));
        assert_eq!(assist("kya kiraye ke ghar mein billi rakh sakte hain").as_deref(),Some("rent home cat keep"));
        assert_eq!(assist("ہفتے میں تین دن گھر سے کام").as_deref(),Some("week three day home work"));
        assert_eq!(assist("parosi ka packet lena").as_deref(),Some("neighbor parcel receive"));
    }
    #[test]
    fn preserves_unknown_names_negation_numbers_and_filenames() {
        assert_eq!(assist("billi ki ijazat nahi").as_deref(),Some("cat permission not"));
        assert_eq!(assist("گھر بلی نہیں").as_deref(),Some("home cat not"));
        assert_eq!(assist("mera ghar billi ABC-42.txt 2026").as_deref(),Some("home cat ABC-42.txt 2026"));
        assert!(assist("main.rs ghar.txt").is_none());
        assert!(assist("the main office file").is_none());
        assert!(assist("unknown unfamiliar words ka").is_none());
        assert!(assist("غیرمعروف اصطلاح").is_none());
        assert!(assist("کیا کے ہیں").is_none());
        assert!(arabic_script("چاند پر خلائی جہاز کا مدار"));
        assert!(!arabic_script("cat?"));
        assert!(!arabic_script("cat؟"));
        assert!(urdu_wording("چاند پر خلائی جہاز کا مدار"));
        assert!(!urdu_wording("هل يمكن الاحتفاظ بقطة"));
        assert!(!urdu_wording("گربه در خانه"));
    }
    #[test]
    fn normalizes_arabic_letter_variants_and_marks_and_bounds_input() {
        assert_eq!(assist("كيا گھر بِلّي").as_deref(),Some("home cat"));
        assert!(assist(&"unknown ".repeat(1000)).is_none());
    }
}
