use crate::{
    CompletedPublication, CrateVersion, Dependency, IndexClient, IndexError, IndexResult,
    Pagination, Publish, SearchResults, SearchResultsEntry, SearchResultsMeta,
};
use anyhow::Context;
use async_trait::async_trait;
use deadpool_postgres::tokio_postgres::{IsolationLevel, NoTls, Row};
use deadpool_postgres::{Pool, Runtime};
use postgres_types::ToSql;
use semver::{Version, VersionReq};
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;

pub struct PostgreSQLIndex {
    pool: Pool,
}

impl PostgreSQLIndex {
    pub fn new(config: deadpool_postgres::Config) -> IndexResult<Self> {
        let pool = config
            .create_pool(Some(Runtime::Tokio1), NoTls)
            .context("Failed to create db pool")?;

        Ok(Self { pool })
    }

    async fn yank_inner(&self, crate_name: &str, version: &Version, val: bool) -> IndexResult<()> {
        let client = self.pool.get().await.unwrap();

        let statement = client
            .prepare_cached(include_str!("../sql/set-yank.sql"))
            .await
            .context("Failed to prepare yank/unyank statement")?;

        let rows = client
            .query(&statement, &[&crate_name, &version.to_string(), &val])
            .await
            .context("Failed to execute yank/unyank query")?;

        assert!(rows.len() <= 1);

        if rows.len() == 1 {
            Ok(())
        } else {
            Err(IndexError::Conflict(
                "Tried to set yank status to an identical status".to_string(),
            ))
        }
    }
}

#[async_trait]
impl IndexClient for PostgreSQLIndex {
    async fn get_sparse_entry(&self, crate_name: &str) -> IndexResult<Option<Vec<CrateVersion>>> {
        let client = self.pool.get().await.unwrap();

        // prepare these at once to take advantage of pipelining
        let (existential_statement, versions_statement, features_statement, dependencies_statement) =
            tokio::try_join!(
                client.prepare_cached(include_str!("../sql/sparse-index/get-crate.sql")),
                client.prepare_cached(include_str!("../sql/sparse-index/get-versions.sql")),
                client.prepare_cached(include_str!("../sql/sparse-index/get-features.sql")),
                client.prepare_cached(include_str!("../sql/sparse-index/get-dependencies.sql"))
            )
            .context("Failed to prepare transaction")?;

        if let Ok(crate_row) = client
            .query_one(&existential_statement, &[&crate_name])
            .await
        {
            let id: i32 = crate_row.get("id");

            // this is a major hotpath
            let version_rows = client
                .query(&versions_statement, &[&id])
                .await
                .context("Failed to query versions")?;

            let mut versions = Vec::with_capacity(version_rows.len());

            // todo maybe look at running all of this concurrently for pipelining purposes
            for version_row in version_rows {
                let version_id: i32 = version_row.get("id");

                // this shouldn't be necessary but it is nonetheless
                let version_id_query = [&version_id as &(dyn ToSql + Sync)];

                // pipeline the queries
                let (feature_rows, dependency_rows) = tokio::try_join!(
                    client.query(&features_statement, &version_id_query),
                    client.query(&dependencies_statement, &version_id_query)
                )
                .context("Failed to query features or dependencies for crate")?;

                let mut features = HashMap::with_capacity(feature_rows.len());
                let mut deps = Vec::with_capacity(dependency_rows.len());

                for feature_row in feature_rows {
                    features.insert(feature_row.get("name"), feature_row.get("values"));
                }

                for deps_row in dependency_rows {
                    deps.push(Dependency {
                        name: deps_row.get("name"),
                        req: VersionReq::parse(deps_row.get("req"))
                            .context("Failed to parse dependency version req in db")?,
                        features: deps_row.get("features"),
                        optional: deps_row.get("optional"),
                        default_features: deps_row.get("default_features"),
                        target: deps_row.get("target"),
                        kind: deps_row.get("kind"),
                        registry: deps_row.get("registry"),
                        package: deps_row.get("package"),
                    });
                }

                versions.push(CrateVersion {
                    name: crate_name.to_string(),
                    vers: Version::parse(version_row.get("version"))
                        .context("Failed to parse crate version in db")?,
                    deps,
                    cksum: version_row.get("cksum"),
                    features,
                    yanked: version_row.get("yanked"),
                    links: version_row.get("links"),
                    v: 2,
                    // todo maybe scrap
                    features2: HashMap::new(),
                });
            }

            Ok(Some(versions))
        } else {
            Ok(None)
        }
    }

