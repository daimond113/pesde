use indicatif::MultiProgress;
use pesde::Project;

mod add;
mod auth;
mod config;
mod execute;
mod init;
mod install;
mod outdated;
#[cfg(feature = "patches")]
mod patch;
#[cfg(feature = "patches")]
mod patch_commit;
mod publish;
mod run;
mod self_install;
mod self_upgrade;
mod update;

#[derive(Debug, clap::Subcommand)]
pub enum Subcommand {
    /// Authentication-related commands
    #[command(subcommand)]
    Auth(auth::AuthCommands),

    /// Configuration-related commands
    #[command(subcommand)]
    Config(config::ConfigCommands),

    /// Initializes a manifest file in the current directory
    Init(init::InitCommand),

    /// Runs a script, an executable package, or a file with Lune
    Run(run::RunCommand),

    /// Installs all dependencies for the project
    Install(install::InstallCommand),

    /// Publishes the project to the registry
    Publish(publish::PublishCommand),

    /// Installs the pesde binary and scripts
    SelfInstall(self_install::SelfInstallCommand),

    /// Sets up a patching environment for a package
    #[cfg(feature = "patches")]
    Patch(patch::PatchCommand),

    /// Finalizes a patching environment for a package
    #[cfg(feature = "patches")]
    PatchCommit(patch_commit::PatchCommitCommand),

    /// Installs the latest version of pesde
    SelfUpgrade(self_upgrade::SelfUpgradeCommand),

    /// Adds a dependency to the project
    Add(add::AddCommand),

    /// Updates the project's lockfile. Run install to apply changes
    Update(update::UpdateCommand),

    /// Checks for outdated dependencies
    Outdated(outdated::OutdatedCommand),

    /// Executes a binary package without needing to be run in a project directory
    #[clap(name = "x", visible_alias = "execute", visible_alias = "exec")]
    Execute(execute::ExecuteCommand),
}

impl Subcommand {
    pub fn run(
        self,
        project: Project,
        multi: MultiProgress,
        reqwest: reqwest::blocking::Client,
    ) -> anyhow::Result<()> {
        match self {
            Subcommand::Auth(auth) => auth.run(project, reqwest),
            Subcommand::Config(config) => config.run(),
            Subcommand::Init(init) => init.run(project),
            Subcommand::Run(run) => run.run(project),
            Subcommand::Install(install) => install.run(project, multi, reqwest),
            Subcommand::Publish(publish) => publish.run(project, reqwest),
            Subcommand::SelfInstall(self_install) => self_install.run(),
            #[cfg(feature = "patches")]
            Subcommand::Patch(patch) => patch.run(project, reqwest),
            #[cfg(feature = "patches")]
            Subcommand::PatchCommit(patch_commit) => patch_commit.run(project),
            Subcommand::SelfUpgrade(self_upgrade) => self_upgrade.run(reqwest),
            Subcommand::Add(add) => add.run(project),
            Subcommand::Update(update) => update.run(project, multi, reqwest),
            Subcommand::Outdated(outdated) => outdated.run(project),
            Subcommand::Execute(execute) => execute.run(project, reqwest),
        }
    }
}
