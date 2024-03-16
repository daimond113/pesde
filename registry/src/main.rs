use std::{fs::read_dir, sync::Mutex, time::Duration};

use actix_cors::Cors;
use actix_governor::{Governor, GovernorConfigBuilder, KeyExtractor, SimpleKeyExtractionError};
use actix_web::{
    dev::ServiceRequest,
    error::ErrorUnauthorized,
    middleware::{Compress, Condition, Logger},
    rt::System,
    web, App, Error, HttpMessage, HttpServer,
};
use actix_web_httpauth::{extractors::bearer::BearerAuth, middleware::HttpAuthentication};
use dotenvy::dotenv;
use git2::{Cred, Signature};
use log::info;
use reqwest::{header::AUTHORIZATION, Client};
use rusty_s3::{Bucket, Credentials, UrlStyle};
use tantivy::{doc, DateTime, IndexReader, IndexWriter};

use pesde::{
    index::{GitIndex, IndexFile},
    package_name::PackageName,
};

mod endpoints;
mod errors;

const S3_EXPIRY: Duration = Duration::from_secs(60 * 60);

struct AppState {
    s3_bucket: Bucket,
    s3_credentials: Credentials,
    reqwest_client: Client,
    index: Mutex<GitIndex>,

    search_reader: IndexReader,
    search_writer: Mutex<IndexWriter>,
}

macro_rules! get_env {
    ($name:expr, "p") => {
        std::env::var($name)
            .expect(concat!("Environment variable `", $name, "` must be set"))
            .parse()
            .expect(concat!(
                "Environment variable `",
                $name,
                "` must be a valid value"
            ))
    };
    ($name:expr) => {
        std::env::var($name).expect(concat!("Environment variable `", $name, "` must be set"))
    };
    ($name:expr, $default:expr, "p") => {
        std::env::var($name)
            .unwrap_or($default.to_string())
            .parse()
            .expect(concat!(
                "Environment variable `",
                $name,
                "` must a valid value"
            ))
    };
    ($name:expr, $default:expr) => {
        std::env::var($name).unwrap_or($default.to_string())
    };
}

pub fn commit_signature<'a>() -> Signature<'a> {
    Signature::now(
        &get_env!("COMMITTER_GIT_NAME"),
        &get_env!("COMMITTER_GIT_EMAIL"),
    )
    .unwrap()
}

#[derive(Debug, Clone, Copy, Hash, Eq, PartialEq)]
pub struct UserId(pub u64);

async fn validator(
    req: ServiceRequest,
    credentials: BearerAuth,
) -> Result<ServiceRequest, (Error, ServiceRequest)> {
    let token = credentials.token();
    let app_state = req.app_data::<web::Data<AppState>>().unwrap();

    let Ok(user_info) = app_state
        .reqwest_client
        .get("https://api.github.com/user")
        .header(AUTHORIZATION, format!("Bearer {}", token))
        .send()
        .await
        .map(|r| r.json::<serde_json::Value>())
    else {
        return Err((ErrorUnauthorized("Failed to fetch user info"), req));
    };

    let Ok(user_info) = user_info.await else {
        return Err((ErrorUnauthorized("Failed to parse user info"), req));
    };

    let Some(id) = user_info["id"].as_u64() else {
        return Err((ErrorUnauthorized("Failed to fetch user info"), req));
    };

    req.extensions_mut().insert(UserId(id));

    Ok(req)
}

#[derive(Debug, Clone)]
struct UserIdKey;

impl KeyExtractor for UserIdKey {
    type Key = UserId;
    type KeyExtractionError = SimpleKeyExtractionError<&'static str>;

    fn extract(&self, req: &ServiceRequest) -> Result<Self::Key, Self::KeyExtractionError> {
        Ok(*req.extensions().get::<UserId>().unwrap())
    }
}

fn search_index(index: &GitIndex) -> (IndexReader, IndexWriter) {
    let mut schema_builder = tantivy::schema::SchemaBuilder::new();
    let name =
        schema_builder.add_text_field("name", tantivy::schema::TEXT | tantivy::schema::STORED);
    let version =
        schema_builder.add_text_field("version", tantivy::schema::TEXT | tantivy::schema::STORED);
    let description = schema_builder.add_text_field("description", tantivy::schema::TEXT);
    let published_at = schema_builder.add_date_field("published_at", tantivy::schema::FAST);

    let search_index = tantivy::Index::create_in_ram(schema_builder.build());
    let search_reader = search_index
        .reader_builder()
        .reload_policy(tantivy::ReloadPolicy::OnCommit)
        .try_into()
        .unwrap();
    let mut search_writer = search_index.writer(50_000_000).unwrap();

    for entry in read_dir(index.path()).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();

        if !path.is_dir() || path.file_name().is_some_and(|v| v == ".git") {
            continue;
        }

        let scope = path.file_name().and_then(|v| v.to_str()).unwrap();

        for entry in read_dir(&path).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();

            if !path.is_file() || path.extension().is_some() {
                continue;
            }

            let package = path.file_name().and_then(|v| v.to_str()).unwrap();

            let package_name = PackageName::new(scope, package).unwrap();
            let entries: IndexFile =
                serde_yaml::from_slice(&std::fs::read(&path).unwrap()).unwrap();
            let entry = entries.last().unwrap().clone();

            search_writer
                .add_document(doc!(
                    name => package_name.to_string(),
                    version => entry.version.to_string(),
                    description => entry.description.unwrap_or_default(),
                    published_at => DateTime::from_timestamp_secs(entry.published_at.timestamp()),
                ))
                .unwrap();
        }
    }

    search_writer.commit().unwrap();

    (search_reader, search_writer)
}

