//! ```sh
//! FREIGHTER_CLIENT_AUTH_TOKEN=x cargo r -p freighter-client -F binary -- 'http://localhost:3000/index' publish test-1.0.0.crate
//! ```

use clap::{Parser, Subcommand};
use freighter_api_types::index::request::Publish;
use freighter_api_types::index::request::PublishDependency;
use freighter_api_types::index::DependencyKind;
use freighter_client::Client;
use semver::VersionReq;
use std::io::Read;
use std::path::{Component, PathBuf};

#[derive(Parser, Debug)]
#[command(version, about)]
pub struct Args {
    /// Registry http URL (`http://rs.example.com/index`)
    registry_url: String,
    #[arg(env = "FREIGHTER_CLIENT_AUTH_TOKEN")]
    auth_token: Option<String>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    Register { username: String },
    Publish { crate_tarball: PathBuf },
}

#[tokio::main]
async fn main() {
    let args = Args::parse();

    let mut client = Client::new(&args.registry_url, args.auth_token).await.unwrap();

    match args.command {
        Commands::Register { username } => {
            client.register(&username).await.unwrap();
            println!("{}", client.token().unwrap_or_default());
        },
        Commands::Publish { crate_tarball } => {
            let tarball = std::fs::read(crate_tarball).expect("tarball file");
            let manifest = cargo_toml_from_tarball(&tarball).expect("tarball");
            let package = manifest.package();
            let all_deps = manifest.dependencies.iter().map(|(key, dep)| (key.as_str(), dep, DependencyKind::Normal))
                .chain(manifest.build_dependencies.iter().map(|(key, dep)| (key.as_str(), dep, DependencyKind::Build)))
                .map(|(key, dep, kind)| (key, dep, kind, None))
                .chain(manifest.target.iter().flat_map(|(target, tdeps)| {
                    let target = Some(target.as_str());
                    tdeps.dependencies.iter().map(|(key, dep)| (key.as_str(), dep, DependencyKind::Normal))
                        .chain(tdeps.build_dependencies.iter().map(|(key, dep)| (key.as_str(), dep, DependencyKind::Build)))
                        .map(move |(key, dep, kind)| (key, dep, kind, target))
                }));
            let published = client
                .publish(
                    &Publish {
                        name: package.name().into(),
                        vers: package.version().parse().unwrap(),
                        deps: all_deps.map(|(key, dep, kind, target)| PublishDependency {
                            name: key.to_string(),
                            version_req: VersionReq::parse(dep.req()).unwrap(),
                            features: dep.req_features().to_vec(),
                            optional: dep.optional(),
                            default_features: dep.detail().map_or(true, |det| det.default_features),
                            target: target.map(From::from),
                            kind,
                            registry: if dep.detail().is_some_and(|d| d.registry.is_some()) { None } else { Some("https://github.com/rust-lang/crates.io-index".into()) },
                            explicit_name_in_toml: dep.package().map(From::from),
                        }).collect(),
                        authors: vec![],
                        description: package.description().map(From::from),
                        documentation: package.documentation().map(From::from),
                        homepage: package.homepage().map(From::from),
                        readme: None,
                        readme_file: None,
                        keywords: vec![],
                        categories: vec![],
                        license: None,
                        license_file: None,
                        repository: package.repository().map(From::from),
                        badges: None,
                        links: package.links().map(From::from),
                        features: manifest.features.into_iter().collect(),
                    },
                    &tarball,
                )
                .await
                .expect("publish");
            println!("{published:#?}");
        },
    }
}

fn cargo_toml_from_tarball(crate_tarball: &[u8]) -> anyhow::Result<cargo_toml::Manifest> {
    let mut untar = crate_untar::Unarchiver::new(crate_tarball)?;
    let entries = untar.entries()?;
    for e in entries {
        let mut e = e?;
        let path = e.path().unwrap();
        if path.components().nth(1).unwrap() == Component::Normal("Cargo.toml".as_ref()) {
            let mut buf = Vec::new();
            e.read_to_end(&mut buf)?;
            return Ok(cargo_toml::Manifest::from_slice(&buf)?);
        }
    }
    anyhow::bail!("Can't find Cargo.toml")
}
