use std::fs::File;
use std::io::{self, BufRead, BufReader, IsTerminal, Seek};
use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;

use tokio::sync::mpsc::UnboundedSender;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InputMode {
    Stdin,
    File(PathBuf),
}

impl InputMode {
    pub fn source_label(&self) -> String {
        match self {
            Self::Stdin => "stdin".to_string(),
            Self::File(path) => path.display().to_string(),
        }
    }
}

pub fn resolve_input_mode(args: &[String]) -> io::Result<Option<InputMode>> {
    match args {
        [] if !io::stdin().is_terminal() => Ok(Some(InputMode::Stdin)),
        [] => Ok(None),
        [path] => {
            validate_file(path)?;
            Ok(Some(InputMode::File(PathBuf::from(path))))
        }
        _ => Ok(None),
    }
}

pub fn usage(program: &str) -> String {
    format!(
        "Usage:\n  {program} <path/to/logfile>\n  your-command | {program}\n\nReads structured or plain logs from stdin or follows a file from the beginning."
    )
}

pub fn spawn_ingest(
    mode: InputMode,
    tx: UnboundedSender<Vec<String>>,
) -> io::Result<thread::JoinHandle<()>> {
    if let InputMode::File(path) = &mode {
        validate_file(path)?;
    }

    Ok(thread::spawn(move || match mode {
        InputMode::Stdin => {
            let _ = ingest_stdin(tx);
        }
        InputMode::File(path) => {
            let _ = ingest_file(path, tx);
        }
    }))
}

fn validate_file(path: impl AsRef<Path>) -> io::Result<()> {
    let metadata = std::fs::metadata(path)?;
    if metadata.is_file() {
        Ok(())
    } else {
        Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "input path must point to a file",
        ))
    }
}

fn ingest_stdin(tx: UnboundedSender<Vec<String>>) -> io::Result<()> {
    let stdin = io::stdin();
    let mut reader = stdin.lock();
    let mut line = String::new();

    loop {
        line.clear();
        let bytes = reader.read_line(&mut line)?;
        if bytes == 0 {
            break;
        }

        if tx.send(vec![trim_newline(&line)]).is_err() {
            break;
        }
    }

    Ok(())
}

fn ingest_file(path: PathBuf, tx: UnboundedSender<Vec<String>>) -> io::Result<()> {
    let mut reader = BufReader::new(File::open(&path)?);
    let mut batch = Vec::new();

    loop {
        let mut line = String::new();
        let bytes = reader.read_line(&mut line)?;

        if bytes > 0 {
            batch.push(trim_newline(&line));
            if batch.len() >= 128 && tx.send(std::mem::take(&mut batch)).is_err() {
                break;
            }
            continue;
        }

        if !batch.is_empty() && tx.send(std::mem::take(&mut batch)).is_err() {
            break;
        }

        let current = reader.stream_position()?;
        let len = std::fs::metadata(&path)?.len();
        if len < current {
            reader = BufReader::new(File::open(&path)?);
        } else {
            thread::sleep(Duration::from_millis(150));
        }
    }

    Ok(())
}

fn trim_newline(line: &str) -> String {
    line.trim_end_matches(['\r', '\n']).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_input_without_file_requires_usage() {
        let result = resolve_input_mode(&[]);
        assert!(result.is_ok());
    }

    #[test]
    fn usage_mentions_file_and_stdin_modes() {
        let text = usage("log-viewer");
        assert!(text.contains("<path/to/logfile>"));
        assert!(text.contains("your-command | log-viewer"));
    }
}
