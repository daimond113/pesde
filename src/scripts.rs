use std::{
    ffi::OsStr,
    io::{BufRead, BufReader},
    path::Path,
    process::{Command, Stdio},
    thread::spawn,
};

pub fn execute_script<A: IntoIterator<Item = S>, S: AsRef<OsStr>, P: AsRef<Path>>(
    script_name: Option<&str>,
    script_path: &Path,
    args: A,
    cwd: P,
    return_stdout: bool,
) -> Result<Option<String>, std::io::Error> {
    match Command::new("lune")
        .arg("run")
        .arg(script_path.as_os_str())
        .args(args)
        .current_dir(cwd)
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
                None => script_path.to_string_lossy().to_string(),
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

            let mut stdout_str = String::new();

            for line in stdout.lines() {
                match line {
                    Ok(line) => {
                        log::info!("[{script_2}]: {line}");

                        if return_stdout {
                            stdout_str.push_str(&line);
                            stdout_str.push('\n');
                        }
                    }
                    Err(e) => {
                        log::error!("ERROR IN READING STDOUT OF {script_2}: {e}");
                        break;
                    }
                }
            }

            if return_stdout {
                Ok(Some(stdout_str))
            } else {
                Ok(None)
            }
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            log::warn!("Lune could not be found in PATH: {e}");

            Ok(None)
        }
        Err(e) => Err(e),
    }
}
