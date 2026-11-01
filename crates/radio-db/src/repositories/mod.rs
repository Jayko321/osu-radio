pub mod audio_mods;
pub use audio_mods::AudioModsRepository;
pub mod audio_source;
pub mod audio_volume;
pub use audio_volume::AudioVolumeRepository;
pub mod beatmap;
pub mod beatmap_metadata;
pub mod beatmap_set;
pub mod osu_installation;
pub mod user_data;

pub use audio_source::AudioSourceRepository;
pub use beatmap::BeatmapRepository;
pub use beatmap_metadata::{BeatmapMetadataRepository, metadata_hash};
pub use beatmap_set::BeatmapSetRepository;
pub use osu_installation::{OsuInstallationRepository, RegisteredInstallation};
pub use user_data::UserDataRepository;
pub mod tag;
pub use tag::TagRepository;

pub mod queue;
pub use queue::QueueRepository;
pub mod playlist;
pub use playlist::PlaylistRepository;
pub mod listening_history;
pub use listening_history::ListeningHistoryRepository;

/// Decode bulk reads as rows arrive, without retaining a second full `SQLx` result buffer.
async fn query_all<T: sea_orm::FromQueryResult + Send>(
    connection: &sea_orm::DatabaseExecutor<'_>,
    statement: sea_orm::Statement,
) -> anyhow::Result<Vec<T>> {
    let mut rows = Vec::new();
    query_each(connection, statement, |row| {
        rows.push(row);
        Ok(())
    })
    .await?;
    Ok(rows)
}

async fn query_each<T: sea_orm::FromQueryResult + Send>(
    connection: &sea_orm::DatabaseExecutor<'_>,
    statement: sea_orm::Statement,
    mut consume: impl FnMut(T) -> anyhow::Result<()> + Send,
) -> anyhow::Result<()> {
    let query = T::find_by_statement(statement);
    let mut stream = match connection {
        sea_orm::DatabaseExecutor::Connection(connection) => query.stream(*connection).await?,
        sea_orm::DatabaseExecutor::Transaction(transaction) => query.stream(*transaction).await?,
        sea_orm::DatabaseExecutor::OwnedTransaction(transaction) => {
            query.stream(transaction).await?
        }
    };
    while let Some(row) =
        tokio::task::coop::cooperative(std::future::poll_fn(|cx| stream.as_mut().poll_next(cx)))
            .await
    {
        consume(row?)?;
    }
    Ok(())
}

#[cfg(all(test, feature = "sqlite"))]
mod tests {
    use super::{query_all, query_each};
    use sea_orm::{DatabaseExecutor, DbBackend, FromQueryResult, Statement, TransactionTrait};

    #[derive(Debug, PartialEq, Eq, FromQueryResult)]
    struct Row {
        value: i32,
    }

    #[tokio::test]
    async fn bulk_reads_preserve_order_and_release_connections_after_errors_and_cancellation() {
        tokio::time::timeout(std::time::Duration::from_secs(5), async {
            // One connection makes a leaked stream or transaction lock observable as a timeout.
            let connection = crate::connection::connect(":memory:").await.unwrap();
            check_reads(DatabaseExecutor::Connection(&connection)).await;
            let transaction = connection.begin().await.unwrap();
            check_reads(DatabaseExecutor::Transaction(&transaction)).await;
            transaction.commit().await.unwrap();
            check_reads(DatabaseExecutor::OwnedTransaction(
                connection.begin().await.unwrap(),
            ))
            .await;
            check_reads(DatabaseExecutor::Connection(&connection)).await;
        })
        .await
        .unwrap();
    }

    async fn check_reads(connection: DatabaseExecutor<'_>) {
        let statement = |sql: &str| Statement::from_string(DbBackend::Sqlite, sql);
        assert!(
            query_each::<Row>(
                &connection,
                statement("SELECT 1 AS value"),
                |_| anyhow::bail!("rejected row"),
            )
            .await
            .is_err()
        );
        assert!(
            tokio::time::timeout(
                std::time::Duration::from_millis(20),
                query_each::<Row>(
                    &connection,
                    statement(
                        "WITH RECURSIVE numbers(value) AS \
                         (SELECT 1 UNION ALL SELECT value + 1 FROM numbers WHERE value < 1024) \
                         SELECT value FROM numbers",
                    ),
                    |_| {
                        std::thread::sleep(std::time::Duration::from_millis(1));
                        Ok(())
                    },
                ),
            )
            .await
            .is_err()
        );
        assert!(
            query_all::<Row>(
                &connection,
                statement("SELECT 1 AS value UNION ALL SELECT NULL AS value"),
            )
            .await
            .is_err()
        );
        assert_eq!(
            query_all::<Row>(
                &connection,
                statement("SELECT 3 AS value UNION ALL SELECT 1 AS value"),
            )
            .await
            .unwrap(),
            [Row { value: 3 }, Row { value: 1 }]
        );
        assert!(
            query_all::<Row>(&connection, statement("SELECT 1 AS value WHERE 0"))
                .await
                .unwrap()
                .is_empty()
        );
    }
}
