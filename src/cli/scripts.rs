use crate::{
    cli::{config::read_config, home_dir},
    util::authenticate_conn,
};
use anyhow::Context;
use gix::remote::Direction;
use pesde::Project;

pub fn update_scripts_folder(project: &Project) -> anyhow::Result<()> {
    let scripts_dir = home_dir()?.join("scripts");

    if scripts_dir.exists() {
        let repo = gix::open(&scripts_dir).context("failed to open scripts repository")?;

        let remote = repo
            .find_default_remote(Direction::Fetch)
            .context("missing default remote of scripts repository")?
            .context("failed to find default remote of scripts repository")?;

        let mut connection = remote
            .connect(Direction::Fetch)
            .context("failed to connect to default remote of scripts repository")?;

        authenticate_conn(&mut connection, project.auth_config());

        let results = connection
            .prepare_fetch(gix::progress::Discard, Default::default())
            .context("failed to prepare scripts repository fetch")?
            .receive(gix::progress::Discard, &false.into())
            .context("failed to receive new scripts repository contents")?;

        let remote_ref = results
            .ref_map
            .remote_refs
            .first()
            .context("failed to get remote refs of scripts repository")?;

        let unpacked = remote_ref.unpack();
        let oid = unpacked
            .1
            .or(unpacked.2)
            .context("couldn't find oid in remote ref")?;

        let tree = repo
            .find_object(oid)
            .context("failed to find scripts repository tree")?
            .peel_to_tree()
            .context("failed to peel scripts repository object to tree")?;

        let mut index = gix::index::File::from_state(
            gix::index::State::from_tree(&tree.id, &repo.objects, Default::default())
                .context("failed to create index state from scripts repository tree")?,
            repo.index_path(),
        );

        let opts = gix::worktree::state::checkout::Options {
            overwrite_existing: true,
            destination_is_initially_empty: false,
            ..Default::default()
        };

        gix::worktree::state::checkout(
            &mut index,
            repo.work_dir().context("scripts repo is bare")?,
            repo.objects
                .clone()
                .into_arc()
                .context("failed to clone objects")?,
            &gix::progress::Discard,
            &gix::progress::Discard,
            &false.into(),
            opts,
        )
        .context("failed to checkout scripts repository")?;

        index
            .write(gix::index::write::Options::default())
            .context("failed to write index")?;
    } else {
        std::fs::create_dir_all(&scripts_dir).context("failed to create scripts directory")?;

        let cli_config = read_config()?;

        gix::prepare_clone(cli_config.scripts_repo, &scripts_dir)
            .context("failed to prepare scripts repository clone")?
            .fetch_then_checkout(gix::progress::Discard, &false.into())
            .context("failed to fetch and checkout scripts repository")?
            .0
            .main_worktree(gix::progress::Discard, &false.into())
            .context("failed to set scripts repository as main worktree")?;
    };

    Ok(())
}
