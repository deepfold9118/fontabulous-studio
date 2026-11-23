//! Glyph names ↔ Unicode (common AGL names, uniXXXX fallback), glyph sets,
//! categories and scripts used by the font window sidebar.

#![allow(dead_code)]

use std::collections::HashMap;
use std::sync::OnceLock;

const AGL: &str = "space 20 exclam 21 quotedbl 22 numbersign 23 dollar 24 percent 25 ampersand 26 quotesingle 27 parenleft 28 parenright 29 asterisk 2A plus 2B comma 2C hyphen 2D period 2E slash 2F zero 30 one 31 two 32 three 33 four 34 five 35 six 36 seven 37 eight 38 nine 39 colon 3A semicolon 3B less 3C equal 3D greater 3E question 3F at 40 bracketleft 5B backslash 5C bracketright 5D asciicircum 5E underscore 5F grave 60 braceleft 7B bar 7C braceright 7D asciitilde 7E \
nbspace A0 exclamdown A1 cent A2 sterling A3 currency A4 yen A5 brokenbar A6 section A7 dieresis A8 copyright A9 ordfeminine AA guillemotleft AB logicalnot AC softhyphen AD registered AE macron AF degree B0 plusminus B1 twosuperior B2 threesuperior B3 acute B4 mu B5 paragraph B6 periodcentered B7 cedilla B8 onesuperior B9 ordmasculine BA guillemotright BB onequarter BC onehalf BD threequarters BE questiondown BF \
Agrave C0 Aacute C1 Acircumflex C2 Atilde C3 Adieresis C4 Aring C5 AE C6 Ccedilla C7 Egrave C8 Eacute C9 Ecircumflex CA Edieresis CB Igrave CC Iacute CD Icircumflex CE Idieresis CF Eth D0 Ntilde D1 Ograve D2 Oacute D3 Ocircumflex D4 Otilde D5 Odieresis D6 multiply D7 Oslash D8 Ugrave D9 Uacute DA Ucircumflex DB Udieresis DC Yacute DD Thorn DE germandbls DF \
agrave E0 aacute E1 acircumflex E2 atilde E3 adieresis E4 aring E5 ae E6 ccedilla E7 egrave E8 eacute E9 ecircumflex EA edieresis EB igrave EC iacute ED icircumflex EE idieresis EF eth F0 ntilde F1 ograve F2 oacute F3 ocircumflex F4 otilde F5 odieresis F6 divide F7 oslash F8 ugrave F9 uacute FA ucircumflex FB udieresis FC yacute FD thorn FE ydieresis FF \
Amacron 100 amacron 101 Abreve 102 abreve 103 Aogonek 104 aogonek 105 Cacute 106 cacute 107 Ccircumflex 108 ccircumflex 109 Cdotaccent 10A cdotaccent 10B Ccaron 10C ccaron 10D Dcaron 10E dcaron 10F Dcroat 110 dcroat 111 Emacron 112 emacron 113 Ebreve 114 ebreve 115 Edotaccent 116 edotaccent 117 Eogonek 118 eogonek 119 Ecaron 11A ecaron 11B Gcircumflex 11C gcircumflex 11D Gbreve 11E gbreve 11F Gdotaccent 120 gdotaccent 121 Gcommaaccent 122 gcommaaccent 123 Hcircumflex 124 hcircumflex 125 Hbar 126 hbar 127 Itilde 128 itilde 129 Imacron 12A imacron 12B Ibreve 12C ibreve 12D Iogonek 12E iogonek 12F Idotaccent 130 dotlessi 131 IJ 132 ij 133 Jcircumflex 134 jcircumflex 135 Kcommaaccent 136 kcommaaccent 137 kgreenlandic 138 Lacute 139 lacute 13A Lcommaaccent 13B lcommaaccent 13C Lcaron 13D lcaron 13E Ldot 13F ldot 140 Lslash 141 lslash 142 Nacute 143 nacute 144 Ncommaaccent 145 ncommaaccent 146 Ncaron 147 ncaron 148 napostrophe 149 Eng 14A eng 14B Omacron 14C omacron 14D Obreve 14E obreve 14F Ohungarumlaut 150 ohungarumlaut 151 OE 152 oe 153 Racute 154 racute 155 Rcommaaccent 156 rcommaaccent 157 Rcaron 158 rcaron 159 Sacute 15A sacute 15B Scircumflex 15C scircumflex 15D Scedilla 15E scedilla 15F Scaron 160 scaron 161 Tcedilla 162 tcedilla 163 Tcaron 164 tcaron 165 Tbar 166 tbar 167 Utilde 168 utilde 169 Umacron 16A umacron 16B Ubreve 16C ubreve 16D Uring 16E uring 16F Uhungarumlaut 170 uhungarumlaut 171 Uogonek 172 uogonek 173 Wcircumflex 174 wcircumflex 175 Ycircumflex 176 ycircumflex 177 Ydieresis 178 Zacute 179 zacute 17A Zdotaccent 17B zdotaccent 17C Zcaron 17D zcaron 17E longs 17F florin 192 Scommaaccent 218 scommaaccent 219 Tcommaaccent 21A tcommaaccent 21B dotlessj 237 \
circumflex 2C6 caron 2C7 breve 2D8 dotaccent 2D9 ring 2DA ogonek 2DB tilde 2DC hungarumlaut 2DD \
gravecomb 300 acutecomb 301 circumflexcomb 302 tildecomb 303 macroncomb 304 brevecomb 306 dotaccentcomb 307 dieresiscomb 308 ringcomb 30A hungarumlautcomb 30B caroncomb 30C commaaccentcomb 326 cedillacomb 327 ogonekcomb 328 \
Alpha 391 Beta 392 Gamma 393 Delta 394 Epsilon 395 Zeta 396 Eta 397 Theta 398 Iota 399 Kappa 39A Lambda 39B Mu 39C Nu 39D Xi 39E Omicron 39F Pi 3A0 Rho 3A1 Sigma 3A3 Tau 3A4 Upsilon 3A5 Phi 3A6 Chi 3A7 Psi 3A8 Omega 3A9 alpha 3B1 beta 3B2 gamma 3B3 delta 3B4 epsilon 3B5 zeta 3B6 eta 3B7 theta 3B8 iota 3B9 kappa 3BA lambda 3BB mu.greek 3BC nu 3BD xi 3BE omicron 3BF pi 3C0 rho 3C1 sigmafinal 3C2 sigma 3C3 tau 3C4 upsilon 3C5 phi 3C6 chi 3C7 psi 3C8 omega 3C9 \
endash 2013 emdash 2014 quoteleft 2018 quoteright 2019 quotesinglbase 201A quotedblleft 201C quotedblright 201D quotedblbase 201E dagger 2020 daggerdbl 2021 bullet 2022 ellipsis 2026 perthousand 2030 guilsinglleft 2039 guilsinglright 203A fraction 2044 Euro 20AC trademark 2122 partialdiff 2202 product 220F summation 2211 minus 2212 radical 221A infinity 221E integral 222B approxequal 2248 notequal 2260 lessequal 2264 greaterequal 2265 lozenge 25CA fi FB01 fl FB02";

