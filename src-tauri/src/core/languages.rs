use std::collections::HashMap;

pub struct LanguageInfo {
    pub code: &'static str,
    pub name: &'static str,
    pub hy_mt_name: &'static str,
    pub is_chinese_family: bool,
}

pub fn all_languages() -> &'static [LanguageInfo] {
    &LANGUAGES
}

pub fn find_by_code(code: &str) -> Option<&'static LanguageInfo> {
    LANGUAGES.iter().find(|l| l.code == code)
}

pub fn hy_mt_language_name(code: &str) -> &'static str {
    find_by_code(code).map(|l| l.hy_mt_name).unwrap_or("English")
}

pub fn is_chinese_family(code: &str) -> bool {
    find_by_code(code).map(|l| l.is_chinese_family).unwrap_or(false)
}

static LANGUAGES: &[LanguageInfo] = &[
    LanguageInfo { code: "zh",    name: "Chinese (Simplified)",  hy_mt_name: "简体中文",        is_chinese_family: true  },
    LanguageInfo { code: "zh-TW", name: "Chinese (Traditional)", hy_mt_name: "繁体中文",        is_chinese_family: true  },
    LanguageInfo { code: "yue",   name: "Cantonese",             hy_mt_name: "粤语",            is_chinese_family: true  },
    LanguageInfo { code: "en",    name: "English",               hy_mt_name: "English",         is_chinese_family: false },
    LanguageInfo { code: "fr",    name: "French",                hy_mt_name: "French",          is_chinese_family: false },
    LanguageInfo { code: "de",    name: "German",                hy_mt_name: "German",          is_chinese_family: false },
    LanguageInfo { code: "es",    name: "Spanish",               hy_mt_name: "Spanish",         is_chinese_family: false },
    LanguageInfo { code: "pt",    name: "Portuguese",            hy_mt_name: "Portuguese",      is_chinese_family: false },
    LanguageInfo { code: "it",    name: "Italian",               hy_mt_name: "Italian",         is_chinese_family: false },
    LanguageInfo { code: "nl",    name: "Dutch",                 hy_mt_name: "Dutch",           is_chinese_family: false },
    LanguageInfo { code: "pl",    name: "Polish",                hy_mt_name: "Polish",          is_chinese_family: false },
    LanguageInfo { code: "cs",    name: "Czech",                 hy_mt_name: "Czech",           is_chinese_family: false },
    LanguageInfo { code: "ru",    name: "Russian",               hy_mt_name: "Russian",         is_chinese_family: false },
    LanguageInfo { code: "uk",    name: "Ukrainian",             hy_mt_name: "Ukrainian",       is_chinese_family: false },
    LanguageInfo { code: "ja",    name: "Japanese",              hy_mt_name: "Japanese",        is_chinese_family: false },
    LanguageInfo { code: "ko",    name: "Korean",                hy_mt_name: "Korean",          is_chinese_family: false },
    LanguageInfo { code: "ar",    name: "Arabic",                hy_mt_name: "Arabic",          is_chinese_family: false },
    LanguageInfo { code: "he",    name: "Hebrew",                hy_mt_name: "Hebrew",          is_chinese_family: false },
    LanguageInfo { code: "fa",    name: "Persian",               hy_mt_name: "Persian",         is_chinese_family: false },
    LanguageInfo { code: "tr",    name: "Turkish",               hy_mt_name: "Turkish",         is_chinese_family: false },
    LanguageInfo { code: "th",    name: "Thai",                  hy_mt_name: "Thai",            is_chinese_family: false },
    LanguageInfo { code: "vi",    name: "Vietnamese",            hy_mt_name: "Vietnamese",      is_chinese_family: false },
    LanguageInfo { code: "ms",    name: "Malay",                 hy_mt_name: "Malay",           is_chinese_family: false },
    LanguageInfo { code: "id",    name: "Indonesian",            hy_mt_name: "Indonesian",      is_chinese_family: false },
    LanguageInfo { code: "tl",    name: "Filipino",              hy_mt_name: "Filipino",        is_chinese_family: false },
    LanguageInfo { code: "hi",    name: "Hindi",                 hy_mt_name: "Hindi",           is_chinese_family: false },
    LanguageInfo { code: "bn",    name: "Bengali",               hy_mt_name: "Bengali",         is_chinese_family: false },
    LanguageInfo { code: "gu",    name: "Gujarati",              hy_mt_name: "Gujarati",        is_chinese_family: false },
    LanguageInfo { code: "ur",    name: "Urdu",                  hy_mt_name: "Urdu",            is_chinese_family: false },
    LanguageInfo { code: "te",    name: "Telugu",                hy_mt_name: "Telugu",          is_chinese_family: false },
    LanguageInfo { code: "mr",    name: "Marathi",               hy_mt_name: "Marathi",         is_chinese_family: false },
    LanguageInfo { code: "ta",    name: "Tamil",                 hy_mt_name: "Tamil",           is_chinese_family: false },
    LanguageInfo { code: "km",    name: "Khmer",                 hy_mt_name: "Khmer",           is_chinese_family: false },
    LanguageInfo { code: "my",    name: "Burmese",               hy_mt_name: "Burmese",         is_chinese_family: false },
    LanguageInfo { code: "kk",    name: "Kazakh",                hy_mt_name: "Kazakh",          is_chinese_family: false },
    LanguageInfo { code: "mn",    name: "Mongolian",             hy_mt_name: "Mongolian",       is_chinese_family: false },
    LanguageInfo { code: "ug",    name: "Uyghur",                hy_mt_name: "Uyghur",          is_chinese_family: false },
    LanguageInfo { code: "bo",    name: "Tibetan",               hy_mt_name: "Tibetan",         is_chinese_family: false },
];
