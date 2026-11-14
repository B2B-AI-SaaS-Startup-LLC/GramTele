//! Headless name-search slice of core/utils.cpp; Qt rendering is not involved.
const LETTERS: &[(char, &str)] = &[
    ('А', "A"),
    ('Б', "B"),
    ('В', "V"),
    ('Г', "G"),
    ('Ґ', "G"),
    ('Д', "D"),
    ('Е', "E"),
    ('Є', "Ye"),
    ('Ё', "Yo"),
    ('Ж', "Zh"),
    ('З', "Z"),
    ('И', "I"),
    ('Ї', "Yi"),
    ('І', "I"),
    ('Й', "J"),
    ('К', "K"),
    ('Л', "L"),
    ('М', "M"),
    ('Н', "N"),
    ('О', "O"),
    ('П', "P"),
    ('Р', "R"),
    ('С', "S"),
    ('Т', "T"),
    ('У', "U"),
    ('Ў', "W"),
    ('Ф', "F"),
    ('Х', "Kh"),
    ('Ц', "Ts"),
    ('Ч', "Ch"),
    ('Ш', "Sh"),
    ('Щ', "Sch"),
    ('Э', "E"),
    ('Ю', "Yu"),
    ('Я', "Ya"),
    ('Ў', "W"),
    ('а', "a"),
    ('б', "b"),
    ('в', "v"),
    ('г', "g"),
    ('ґ', "g"),
    ('д', "d"),
    ('е', "e"),
    ('є', "ye"),
    ('ё', "yo"),
    ('ж', "zh"),
    ('з', "z"),
    ('й', "y"),
    ('ї', "yi"),
    ('і', "i"),
    ('л', "l"),
    ('м', "m"),
    ('н', "n"),
    ('о', "o"),
    ('п', "p"),
    ('р', "r"),
    ('с', "s"),
    ('т', "t"),
    ('у', "u"),
    ('ў', "w"),
    ('ф', "f"),
    ('х', "kh"),
    ('ц', "ts"),
    ('ч', "ch"),
    ('ш', "sh"),
    ('щ', "sch"),
    ('ъ', ""),
    ('э', "e"),
    ('ю', "yu"),
    ('я', "ya"),
    ('ў', "w"),
    ('Ы', "Y"),
    ('и', "i"),
    ('к', "k"),
    ('ы', "y"),
    ('ь', ""),
];
const DOUBLES: &[(&str, &str)] = &[
    ("Ый", "Y"),
    ("ия", "ia"),
    ("ий", "y"),
    ("кс", "x"),
    ("ый", "y"),
    ("ье", "ye"),
];
const SPECIAL: &[(&str, &str)] = &[
    ("Александр", "Alexander"),
    ("александр", "alexander"),
    ("Филипп", "Philip"),
    ("филипп", "philip"),
    ("Пётр", "Petr"),
    ("пётр", "petr"),
    ("Гай", "Gai"),
    ("гай", "gai"),
    ("Ильин", "Ilyin"),
    ("ильин", "ilyin"),
];
pub fn translit_rus_eng(rus: &str) -> String {
    if let Some((_, v)) = SPECIAL.iter().find(|(key, _)| *key == rus) {
        return (*v).into();
    }
    let mut result = String::new();
    let mut chars = rus.chars().peekable();
    while let Some(ch) = chars.next() {
        if let Some(next) = chars.peek() {
            let pair = format!("{ch}{next}");
            if let Some((_, v)) = DOUBLES.iter().find(|(key, _)| *key == pair) {
                result.push_str(v);
                chars.next();
                continue;
            }
        }
        if let Some((_, v)) = LETTERS.iter().rev().find(|(key, _)| *key == ch) {
            result.push_str(v);
        } else {
            result.push(ch);
        }
    }
    result
}
fn switch_layout(from: &str, alphabet: &str) -> String {
    let english = "qwertyuiop[]asdfghjkl;'zxcvbnm,.";
    let mut mapping = std::collections::BTreeMap::new();
    for (a, b) in english.chars().zip(alphabet.chars()) {
        mapping.insert(a, b);
        mapping.insert(a.to_ascii_uppercase(), b.to_uppercase().next().unwrap());
        mapping.insert(b, a);
        mapping.insert(b.to_uppercase().next().unwrap(), a.to_ascii_uppercase());
    }
    // Original inserts uppercase BEFORE lowercase. Punctuation is its own upper
    // case and ends up mapping to LOWERCASE Cyrillic, not uppercase.
    for (a, b) in english.chars().zip(alphabet.chars()) {
        if !a.is_ascii_alphabetic() {
            mapping.insert(a, b);
        }
    }
    from.chars()
        .map(|c| mapping.get(&c).copied().unwrap_or(c))
        .collect()
}
pub fn rus_keyboard_layout_switch(from: &str) -> String {
    let rus = switch_layout(from, "йцукенгшщзхъфывапролджэячсмитьбю");
    let ukr = switch_layout(from, "йцукенгшщзхїфівапролджєячсмитьбю");
    if rus == ukr {
        rus
    } else {
        format!("{rus} {ukr}")
    }
}