fn tables() -> &'static (HashMap<String, u32>, HashMap<u32, String>) {
    static T: OnceLock<(HashMap<String, u32>, HashMap<u32, String>)> = OnceLock::new();
    T.get_or_init(|| {
        let mut n2u = HashMap::new();
        let mut u2n = HashMap::new();
        let mut it = AGL.split_whitespace();
        while let (Some(n), Some(h)) = (it.next(), it.next()) {
            if let Ok(u) = u32::from_str_radix(h, 16) {
                n2u.insert(n.to_string(), u);
                u2n.entry(u).or_insert_with(|| n.to_string());
            }
        }
        for c in 'A'..='Z' {
            n2u.insert(c.to_string(), c as u32);
            u2n.insert(c as u32, c.to_string());
        }
        for c in 'a'..='z' {
            n2u.insert(c.to_string(), c as u32);
            u2n.insert(c as u32, c.to_string());
        }
        (n2u, u2n)
    })
}

pub fn unicode_for_name(name: &str) -> Option<u32> {
    if let Some(&u) = tables().0.get(name) {
        return Some(u);
    }
    let base = name.split('.').next().unwrap_or(name);
    if base != name {
        return None; // suffixed alternates are unencoded
    }
    if let Some(h) = name.strip_prefix("uni") {
        if h.len() == 4 {
            return u32::from_str_radix(h, 16).ok();
        }
    }
    if let Some(h) = name.strip_prefix('u') {
        if (4..=6).contains(&h.len()) && h.chars().all(|c| c.is_ascii_hexdigit()) {
            return u32::from_str_radix(h, 16).ok();
        }
    }
    None
}