fn main() -> std::io::Result<()> {
    dotenv().ok();

    let sentry_url = std::env::var("SENTRY_URL").ok();
    let with_sentry = sentry_url.is_some();

    let mut log_builder = pretty_env_logger::formatted_builder();
    log_builder.parse_env(pretty_env_logger::env_logger::Env::default().default_filter_or("info"));

    if with_sentry {
        let logger = sentry_log::SentryLogger::with_dest(log_builder.build());
        log::set_boxed_logger(Box::new(logger)).unwrap();
        log::set_max_level(log::LevelFilter::Info);
    } else {
        log_builder.try_init().unwrap();
    }

    let _guard = if let Some(sentry_url) = sentry_url {
        std::env::set_var("RUST_BACKTRACE", "1");

        Some(sentry::init((
            sentry_url,
            sentry::ClientOptions {
                release: sentry::release_name!(),
                ..Default::default()
            },
        )))
    } else {
        None
    };

    let address = get_env!("ADDRESS", "127.0.0.1");
    let port: u16 = get_env!("PORT", "8080", "p");

    let current_dir = std::env::current_dir().unwrap();

    let index = GitIndex::new(
        current_dir.join("cache"),
        &get_env!("INDEX_REPO_URL"),
        Some(Box::new(|| {
            Box::new(|_, _, _| {
                let username = get_env!("GITHUB_USERNAME");
                let pat = get_env!("GITHUB_PAT");

                Cred::userpass_plaintext(&username, &pat)
            })
        })),
    );
    index.refresh().expect("failed to refresh index");

    let (search_reader, search_writer) = search_index(&index);

    let app_data = web::Data::new(AppState {
        s3_bucket: Bucket::new(
            get_env!("S3_ENDPOINT", "p"),
            UrlStyle::Path,
            get_env!("S3_BUCKET_NAME"),
            get_env!("S3_REGION"),
        )
        .unwrap(),
        s3_credentials: Credentials::new(get_env!("S3_ACCESS_KEY"), get_env!("S3_SECRET_KEY")),
        reqwest_client: Client::builder()
            .user_agent(concat!(
                env!("CARGO_PKG_NAME"),
                "/",
                env!("CARGO_PKG_VERSION")
            ))
            .build()
            .unwrap(),
        index: Mutex::new(index),

        search_reader,
        search_writer: Mutex::new(search_writer),
    });

    let upload_governor_config = GovernorConfigBuilder::default()
        .burst_size(10)
        .per_second(600)
        .key_extractor(UserIdKey)
        .use_headers()
        .finish()
        .unwrap();

    let generic_governor_config = GovernorConfigBuilder::default()
        .burst_size(50)
        .per_second(10)
        .use_headers()
        .finish()
        .unwrap();

    info!("listening on {address}:{port}");

    System::new().block_on(async move {
        HttpServer::new(move || {
            App::new()
                .wrap(Condition::new(with_sentry, sentry_actix::Sentry::new()))
                .wrap(Logger::default())
                .wrap(Cors::permissive())
                .wrap(Compress::default())
                .app_data(app_data.clone())
                .route("/", web::get().to(|| async { env!("CARGO_PKG_VERSION") }))
                .service(
                    web::scope("/v0")
                        .route(
                            "/search",
                            web::get()
                                .to(endpoints::search::search_packages)
                                .wrap(Governor::new(&generic_governor_config)),
                        )
                        .route(
                            "/packages/{scope}/{name}/versions",
                            web::get()
                                .to(endpoints::packages::get_package_versions)
                                .wrap(Governor::new(&generic_governor_config)),
                        )
                        .route(
                            "/packages/{scope}/{name}/{version}",
                            web::get()
                                .to(endpoints::packages::get_package_version)
                                .wrap(Governor::new(&generic_governor_config)),
                        )
                        .route(
                            "/packages",
                            web::post()
                                .to(endpoints::packages::create_package)
                                .wrap(Governor::new(&upload_governor_config))
                                .wrap(HttpAuthentication::bearer(validator)),
                        ),
                )
        })
        .bind((address, port))?
        .run()
        .await
    })
}
