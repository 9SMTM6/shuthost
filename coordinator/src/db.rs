//! Database operations for persisting application state.
//!
//! This module handles SQLite database operations for persisting leases and other state.

use std::path::Path;

use sqlx::{Sqlite, SqlitePool, migrate::MigrateDatabase};

use crate::routes::{LeaseMap, LeaseSource};

/// Database connection pool type alias.
pub type DbPool = SqlitePool;

/// Creates or opens the SQLite database and runs migrations.
///
/// # Arguments
///
/// * `db_path` - Path to the SQLite database file.
///
/// # Returns
///
/// A database connection pool.
///
/// # Errors
///
/// Returns an error if the database cannot be created or migrated.
pub async fn init_db(db_path: &Path) -> eyre::Result<DbPool> {
    let db_url = format!("sqlite:{}", db_path.display());

    // Create database if it doesn't exist
    if !Sqlite::database_exists(&db_url).await? {
        Sqlite::create_database(&db_url).await?;
    }

    let pool = SqlitePool::connect(&db_url).await?;

    // Run migrations
    sqlx::migrate!("./migrations").run(&pool).await?;

    Ok(pool)
}

/// Loads all leases from the database into the in-memory map.
///
/// # Arguments
///
/// * `pool` - Database connection pool.
/// * `leases` - The in-memory lease map to populate.
///
/// # Errors
///
/// Returns an error if the database query fails.
pub async fn load_leases(pool: &DbPool, leases: &LeaseMap) -> eyre::Result<()> {
    let mut leases_guard = leases.lock().await;

    // Clear existing leases
    leases_guard.clear();

    // Load all lease records
    let lease_records = sqlx::query!("SELECT hostname, lease_source_type, lease_source_value FROM leases")
        .fetch_all(pool)
        .await?;

    for row in lease_records {
        let hostname: String = row.hostname;
        let lease_source_type: String = row.lease_source_type;
        let lease_source_value: Option<String> = row.lease_source_value;

        let lease_source = match lease_source_type.as_str() {
            "web_interface" => LeaseSource::WebInterface,
            "client" => LeaseSource::Client(lease_source_value.unwrap_or_default()),
            _ => continue, // Skip invalid records
        };

        leases_guard
            .entry(hostname)
            .or_default()
            .insert(lease_source);
    }

    Ok(())
}

/// Persists a lease change to the database.
///
/// # Arguments
///
/// * `pool` - Database connection pool.
/// * `hostname` - The hostname for the lease.
/// * `lease_source` - The lease source being added or removed.
/// * `action` - "add" to add the lease, "remove" to remove it.
///
/// # Errors
///
/// Returns an error if the database operation fails.
pub async fn add_lease(
    pool: &DbPool,
    hostname: &str,
    lease_source: &LeaseSource,
) -> eyre::Result<()> {
    match lease_source {
        LeaseSource::WebInterface => {
            sqlx::query!("INSERT OR IGNORE INTO web_interface_leases (hostname) VALUES (?)", hostname)
                .execute(pool)
                .await?;
        }
        LeaseSource::Client(client_id) => {
            sqlx::query!("INSERT OR IGNORE INTO client_leases (hostname, client_id) VALUES (?, ?)", hostname, client_id)
                .execute(pool)
                .await?;
        }
    }
    Ok(())
}

pub async fn remove_lease(
    pool: &DbPool,
    hostname: &str,
    lease_source: &LeaseSource,
) -> eyre::Result<()> {
    match lease_source {
        LeaseSource::WebInterface => {
            sqlx::query!("DELETE FROM web_interface_leases WHERE hostname = ?", hostname)
                .execute(pool)
                .await?;
        }
        LeaseSource::Client(client_id) => {
            sqlx::query!("DELETE FROM client_leases WHERE hostname = ? AND client_id = ?", hostname, client_id)
                .execute(pool)
                .await?;
        }
    }
    Ok(())
}

/// Removes all leases for a specific client from the database.
///
/// # Arguments
///
/// * `pool` - Database connection pool.
/// * `client_id` - The client ID whose leases should be removed.
///
/// # Errors
///
/// Returns an error if the database operation fails.
pub async fn remove_client_leases(pool: &DbPool, client_id: &str) -> eyre::Result<()> {
    sqlx::query!("DELETE FROM client_leases WHERE client_id = ?", client_id)
        .execute(pool)
        .await?;

    Ok(())
}