pub fn name_for_unicode(u: u32) -> String {
    if let Some(n) = tables().1.get(&u) {
        return n.clone();
    }
    if u <= 0xFFFF {
        format!("uni{u:04X}")
    } else {
        format!("u{u:05X}")
    }
}

// ------------------------------------------------------------------------------------------
// Glyph sets

pub fn latin_simple() -> Vec<String> {
    let mut v = vec!["space".to_string()];
    v.extend(('A'..='Z').map(|c| c.to_string()));
    v.push("period".into());
    v.extend(('a'..='z').map(|c| c.to_string()));
    v.push("comma".into());
    for n in ["zero", "one", "two", "three", "four", "five", "six", "seven", "eight", "nine"] {
        v.push(n.into());
    }
    for n in [
        "hyphen", "plus", "asterisk", "asciicircum", "asciitilde", "slash", "backslash", "colon", "semicolon", "less", "equal", "greater", "exclam", "question",
        "quotesingle", "quotedbl", "numbersign", "at", "dollar", "percent", "ampersand", "parenleft", "parenright", "bracketleft", "bracketright", "braceleft",
        "braceright", "underscore", "bar", "grave",
    ] {
        v.push(n.into());
    }
    v
}

pub fn latin_pro() -> Vec<String> {
    let mut v = latin_simple();
    for u in (0xA0u32..=0xFF).chain(0x100..=0x17F).chain([0x192, 0x218, 0x219, 0x21A, 0x21B, 0x237, 0x2C6, 0x2C7, 0x2D8, 0x2D9, 0x2DA, 0x2DB, 0x2DC, 0x2DD]) {
        let n = name_for_unicode(u);
        if !v.contains(&n) {
            v.push(n);
        }
    }
    for u in [0x300u32, 0x301, 0x302, 0x303, 0x304, 0x306, 0x307, 0x308, 0x30A, 0x30B, 0x30C, 0x326, 0x327, 0x328] {
        v.push(name_for_unicode(u));
    }
    for u in [0x2013u32, 0x2014, 0x2018, 0x2019, 0x201A, 0x201C, 0x201D, 0x201E, 0x2020, 0x2021, 0x2022, 0x2026, 0x2030, 0x2039, 0x203A, 0x2044, 0x20AC, 0x2122, 0x2212, 0xFB01, 0xFB02] {
        v.push(name_for_unicode(u));
    }
    v
}

pub fn greek_basic() -> Vec<String> {
    (0x391u32..=0x3A9).chain(0x3B1..=0x3C9).filter(|u| *u != 0x3A2).map(name_for_unicode).collect()
}

pub fn cyrillic_basic() -> Vec<String> {
    (0x410u32..=0x44F).map(name_for_unicode).collect()
}

pub const GLYPH_SETS: [&str; 5] = ["Draw a New Font", "Latin Simple", "Latin Pro", "Latin + Greek", "Latin + Cyrillic"];

pub fn glyph_set(idx: usize) -> Vec<String> {
    match idx {
        0 => vec!["space".into()],
        1 => latin_simple(),
        2 => latin_pro(),
        3 => {
            let mut v = latin_simple();
            v.extend(greek_basic());
            v
        }
        _ => {
            let mut v = latin_simple();
            v.extend(cyrillic_basic());
            v
        }
    }
}

// ------------------------------------------------------------------------------------------
// Categories

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum Category {
    Uppercase,
    Lowercase,
    OtherLetter,
    Figure,
    Punctuation,
    Symbol,
    Mark,
    Separator,
    Other,
}

impl Category {
    pub fn label(self) -> &'static str {
        match self {
            Category::Uppercase => "Uppercase",
            Category::Lowercase => "Lowercase",
            Category::OtherLetter => "Other Letter",
            Category::Figure => "Figure",
            Category::Punctuation => "Punctuation",
            Category::Symbol => "Symbol",
            Category::Mark => "Mark",
            Category::Separator => "Separator",
            Category::Other => "Other",
        }
    }
    pub fn is_letter(self) -> bool {
        matches!(self, Category::Uppercase | Category::Lowercase | Category::OtherLetter)
    }
}

