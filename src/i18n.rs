//! Startup language selection shared by the GUI, CLI and upload workers.
use std::sync::OnceLock;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Language {
    Zh,
    En,
}
impl Default for Language {
    fn default() -> Self {
        Self::detect()
    }
}
impl Language {
    /// Chinese locales select Chinese; other locales fall back to English.
    pub fn from_locale(locale: &str) -> Self {
        let language = locale
            .trim()
            .split(['-', '_', '.', '@'])
            .next()
            .unwrap_or("");
        if language.eq_ignore_ascii_case("zh") {
            Self::Zh
        } else {
            Self::En
        }
    }
    pub fn text(self, zh: &'static str, en: &'static str) -> &'static str {
        match self {
            Self::Zh => zh,
            Self::En => en,
        }
    }
    pub fn system() -> Self {
        static LANGUAGE: OnceLock<Language> = OnceLock::new();
        *LANGUAGE.get_or_init(|| platform_language().unwrap_or(Self::En))
    }
    pub fn detect() -> Self {
        match std::env::var("IA_UPLOADER_LANG").as_deref() {
            Ok("zh") => Self::Zh,
            Ok("en") => Self::En,
            _ => Self::system(),
        }
    }
}

// Read the preferred UI language, not LANG (which may only describe formatting).
#[cfg(target_os = "macos")]
fn platform_language() -> Option<Language> {
    let output = std::process::Command::new("/usr/bin/defaults")
        .args(["read", "-g", "AppleLanguages"])
        .output()
        .ok()?;
    if output.status.success() {
        let text = String::from_utf8(output.stdout).ok()?;
        if let Some(language) = apple_language(&text) {
            return Some(language);
        }
    }
    environment_language()
}
#[cfg(any(target_os = "macos", test))]
fn apple_language(value: &str) -> Option<Language> {
    value
        .split(['(', ')', ',', '"', ' ', '\n', '\r', '\t'])
        .find(|token| !token.is_empty())
        .map(Language::from_locale)
}

#[cfg(target_os = "windows")]
fn platform_language() -> Option<Language> {
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetUserDefaultUILanguage() -> u16;
    }
    // The OS function takes no pointers and returns a LANGID.
    let id = unsafe { GetUserDefaultUILanguage() };
    if id == 0 {
        environment_language()
    } else {
        Some(if id & 0x03ff == 0x04 {
            Language::Zh
        } else {
            Language::En
        })
    }
}
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn platform_language() -> Option<Language> {
    environment_language()
}
fn environment_language() -> Option<Language> {
    // LC_ALL/LC_MESSAGES take precedence over the user's preferred fallback list.
    for key in ["LC_ALL", "LC_MESSAGES", "LANGUAGE", "LANG"] {
        if let Ok(value) = std::env::var(key)
            && !value.trim().is_empty()
        {
            return Some(Language::from_locale(value.split(':').next().unwrap_or("")));
        }
    }
    None
}

#[macro_export]
macro_rules! tr {
    ($language:expr, $zh:literal, $en:literal $(, $argument:expr)* $(,)?) => {
        match $language {
            $crate::Language::Zh => format!($zh $(, $argument)*),
            $crate::Language::En => format!($en $(, $argument)*),
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn chinese_locales_and_english_fallback() {
        for locale in [
            "zh",
            "zh-CN",
            "zh-Hans-CN",
            "zh-Hant-TW",
            "zh_TW.UTF-8",
            "ZH_hk",
        ] {
            assert_eq!(Language::from_locale(locale), Language::Zh);
        }
        for locale in [
            "en",
            "en-US",
            "en_GB.UTF-8",
            "fr-FR",
            "C",
            "POSIX",
            "",
            "zhuang",
            "zhx",
        ] {
            assert_eq!(Language::from_locale(locale), Language::En);
        }
    }
    #[test]
    fn macos_uses_first_preferred_language() {
        assert_eq!(
            apple_language("(\n    \"zh-Hans-CN\",\n    \"en-CN\"\n)"),
            Some(Language::Zh)
        );
        assert_eq!(
            apple_language("(\n    \"en-US\",\n    \"zh-Hans\"\n)"),
            Some(Language::En)
        );
        assert_eq!(apple_language("(\n)"), None);
        assert_eq!(
            apple_language("(\"zh-Hant-TW\", \"en-US\")"),
            Some(Language::Zh)
        );
    }
}
