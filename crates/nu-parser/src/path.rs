use std::borrow::Cow;

const EXPAND_STR: &str = if cfg!(windows) { r"..\" } else { "../" };

fn handle_dots_push(string: &mut String, count: u8) {
    if count < 1 {
        return;
    }

    if count == 1 {
        string.push('.');
        return;
    }

    for _ in 0..(count - 1) {
        string.push_str(EXPAND_STR);
    }

    string.pop(); // remove last '/'
}

pub fn expand_ndots(path: &str) -> Cow<'_, str> {
    // helpers
    #[cfg(windows)]
    fn is_separator(c: char) -> bool {
        // AFAIK, Windows can have both \ and / as path components separators
        (c == '/') || (c == '\\')
    }

    #[cfg(not(windows))]
    fn is_separator(c: char) -> bool {
        c == '/'
    }

    // find if we need to expand any >2 dot paths and early exit if not
    let mut dots_count = 0u8;
    let ndots_present = {
        for chr in path.chars() {
            if chr == '.' {
                dots_count += 1;
            } else {
                if is_separator(chr) && (dots_count > 2) {
                    // this path component had >2 dots
                    break;
                }

                dots_count = 0;
            }
        }

        dots_count > 2
    };

    if !ndots_present {
        return path.into();
    }

    let mut dots_count = 0u8;
    let mut expanded = String::new();
    for chr in path.chars() {
        if chr == '.' {
            dots_count += 1;
        } else {
            if is_separator(chr) {
                // check for dots expansion only at path component boundaries
                handle_dots_push(&mut expanded, dots_count);
                dots_count = 0;
            } else {
                // got non-dot within path component => do not expand any dots
                while dots_count > 0 {
                    expanded.push('.');
                    dots_count -= 1;
                }
            }
            expanded.push(chr);
        }
    }

    handle_dots_push(&mut expanded, dots_count);

    expanded.into()
}

pub fn expand_path<'a>(path: &'a str) -> Cow<'a, str> {
    let tilde_expansion: Cow<'a, str> = shellexpand::tilde(path);
    let ndots_expansion: Cow<'a, str> = match tilde_expansion {
        Cow::Borrowed(b) => expand_ndots(b),
        Cow::Owned(o) => expand_ndots(&o).to_string().into(),
    };

    ndots_expansion
}

#[cfg(test)]
mod tests {
    use super::*;

    // common tests
    #[test]
    fn string_without_ndots() {
        assert_eq!("../hola", &expand_ndots("../hola").to_string());
    }

    #[test]
    fn string_with_three_ndots_and_chars() {
        assert_eq!("a...b", &expand_ndots("a...b").to_string());
    }

    #[test]
    fn string_with_two_ndots_and_chars() {
        assert_eq!("a..b", &expand_ndots("a..b").to_string());
    }

    #[test]
    fn string_with_one_dot_and_chars() {
        assert_eq!("a.b", &expand_ndots("a.b").to_string());
    }

    // Windows tests
    #[cfg(windows)]
    #[test]
    fn string_with_three_ndots() {
        assert_eq!(r"..\..", &expand_ndots("...").to_string());
    }

    #[cfg(windows)]
    #[test]
    fn string_with_mixed_ndots_and_chars() {
        assert_eq!(
            r"a...b/./c..d/../e.f/..\..\..//.",
            &expand_ndots("a...b/./c..d/../e.f/....//.").to_string()
        );
    }

    #[cfg(windows)]
    #[test]
    fn string_with_three_ndots_and_final_slash() {
        assert_eq!(r"..\../", &expand_ndots(".../").to_string());
    }

    #[cfg(windows)]
    #[test]
    fn string_with_three_ndots_and_garbage() {
        assert_eq!(
            r"ls ..\../ garbage.*[",
            &expand_ndots("ls .../ garbage.*[").to_string(),
        );
    }

    // non-Windows tests
    #[cfg(not(windows))]
    #[test]
    fn string_with_three_ndots() {
        assert_eq!(r"../..", &expand_ndots("...").to_string());
    }

    #[cfg(not(windows))]
    #[test]
    fn string_with_mixed_ndots_and_chars() {
        assert_eq!(
            "a...b/./c..d/../e.f/../../..//.",
            &expand_ndots("a...b/./c..d/../e.f/....//.").to_string()
        );
    }

    #[cfg(not(windows))]
    #[test]
    fn string_with_three_ndots_and_final_slash() {
        assert_eq!("../../", &expand_ndots(".../").to_string());
    }

    #[cfg(not(windows))]
    #[test]
    fn string_with_three_ndots_and_garbage() {
        assert_eq!(
            "ls ../../ garbage.*[",
            &expand_ndots("ls .../ garbage.*[").to_string(),
        );
    }
}
