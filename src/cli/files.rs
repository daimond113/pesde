use std::path::Path;

pub fn make_executable<P: AsRef<Path>>(_path: P) -> anyhow::Result<()> {
    // TODO: test if this actually works
    #[cfg(unix)]
    {
        use anyhow::Context;
        use std::os::unix::fs::PermissionsExt;

        let mut perms = std::fs::metadata(&_path)
            .context("failed to get bin link file metadata")?
            .permissions();
        perms.set_mode(perms.mode() | 0o111);
        std::fs::set_permissions(&_path, perms)
            .context("failed to set bin link file permissions")?;
    }

    Ok(())
}
