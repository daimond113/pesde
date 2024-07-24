use crate::AuthConfig;
use gix::bstr::BStr;
use serde::{ser::SerializeMap, Deserialize, Deserializer, Serializer};
use std::collections::BTreeMap;

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

pub fn serialize_gix_url<S: Serializer>(url: &gix::Url, serializer: S) -> Result<S::Ok, S::Error> {
    serializer.serialize_str(&url.to_bstring().to_string())
}

pub fn serialize_gix_url_map<S: Serializer>(
    url: &BTreeMap<String, gix::Url>,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    let mut map = serializer.serialize_map(Some(url.len()))?;
    for (k, v) in url {
        map.serialize_entry(k, &v.to_bstring().to_string())?;
    }
    map.end()
}

pub fn deserialize_gix_url<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<gix::Url, D::Error> {
    let s = String::deserialize(deserializer)?;
    gix::Url::from_bytes(BStr::new(&s)).map_err(serde::de::Error::custom)
}

pub fn deserialize_gix_url_map<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<BTreeMap<String, gix::Url>, D::Error> {
    BTreeMap::<String, String>::deserialize(deserializer)?
        .into_iter()
        .map(|(k, v)| {
            gix::Url::from_bytes(BStr::new(&v))
                .map(|v| (k, v))
                .map_err(serde::de::Error::custom)
        })
        .collect()
}
