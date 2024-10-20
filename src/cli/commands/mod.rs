use indicatif::MultiProgress;
use pesde::Project;
use std::thread::JoinHandle;

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
#[cfg(feature = "version-management")]
mod self_install;
#[cfg(feature = "version-management")]
mod self_upgrade;
mod update;

#[derive(Debug, clap::Subcommand)]
pub enum Subcommand {
    /// Authentication-related commands
    Auth(auth::AuthSubcommand),

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
    #[cfg(feature = "version-management")]
    SelfInstall(self_install::SelfInstallCommand),

    /// Sets up a patching environment for a package
    #[cfg(feature = "patches")]
    Patch(patch::PatchCommand),

    /// Finalizes a patching environment for a package
    #[cfg(feature = "patches")]
    PatchCommit(patch_commit::PatchCommitCommand),

    /// Installs the latest version of pesde
    #[cfg(feature = "version-management")]
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
        update_task: JoinHandle<()>,
    ) -> anyhow::Result<()> {
        let mut update_task = Some(update_task);

        let res = match self {
            Subcommand::Auth(auth) => auth.run(project, reqwest),
            Subcommand::Config(config) => config.run(),
            Subcommand::Init(init) => init.run(project),
            Subcommand::Run(run) => run.run(project, &mut update_task),
            Subcommand::Install(install) => install.run(project, multi, reqwest, &mut update_task),
            Subcommand::Publish(publish) => publish.run(project, reqwest),
            #[cfg(feature = "version-management")]
            Subcommand::SelfInstall(self_install) => self_install.run(),
            #[cfg(feature = "patches")]
            Subcommand::Patch(patch) => patch.run(project, reqwest),
            #[cfg(feature = "patches")]
            Subcommand::PatchCommit(patch_commit) => patch_commit.run(project),
            #[cfg(feature = "version-management")]
            Subcommand::SelfUpgrade(self_upgrade) => self_upgrade.run(reqwest),
            Subcommand::Add(add) => add.run(project),
            Subcommand::Update(update) => update.run(project, multi, reqwest, &mut update_task),
            Subcommand::Outdated(outdated) => outdated.run(project),
            Subcommand::Execute(execute) => execute.run(project, reqwest),
        };

        if let Some(handle) = update_task.take() {
            handle.join().expect("failed to join update task");
        }

        res
    }
}
