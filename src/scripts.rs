use crate::Project;
use std::{
    ffi::OsStr,
    fmt::{Display, Formatter},
    io::{BufRead, BufReader},
    path::Path,
    process::{Command, Stdio},
    thread::spawn,
};

/// Script names used by pesde
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ScriptName {
    /// Generates a config for syncing tools for Roblox. For example, for Rojo it should create a `default.project.json` file
    #[cfg(feature = "roblox")]
    RobloxSyncConfigGenerator,
    /// Prints a sourcemap for a Wally package, used for finding the library export file
    #[cfg(feature = "wally-compat")]
    SourcemapGenerator,
}

impl Display for ScriptName {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        #[cfg(feature = "roblox")]
        match self {
            #[cfg(feature = "roblox")]
            ScriptName::RobloxSyncConfigGenerator => write!(f, "roblox_sync_config_generator"),
            #[cfg(feature = "wally-compat")]
            ScriptName::SourcemapGenerator => write!(f, "sourcemap_generator"),
        }
        #[cfg(not(feature = "roblox"))]
        Ok(())
    }
}

pub(crate) fn execute_script<A: IntoIterator<Item = S>, S: AsRef<OsStr>>(
    script_name: ScriptName,
    script_path: &Path,
    args: A,
    project: &Project,
    return_stdout: bool,
) -> Result<Option<String>, std::io::Error> {
    match Command::new("lune")
        .arg("run")
        .arg(script_path.as_os_str())
        .arg("--")
        .args(args)
        .current_dir(project.path())
        .stdin(Stdio::inherit())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(mut child) => {
            let stdout = BufReader::new(child.stdout.take().unwrap());
            let stderr = BufReader::new(child.stderr.take().unwrap());

            let script = script_name.to_string();
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
                        if return_stdout {
                            stdout_str.push_str(&line);
                            stdout_str.push('\n');
                        } else {
                            log::info!("[{script_2}]: {line}");
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