    async fn yank_crate(&self, crate_name: &str, version: &Version) -> IndexResult<()> {
        self.yank_inner(crate_name, version, true).await
    }

    async fn unyank_crate(&self, crate_name: &str, version: &Version) -> IndexResult<()> {
        self.yank_inner(crate_name, version, false).await
    }

    async fn search(&self, query_string: &str, limit: usize) -> IndexResult<SearchResults> {
        let client = self.pool.get().await.unwrap();

        let statement = client
            .prepare_cached(include_str!("../sql/search.sql"))
            .await
            .context("Failed to prepare search statement")?;

        let mut rows: Vec<Row> = client
            .query(&statement, &[&query_string])
            .await
            .context("Failed to execute search query")?;

        // return the client immediately to the pool in case sorting takes longer than we'd like
        drop(client);

        // we can't scale the DB as easily as we can this server, so let's sort in here
        // warning: may be expensive!
        rows.sort_unstable_by_key(|r| (r.get::<_, i64>("count"), r.get::<_, String>("name")));

        let total = rows.len();

        // also might be expensive
        let crates = rows
            .into_iter()
            .take(limit as usize)
            .map(|row| {
                let versions: Vec<String> = row.get("versions");

                // we should never receive 0 versions from our query
                let max_version = versions
                    .iter()
                    .map(|s| Version::parse(&s).unwrap())
                    .max()
                    .unwrap();

                SearchResultsEntry {
                    name: row.get("name"),
                    max_version,
                    description: String::new(),
                }
            })
            .collect();

        let meta = SearchResultsMeta { total };

        Ok(SearchResults { crates, meta })
    }

    async fn publish(
        &self,
        version: &Publish,
        checksum: &str,
        end_step: Pin<Box<dyn Future<Output = anyhow::Result<()>> + Send>>,
    ) -> IndexResult<CompletedPublication> {
        let mut client = self
            .pool
            .get()
            .await
            .context("Failed to get client from pool")?;

        let transaction = client
            .build_transaction()
            .isolation_level(IsolationLevel::ReadCommitted)
            .start()
            .await
            .context("Failed to create publication transaction")?;

        let (
            get_or_insert_crate_statement,
            insert_version_statement,
            insert_dependency_statement,
            insert_features_statement,
        ) = tokio::try_join!(
            transaction.prepare_cached(include_str!("../sql/publish/insert-crate.sql")),
            transaction.prepare_cached(include_str!("../sql/publish/insert-version.sql")),
            transaction.prepare_cached(include_str!("../sql/publish/insert-dependency.sql")),
            transaction.prepare_cached(include_str!("../sql/publish/insert-features.sql")),
        )
        .context("Failed to prepare statements for publish transaction")?;

        let crate_id_row = transaction
            .query_one(&get_or_insert_crate_statement, &[&version.name])
            .await
            .context("Crate get or insert failed")?;

        let crate_id: i32 = crate_id_row.get("id");

        let insert_version_row = transaction
            .query_one(
                &insert_version_statement,
                &[
                    &crate_id,
                    &version.vers.to_string(),
                    &checksum,
                    &false,
                    &version.links,
                ],
            )
            .await
            .context("Failed to insert version")?;

        let version_id: i32 = insert_version_row.get("id");

        for dependency in version.deps.iter() {
            transaction
                .query_one(
                    &insert_dependency_statement,
                    &[
                        &dependency.name,
                        &dependency.registry,
                        &version_id,
                        &dependency.version_req.to_string(),
                        &dependency.features,
                        &dependency.optional,
                        &dependency.default_features,
                        &dependency.target,
                        &dependency.kind,
                        &dependency.explicit_name_in_toml,
                    ],
                )
                .await
                .context("Failed to insert dependency")?;
        }

        for feature in version.features.iter() {
            transaction
                .query_one(
                    &insert_features_statement,
                    &[&version_id, &feature.0, &feature.1],
                )
                .await
                .context("Failed to insert feature")?;
        }

        end_step
            .await
            .context("Failed to execute end step in index upload transaction")?;

        transaction
            .commit()
            .await
            .context("Failed to commit transaction")?;

        Ok(CompletedPublication { warnings: None })
    }

    async fn list(&self, _pagination: Option<&Pagination>) -> IndexResult<Vec<SearchResultsEntry>> {
        todo!()
    }
}