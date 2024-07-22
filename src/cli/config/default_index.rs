use crate::cli::{read_config, write_config, CliConfig};
use clap::Args;
use pesde::Project;

#[derive(Debug, Args)]
pub struct DefaultIndexCommand {
    /// The new index URL to set as default, don't pass any value to check the current default index
    #[arg(index = 1)]
    index: Option<url::Url>,

    /// Resets the default index to the default value
    #[arg(short, long, conflicts_with = "index")]
    reset: bool,
}

impl DefaultIndexCommand {
    pub fn run(self, project: Project) -> anyhow::Result<()> {
        let mut config = read_config(project.data_dir())?;

        let index = if self.reset {
            Some(CliConfig::default().default_index)
        } else {
            self.index
        };

        match index {
            Some(index) => {
                config.default_index = index.clone();
                write_config(project.data_dir(), &config)?;
                println!("default index set to: {index}");
            }
            None => {
                println!("current default index: {}", config.default_index);
            }
        }

        Ok(())
    }
}
