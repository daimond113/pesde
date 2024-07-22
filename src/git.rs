use crate::AuthConfig;

pub fn authenticate_conn(
    conn: &mut gix::remote::Connection<
        '_,
        '_,
        Box<dyn gix::protocol::transport::client::Transport + Send>,
    >,
    auth_config: &AuthConfig,
) {
    if let Some(iden) = auth_config.git_credentials().cloned() {
        conn.set_credentials(move |action| match action {
            gix::credentials::helper::Action::Get(ctx) => {
                Ok(Some(gix::credentials::protocol::Outcome {
                    identity: iden.clone(),
                    next: gix::credentials::helper::NextAction::from(ctx),
                }))
            }
            gix::credentials::helper::Action::Store(_) => Ok(None),
            gix::credentials::helper::Action::Erase(_) => Ok(None),
        });
    }
}
