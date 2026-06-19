use super::languages::{hy_mt_language_name, is_chinese_family};

fn is_v2(version: &str) -> bool {
    version.to_lowercase().contains("mt2")
}

/// Builds the model-appropriate prompt. HY-MT1.5 keeps its original templates;
/// Hy-MT2 uses its English instruction templates and supports extra modes.
///
/// `mode` ∈ standard | contextual | formatted | style | structured | delimiter.
pub fn build_prompt(
    version: &str,
    source_text: &str,
    source_lang: &str,
    target_lang: &str,
    glossary: Option<&[(&str, &str)]>,
    context: Option<&str>,
    mode: &str,
    style: Option<&str>,
) -> String {
    if is_v2(version) {
        build_v2(source_text, target_lang, glossary, context, mode, style)
    } else {
        build_v1(
            source_text,
            source_lang,
            target_lang,
            glossary,
            context,
            mode == "formatted",
        )
    }
}

/// Hy-MT2 prompts (official English instruction templates).
fn build_v2(
    source_text: &str,
    target_lang: &str,
    glossary: Option<&[(&str, &str)]>,
    context: Option<&str>,
    mode: &str,
    style: Option<&str>,
) -> String {
    let tgt = hy_mt_language_name(target_lang);

    match mode {
        "contextual" => {
            if let Some(ctx) = context.filter(|c| !c.is_empty()) {
                return format!(
                    "[Background Information]\n{ctx}\n\nPlease translate the following text into {tgt}, taking the provided background information into consideration.\n\n[Source Text]\n{source_text}"
                );
            }
        }
        "style" => {
            let s = style.unwrap_or("").trim();
            if !s.is_empty() {
                return format!(
                    "Please translate the following text into {tgt}. Note that the translation style must strictly conform to [{s}]:\n\n{source_text}"
                );
            }
        }
        "structured" => {
            return format!(
                "Translate the user-facing text within the following data into {tgt}. Note that you should only output the translated result without any additional explanation. NEVER translate or alter code, tags, keys, property names, or variable placeholders:\n\n{source_text}"
            );
        }
        "delimiter" => {
            return format!(
                "Please accurately translate the following text into {tgt}. You must retain the exact same number of delimiters in the translation; do not omit, escape, or translate these symbols, and pay close attention to their placement. Only output the translated result:\n\n{source_text}"
            );
        }
        _ => {}
    }

    // Terminology takes effect whenever a glossary is supplied.
    if let Some(terms) = glossary.filter(|t| !t.is_empty()) {
        let lines: String = terms
            .iter()
            .map(|(s, t)| format!("{s} translates to {t}"))
            .collect::<Vec<_>>()
            .join("\n");
        return format!(
            "Reference the following translations:\n{lines}\n\nTranslate the following text into {tgt}. Note that you must ONLY output the translated result without any additional explanation:\n\n{source_text}"
        );
    }

    // Default.
    format!(
        "Translate the following text into {tgt}. Note that you should only output the translated result without any additional explanation:\n\n{source_text}"
    )
}

/// HY-MT1.5 prompts (unchanged original templates).
fn build_v1(
    source_text: &str,
    source_lang: &str,
    target_lang: &str,
    glossary: Option<&[(&str, &str)]>,
    context: Option<&str>,
    formatted: bool,
) -> String {
    let target_name = hy_mt_language_name(target_lang);

    if formatted {
        return format!(
            "将以下<source></source>之间的文本翻译为{target_name}，注意只需要输出翻译后的结果，不要额外解释，原文中的<sn></sn>标签表示标签内文本包含格式信息，需要在译文中相应的位置尽量保留该标签。输出格式为：<target>str</target>\n\n<source>{source_text}</source>"
        );
    }

    if let Some(ctx) = context {
        if !ctx.is_empty() {
            if is_chinese_family(source_lang) || is_chinese_family(target_lang) {
                return format!(
                    "{ctx}\n参考上面的信息，把下面的文本翻译成{target_name}，注意不需要翻译上文，也不要额外解释:\n{source_text}"
                );
            } else {
                return format!(
                    "{ctx}\nUsing the above as context, translate the following into {target_name} without additional explanation:\n{source_text}"
                );
            }
        }
    }

    if let Some(terms) = glossary {
        if !terms.is_empty() {
            if is_chinese_family(source_lang) || is_chinese_family(target_lang) {
                let term_lines: String = terms
                    .iter()
                    .map(|(s, t)| format!("{s} 翻译成 {t}"))
                    .collect::<Vec<_>>()
                    .join("\n");
                return format!(
                    "参考下面的翻译:\n{term_lines}\n\n将以下文本翻译为{target_name}，注意只需要输出翻译后的结果，不要额外解释:\n{source_text}"
                );
            } else {
                let term_lines_en: String = terms
                    .iter()
                    .map(|(s, t)| format!("{s} → {t}"))
                    .collect::<Vec<_>>()
                    .join("\n");
                return format!(
                    "Use the following terminology:\n{term_lines_en}\n\nTranslate the following segment into {target_name}, without additional explanation.\n\n{source_text}"
                );
            }
        }
    }

    if is_chinese_family(source_lang) || is_chinese_family(target_lang) {
        format!(
            "将以下文本翻译为{target_name}，注意只需要输出翻译后的结果，不要额外解释:\n\n{source_text}"
        )
    } else {
        format!(
            "Translate the following segment into {target_name}, without additional explanation.\n\n{source_text}"
        )
    }
}
