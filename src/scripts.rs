use crate::manifest::Manifest;
use relative_path::RelativePathBuf;
use std::{
    ffi::OsStr,
    io::{BufRead, BufReader},
    process::{Command, Stdio},
    thread::spawn,
};

pub fn execute_lune_script<A: IntoIterator<Item = S>, S: AsRef<OsStr>>(
    script_name: Option<&str>,
    script_path: &RelativePathBuf,
    args: A,
) -> Result<(), std::io::Error> {
    match Command::new("lune")
        .arg("run")
        .arg(script_path.as_str())
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(mut child) => {
            let stdout = BufReader::new(child.stdout.take().unwrap());
            let stderr = BufReader::new(child.stderr.take().unwrap());

            let script = match script_name {
                Some(script) => script.to_string(),
                None => script_path.to_string(),
            };

            let script_2 = script.to_string();

            spawn(move || {
                for line in stderr.lines() {
                    match line {
                        Ok(line) => {
                            log::error!("[{script}]: {line}");
                        }
                        Err(e) => {
                            log::error!("ERROR IN READING STDERR OF {script}: {e}");
                            break;
                        }
                    }
                }
            });

            for line in stdout.lines() {
                match line {
                    Ok(line) => {
                        log::info!("[{script_2}]: {line}");
                    }
                    Err(e) => {
                        log::error!("ERROR IN READING STDOUT OF {script_2}: {e}");
                        break;
                    }
                }
            }
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            log::warn!("Lune could not be found in PATH: {e}")
        }
        Err(e) => return Err(e),
    };

    Ok(())
}

pub fn execute_script<A: IntoIterator<Item = S>, S: AsRef<OsStr>>(
    manifest: &Manifest,
    script: &str,
    args: A,
) -> Result<(), std::io::Error> {
    if let Some(script_path) = manifest.scripts.get(script) {
        return execute_lune_script(Some(script), script_path, args);
    }

    Ok(())
}