pub fn category(name: &str, unicode: Option<u32>) -> Category {
    let u = unicode.or_else(|| unicode_for_name(name.split('.').next().unwrap_or(name)));
    let Some(c) = u.and_then(char::from_u32) else {
        if name.ends_with("comb") {
            return Category::Mark;
        }
        return Category::Other;
    };
    let cp = c as u32;
    if (0x300..=0x36F).contains(&cp) {
        return Category::Mark;
    }
    if c.is_whitespace() || cp == 0xA0 {
        return Category::Separator;
    }
    if c.is_uppercase() {
        return Category::Uppercase;
    }
    if c.is_lowercase() {
        return Category::Lowercase;
    }
    if c.is_alphabetic() {
        return Category::OtherLetter;
    }
    if c.is_numeric() {
        return Category::Figure;
    }
    if c.is_ascii_punctuation() {
        return if "+<=>^`|~$".contains(c) { Category::Symbol } else { Category::Punctuation };
    }
    if (0x2010..=0x2027).contains(&cp) || (0x2030..=0x205E).contains(&cp) || [0xA1, 0xA7, 0xAB, 0xB6, 0xB7, 0xBB, 0xBF].contains(&cp) {
        return Category::Punctuation;
    }
    if (0x2C6..=0x2DD).contains(&cp) || cp == 0xA8 || cp == 0xAF || cp == 0xB4 || cp == 0xB8 {
        return Category::Mark;
    }
    Category::Symbol
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Script {
    Latin,
    Greek,
    Cyrillic,
    Common,
}

impl Script {
    pub fn label(self) -> &'static str {
        match self {
            Script::Latin => "Latin",
            Script::Greek => "Greek",
            Script::Cyrillic => "Cyrillic",
            Script::Common => "Common",
        }
    }
}

pub fn script(unicode: Option<u32>) -> Script {
    let Some(u) = unicode else { return Script::Common };
    let c = char::from_u32(u);
    match u {
        0x370..=0x3FF | 0x1F00..=0x1FFF => Script::Greek,
        0x400..=0x52F => Script::Cyrillic,
        0x41..=0x5A | 0x61..=0x7A | 0xC0..=0x24F | 0x1E00..=0x1EFF | 0xFB00..=0xFB06 if c.map(|c| c.is_alphabetic()).unwrap_or(false) => Script::Latin,
        _ => Script::Common,
    }
}

/// Unicode block name for the Font Map panel.
pub const BLOCKS: [(u32, u32, &str); 14] = [
    (0x0000, 0x007F, "Basic Latin"),
    (0x0080, 0x00FF, "Latin-1 Supplement"),
    (0x0100, 0x017F, "Latin Extended-A"),
    (0x0180, 0x024F, "Latin Extended-B"),
    (0x02B0, 0x02FF, "Spacing Modifier Letters"),
    (0x0300, 0x036F, "Combining Diacritical Marks"),
    (0x0370, 0x03FF, "Greek and Coptic"),
    (0x0400, 0x04FF, "Cyrillic"),
    (0x1E00, 0x1EFF, "Latin Extended Additional"),
    (0x2000, 0x206F, "General Punctuation"),
    (0x20A0, 0x20CF, "Currency Symbols"),
    (0x2100, 0x214F, "Letterlike Symbols"),
    (0x2200, 0x22FF, "Mathematical Operators"),
    (0xFB00, 0xFB4F, "Alphabetic Presentation Forms"),
];

/// Recipes for composite glyphs: base + marks (anchor names) for Build Glyphs.
pub fn composite_recipe(name: &str) -> Option<(String, Vec<(String, String)>)> {
    let marks = [
        ("acute", "acutecomb", "top"),
        ("grave", "gravecomb", "top"),
        ("circumflex", "circumflexcomb", "top"),
        ("tilde", "tildecomb", "top"),
        ("dieresis", "dieresiscomb", "top"),
        ("ring", "ringcomb", "top"),
        ("caron", "caroncomb", "top"),
        ("breve", "brevecomb", "top"),
        ("macron", "macroncomb", "top"),
        ("dotaccent", "dotaccentcomb", "top"),
        ("hungarumlaut", "hungarumlautcomb", "top"),
        ("cedilla", "cedillacomb", "bottom"),
        ("ogonek", "ogonekcomb", "ogonek"),
        ("commaaccent", "commaaccentcomb", "bottom"),
    ];
    for (suffix, mark, anchor) in marks {
        if let Some(base) = name.strip_suffix(suffix) {
            if base.is_empty() || base.len() > 2 {
                continue;
            }
            let base = match base {
                "i" if anchor == "top" => "dotlessi".to_string(),
                "j" if anchor == "top" => "dotlessj".to_string(),
                b => b.to_string(),
            };
            return Some((base, vec![(mark.to_string(), anchor.to_string())]));
        }
    }
    None
}
