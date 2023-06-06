use aho_corasick::{AhoCorasick, PatternID};
use std::{error::Error, ffi::OsStr};
use tempfile::NamedTempFile;

pub type TestResult<T> = std::result::Result<T, Box<dyn Error>>;

pub fn get_matches<I, P>(patterns: I, output: String) -> TestResult<Vec<(PatternID, usize)>>
where
    I: IntoIterator<Item = P>,
    P: AsRef<[u8]>,
{
    let ac = AhoCorasick::new(patterns)?;
    let mut matches = vec![];
    for mat in ac.find_iter(output.as_str()) {
        add_match_to_vector(&mut matches, mat);
    }
    Ok(matches)
}

pub fn add_match_to_vector(matches: &mut Vec<(PatternID, usize)>, mat: aho_corasick::Match) {
    matches.push((mat.pattern(), mat.end() - mat.start()))
}

pub fn get_temp_file() -> TestResult<NamedTempFile> {
    Ok(NamedTempFile::new()?)
}

pub fn files_differ(
    path_left: impl AsRef<OsStr>,
    path_right: impl AsRef<OsStr>,
) -> TestResult<bool> {
    // diff the directories
    #[cfg(not(windows))]
    {
        let proc = std::process::Command::new("diff")
            .arg(path_left)
            .arg(path_right)
            .output()?;

        if proc.stdout.is_empty() {
            return Ok(false);
        }
    }

    #[cfg(windows)]
    {
        let proc = std::process::Command::new("fc.exe")
            .arg("/L")
            .arg(path_left)
            .arg(path_right)
            .output()?;

        let output = String::from_utf8(proc.stdout)?;

        dbg!(&output);

        let patterns = &["FC: no differences encountered"];
        let ac = AhoCorasick::new(patterns)?;
        let mut matches = vec![];

        for mat in ac.find_iter(output.as_str()) {
            matches.push((mat.pattern(), mat.end() - mat.start()));
        }

        if matches == vec![(PatternID::must(0), 30)] {
            return Ok(false);
        }
    }

    Ok(true)
}
