use super::languages::{hy_mt_language_name, is_chinese_family};

/// Builds the exact HY-MT prompt for a given translation request.
/// Uses the official templates from the README verbatim.
pub fn build_prompt(
    source_text: &str,
    source_lang: &str,    // resolved, not "auto"
    target_lang: &str,
    glossary: Option<&[(&str, &str)]>,
    context: Option<&str>,
    formatted: bool,
) -> String {
    let target_name = hy_mt_language_name(target_lang);

    // Formatted translation (preserves <sn> tags)
    if formatted {
        return format!(
            "将以下<source></source>之间的文本翻译为{target_name}，注意只需要输出翻译后的结果，不要额外解释，原文中的<sn></sn>标签表示标签内文本包含格式信息，需要在译文中相应的位置尽量保留该标签。输出格式为：<target>str</target>\n\n<source>{source_text}</source>"
        );
    }

    // Contextual translation
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

    // Terminology intervention
    if let Some(terms) = glossary {
        if !terms.is_empty() {
            let term_lines: String = terms
                .iter()
                .map(|(s, t)| format!("{s} 翻译成 {t}"))
                .collect::<Vec<_>>()
                .join("\n");

            if is_chinese_family(source_lang) || is_chinese_family(target_lang) {
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

    // Standard translation
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
