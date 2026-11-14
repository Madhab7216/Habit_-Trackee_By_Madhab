//! GNU gettext setup and small, explicit formatting helpers for the GTK interface.
use std::{
    env,
    ffi::{CStr, CString},
    path::PathBuf,
    sync::atomic::{AtomicU8, Ordering},
};

const DOMAIN: &str = "habit-trackers-by-madhab";
const LC_MESSAGES: i32 = 5;
static CURRENT_LANGUAGE: AtomicU8 = AtomicU8::new(0);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Language {
    English,
    Bengali,
}

impl Language {
    pub fn code(self) -> &'static str {
        match self {
            Self::English => "en",
            Self::Bengali => "bn",
        }
    }
}

fn language_from_code(value: &str) -> Language {
    let primary = value
        .split([':', '_', '.', '@'])
        .next()
        .unwrap_or(value)
        .trim()
        .to_ascii_lowercase();
    if primary == "bn" {
        Language::Bengali
    } else {
        Language::English
    }
}

fn detect_system_language() -> Language {
    ["LANGUAGE", "LC_ALL", "LC_MESSAGES", "LANG"]
        .into_iter()
        .filter_map(|key| env::var(key).ok())
        .find(|value| !value.trim().is_empty())
        .map(|value| language_from_code(&value))
        .unwrap_or(Language::English)
}

fn locale_directory() -> PathBuf {
    let development = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("locale");
    if development.is_dir() {
        return development;
    }
    if let Ok(executable) = env::current_exe() {
        if let Some(directory) = executable.parent() {
            for candidate in [
                directory.join("../share/locale"),
                directory.join("../../share/locale"),
            ] {
                if candidate.is_dir() {
                    return candidate;
                }
            }
        }
    }
    PathBuf::from("/usr/share/locale")
}

/// Configure GNU gettext before GTK is initialised. Returns the language for this launch.
pub fn initialize(preferred_language: Option<&str>) -> Language {
    let language = preferred_language
        .filter(|value| !value.trim().is_empty())
        .map(language_from_code)
        .unwrap_or_else(detect_system_language);
    CURRENT_LANGUAGE.store(
        if language == Language::Bengali { 1 } else { 0 },
        Ordering::Relaxed,
    );
    let directory = CString::new(locale_directory().to_string_lossy().as_bytes())
        .expect("locale directory must not contain NUL bytes");
    unsafe {
        let mut locale_result = setlocale(LC_MESSAGES, c"".as_ptr());
        let locale_is_unusable = locale_result.is_null()
            || matches!(CStr::from_ptr(locale_result).to_bytes(), b"C" | b"POSIX");
        if locale_is_unusable {
            locale_result = setlocale(LC_MESSAGES, c"C.UTF-8".as_ptr());
            if locale_result.is_null() {
                locale_result = setlocale(LC_MESSAGES, c"C.utf8".as_ptr());
            }
        }
        // GNU gettext ignores LANGUAGE under the special C/POSIX locales.
        // Prefer an available UTF-8 message locale before activating translations.
        let _ = locale_result;
        env::set_var("LANGUAGE", language.code());
        let _ = bindtextdomain(c"habit-trackers-by-madhab".as_ptr(), directory.as_ptr());
        let _ = bind_textdomain_codeset(c"habit-trackers-by-madhab".as_ptr(), c"UTF-8".as_ptr());
        let _ = textdomain(c"habit-trackers-by-madhab".as_ptr());
    }
    language
}

pub fn current_language() -> Language {
    if CURRENT_LANGUAGE.load(Ordering::Relaxed) == 1 {
        Language::Bengali
    } else {
        Language::English
    }
}

pub fn tr(message: &str) -> String {
    gtk::glib::dgettext(Some(DOMAIN), message).to_string()
}

/// Translate a template containing numbered placeholders such as {0} and {1}.
pub fn format(template: &str, arguments: &[&str]) -> String {
    let mut translated = tr(template);
    for (index, argument) in arguments.iter().enumerate() {
        translated = translated.replace(&format!("{{{index}}}"), argument);
    }
    translated
}

pub fn error(error: &rusqlite::Error) -> String {
    match error {
        rusqlite::Error::InvalidParameterName(message) => tr(message),
        _ => error.to_string(),
    }
}

#[link(name = "c")]
unsafe extern "C" {
    fn setlocale(category: i32, locale: *const std::ffi::c_char) -> *mut std::ffi::c_char;
    fn bindtextdomain(
        domainname: *const std::ffi::c_char,
        dirname: *const std::ffi::c_char,
    ) -> *mut std::ffi::c_char;
    fn bind_textdomain_codeset(
        domainname: *const std::ffi::c_char,
        codeset: *const std::ffi::c_char,
    ) -> *mut std::ffi::c_char;
    fn textdomain(domainname: *const std::ffi::c_char) -> *mut std::ffi::c_char;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn language_selection_normalizes_locale_names() {
        assert_eq!(language_from_code("bn_BD.UTF-8"), Language::Bengali);
        assert_eq!(language_from_code("bn_IN"), Language::Bengali);
        assert_eq!(language_from_code("en_IN.UTF-8"), Language::English);
        assert_eq!(language_from_code("en_US:bn"), Language::English);
    }

    #[test]
    fn supported_language_codes_are_stable() {
        assert_eq!(Language::Bengali.code(), "bn");
        assert_eq!(Language::English.code(), "en");
    }
}
