use sea_orm::{ConnectOptions, Database, DatabaseConnection, DbErr};
use std::time::Duration;
use migration::{Migrator, MigratorTrait};

pub struct DbState {
    pub conn: DatabaseConnection,
}

pub async fn init_db() -> Result<DatabaseConnection, DbErr> {
    let mut options = ConnectOptions::new("sqlite://share_clip.db?mode=rwc".to_string());
    options
        .max_connections(10)
        .min_connections(1)
        .connect_timeout(Duration::from_secs(8));

    let db = Database::connect(options).await?;

    Migrator::up(&db, None).await?;

    Ok(db)
}
