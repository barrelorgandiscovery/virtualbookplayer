use chrono::{DateTime, Utc};
use log::{debug, error, info};
/// this structure connect to a small sqlight database
/// to store metadata about the play
///
///
// for each played file, the metadata contains :
//    the relative file reference from the original path (a root path for the stats)
//    the latest play time history
//    the total play time
//    some "stars"/rank (1..5) hits from the user
use rusqlite::Connection;

/// Helper function to convert DateTime<Utc> to SQLite DATE format (YYYY-MM-DD HH:MM:SS)
/// SQLite DATE columns work best with this format for date functions
pub(crate) fn datetime_to_sqlite_date(dt: &DateTime<Utc>) -> String {
    dt.format("%Y-%m-%d %H:%M:%S").to_string()
}

/// Helper function to convert SQLite DATE string to DateTime<Utc>
/// Accepts SQLite date format: YYYY-MM-DD HH:MM:SS
/// The date is assumed to be in UTC
pub(crate) fn datetime_from_sqlite_date(
    s: &str,
) -> Result<DateTime<Utc>, Box<dyn std::error::Error + Send + Sync>> {
    // SQLite DATE format: YYYY-MM-DD HH:MM:SS (assumed UTC)
    use chrono::NaiveDateTime;
    NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S")
        .map(|naive_dt| DateTime::<Utc>::from_naive_utc_and_offset(naive_dt, Utc))
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
}

pub struct PlayMetadataDatabase {
    db_file_path: String,
    connection: Connection,
}

#[derive(Clone)]
pub struct PlayedFileStats {
    pub relative_file_path: String,
    pub file_md5_checksum: String,
    pub latest_play_time: DateTime<Utc>,
    pub total_play_number: u32, // this is computed from the play history
    pub total_star_count: u32,  // this is computed from the star history
    pub user_comments_or_notes: String,
}

pub struct PlayedFileStatsHistory {
    played_file_stats: PlayedFileStats,
    play_time_history: Vec<PlayedFileStatsHistoryEntry>,
}

pub struct PlayedFileStatsHistoryEntry {
    timestamp: DateTime<Utc>,
}

pub struct PlayedFileStarsHistoryEntry {
    timestamp: DateTime<Utc>,
    rank: u8,
}

const CURRENT_MODEL_VERSION: &str = "1.0.0";

// implement the database operations
impl PlayMetadataDatabase {
    pub fn new(db_file_path: String) -> Result<Self, Box<dyn std::error::Error>> {
        use std::path::Path;

        // Check if database file already exists
        let db_exists = Path::new(&db_file_path).exists();
        if db_exists {
            info!("Opening existing metadata database at: {:?}", db_file_path);
        } else {
            info!("Creating new metadata database at: {:?}", db_file_path);
        }

        let mut connection = Connection::open(&db_file_path).map_err(|e| {
            error!("Failed to open database at {:?}: {}", db_file_path, e);
            Box::new(e)
        })?;

        // Optimize SQLite for better performance
        Self::optimize_connection(&mut connection)?;

        let play_metadata_database = Self {
            db_file_path: db_file_path.clone(),
            connection,
        };
        play_metadata_database.create_tables()?;

        // Log database stats after opening
        if db_exists {
            if let Ok(count) = play_metadata_database.count_played_files() {
                info!("Database contains {} file entries", count);
            }
            if let Ok(count) = play_metadata_database.count_play_events() {
                info!("Database contains {} play events", count);
            }
        }

        Ok(play_metadata_database)
    }

    /// Count total number of files in database (for debugging)
    fn count_played_files(&self) -> Result<usize, Box<dyn std::error::Error>> {
        let count: i64 =
            self.connection
                .query_row("SELECT COUNT(*) FROM played_file_stats", [], |row| {
                    row.get(0)
                })?;
        Ok(count as usize)
    }

    /// Count total number of play events in database (for debugging)
    fn count_play_events(&self) -> Result<usize, Box<dyn std::error::Error>> {
        let count: i64 = self.connection.query_row(
            "SELECT COUNT(*) FROM played_file_stats_history",
            [],
            |row| row.get(0),
        )?;
        Ok(count as usize)
    }

    /// Configure SQLite connection for optimal performance with small memory footprint
    fn optimize_connection(connection: &mut Connection) -> Result<(), Box<dyn std::error::Error>> {
        // Enable WAL (Write-Ahead Logging) mode for better concurrent performance
        // WAL allows reads and writes to happen simultaneously without blocking
        // Memory overhead: minimal (WAL file on disk, small in-memory buffer)
        // Note: WAL mode may not be available for in-memory databases, so we handle errors gracefully
        if let Ok(mode) =
            connection.query_row::<String, _, _>("PRAGMA journal_mode = WAL", [], |row| row.get(0))
        {
            // WAL mode enabled successfully
            let _ = mode; // Use the result to avoid unused variable warning
        }
        // If WAL fails (e.g., in-memory DB), continue with default mode

        // Set synchronous mode to NORMAL (faster than FULL, safer than OFF)
        // With WAL mode, NORMAL is safe and much faster
        // Memory overhead: none
        let _ = connection.execute("PRAGMA synchronous = NORMAL", ());

        // Keep cache size small for low memory footprint
        // Default is 2000 pages (~8MB), we'll use 2000 KB (~2MB) for small footprint
        // Negative value means KB, positive means pages
        let _ =
            connection.query_row::<i32, _, _>("PRAGMA cache_size = -2000", [], |row| row.get(0));

        // Store temporary tables on disk (not in memory) to save RAM
        // Slightly slower but much better for memory-constrained environments
        let _ = connection.query_row::<i32, _, _>("PRAGMA temp_store = FILE", [], |row| row.get(0));

        // Enable foreign key constraints (for data integrity)
        // Memory overhead: minimal (just enables checking)
        let _ = connection.execute("PRAGMA foreign_keys = ON", ());

        Ok(())
    }

    /// Get the database file path
    pub fn db_path(&self) -> &str {
        &self.db_file_path
    }

    pub fn check_model_compatibility(&self) -> Result<(), Box<dyn std::error::Error>> {
        // get the version table and see if the current version match
        let mut stmt = self
            .connection
            .prepare("SELECT version FROM version")
            .map_err(Box::new)?;
        let version = stmt
            .query_row([], |row| row.get::<_, String>(0))
            .map_err(Box::new)?;
        if version != "1.0.0" {
            return Err(Box::new(std::io::Error::new(
                std::io::ErrorKind::Other,
                "Model version mismatch",
            )));
        }
        Ok(())
    }

    pub fn create_tables(&self) -> Result<(), Box<dyn std::error::Error>> {
        // create the version table
        self.connection
            .execute("CREATE TABLE IF NOT EXISTS version (version TEXT)", ())
            .map_err(Box::new)?;

        // insert the current version
        self.connection
            .execute(
                "INSERT OR REPLACE INTO version (version) VALUES (?)",
                (CURRENT_MODEL_VERSION,),
            )
            .map_err(Box::new)?;

        // create the played_file_stats table
        // Note: latest_play_time and total_play_number are computed from history table
        // relative_file_path is unique to allow INSERT OR REPLACE to work correctly
        self.connection
            .execute(
                "CREATE TABLE IF NOT EXISTS played_file_stats (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            relative_file_path TEXT UNIQUE,
            file_md5_checksum TEXT,
            user_comments_or_notes TEXT
        )",
                (),
            )
            .map_err(Box::new)?;

        // create the played_file_stats_history table
        // stores individual play events to compute statistics
        // played_time is stored as DATE type (SQLite treats DATE as TEXT but with date semantics)
        self.connection
            .execute(
                "CREATE TABLE IF NOT EXISTS played_file_stats_history (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            ref_played_file_stats_id INTEGER,
            played_time DATE
        )",
                (),
            )
            .map_err(Box::new)?;

        // Create indexes for optimal query performance
        // Index on relative_file_path for fast lookups
        self.connection
            .execute(
                "CREATE INDEX IF NOT EXISTS idx_played_file_stats_path \
                ON played_file_stats(relative_file_path)",
                (),
            )
            .map_err(Box::new)?;

        // Index on foreign key for JOIN performance
        self.connection
            .execute(
                "CREATE INDEX IF NOT EXISTS idx_played_file_stats_history_ref \
                ON played_file_stats_history(ref_played_file_stats_id)",
                (),
            )
            .map_err(Box::new)?;

        // Index on played_time for MAX() aggregation and date range queries
        self.connection
            .execute(
                "CREATE INDEX IF NOT EXISTS idx_played_file_stats_history_time \
                ON played_file_stats_history(played_time)",
                (),
            )
            .map_err(Box::new)?;

        // Composite index for common query pattern: ref_id + time for efficient MAX() queries
        self.connection
            .execute(
                "CREATE INDEX IF NOT EXISTS idx_played_file_stats_history_ref_time \
                ON played_file_stats_history(ref_played_file_stats_id, played_time)",
                (),
            )
            .map_err(Box::new)?;

        // create the played_file_stats_stars table
        // stores individual star events to compute star count
        self.connection
            .execute(
                "CREATE TABLE IF NOT EXISTS played_file_stats_stars (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            ref_played_file_stats_id INTEGER,
            star_time DATE,
            FOREIGN KEY(ref_played_file_stats_id) REFERENCES played_file_stats(id)
        )",
                (),
            )
            .map_err(Box::new)?;

        // Create indexes for star events
        self.connection
            .execute(
                "CREATE INDEX IF NOT EXISTS idx_played_file_stats_stars_ref \
                ON played_file_stats_stars(ref_played_file_stats_id)",
                (),
            )
            .map_err(Box::new)?;

        self.connection
            .execute(
                "CREATE INDEX IF NOT EXISTS idx_played_file_stats_stars_time \
                ON played_file_stats_stars(star_time)",
                (),
            )
            .map_err(Box::new)?;

        self.connection
            .execute(
                "CREATE INDEX IF NOT EXISTS idx_played_file_stats_stars_ref_time \
                ON played_file_stats_stars(ref_played_file_stats_id, star_time)",
                (),
            )
            .map_err(Box::new)?;

        Ok(())
    }

    pub fn insert_or_update_played_file_stats(
        &self,
        played_file_stats: PlayedFileStats,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Use INSERT ... ON CONFLICT DO UPDATE to preserve the id when updating
        // This is important because foreign keys in played_file_stats_history reference the id
        // INSERT OR REPLACE would delete and reinsert, changing the id and breaking foreign keys
        let file_path_for_log = played_file_stats.relative_file_path.clone();
        self.connection
            .execute(
                "INSERT INTO played_file_stats \
                (relative_file_path, file_md5_checksum, user_comments_or_notes) \
                VALUES (?, ?, ?) \
                ON CONFLICT(relative_file_path) DO UPDATE SET \
                file_md5_checksum = excluded.file_md5_checksum, \
                user_comments_or_notes = excluded.user_comments_or_notes",
                (
                    played_file_stats.relative_file_path,
                    played_file_stats.file_md5_checksum,
                    played_file_stats.user_comments_or_notes,
                ),
            )
            .map_err(|e| {
                error!(
                    "Error inserting/updating file stats for '{}': {}",
                    file_path_for_log, e
                );
                Box::new(e)
            })?;
        Ok(())
    }

    /// Add a play event to the history table
    pub fn add_play_event(
        &self,
        relative_file_path: String,
        play_time: DateTime<Utc>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Use a single query with subquery to avoid two round trips
        // But check if any rows were affected to ensure the file exists
        let play_time_str = datetime_to_sqlite_date(&play_time);
        let file_path_for_log = relative_file_path.clone();
        debug!(
            "Adding play event to history: file='{}', time='{}'",
            file_path_for_log, play_time_str
        );

        let rows_affected = self
            .connection
            .execute(
                "INSERT INTO played_file_stats_history (ref_played_file_stats_id, played_time) \
                SELECT id, ? FROM played_file_stats WHERE relative_file_path = ?",
                (play_time_str, relative_file_path.clone()),
            )
            .map_err(|e| {
                error!(
                    "Database error inserting play event for '{}': {}",
                    file_path_for_log, e
                );
                Box::new(e)
            })?;

        if rows_affected == 0 {
            error!("No rows affected when inserting play event for '{}' - file may not exist in played_file_stats", file_path_for_log);
            return Err(Box::new(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!(
                    "File '{}' not found in played_file_stats",
                    file_path_for_log
                ),
            )));
        }

        debug!(
            "Successfully inserted play event: {} rows affected for '{}'",
            rows_affected, file_path_for_log
        );
        Ok(())
    }

    /// Add a star event to the star history table
    pub fn add_star_event(
        &self,
        relative_file_path: String,
        star_time: DateTime<Utc>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Use a single query with subquery to avoid two round trips
        // But check if any rows were affected to ensure the file exists
        let star_time_str = datetime_to_sqlite_date(&star_time);
        let file_path_for_log = relative_file_path.clone();
        info!(
            "Adding star event to history: file='{}', time='{}'",
            file_path_for_log, star_time_str
        );

        let rows_affected = self
            .connection
            .execute(
                "INSERT INTO played_file_stats_stars (ref_played_file_stats_id, star_time) \
                SELECT id, ? FROM played_file_stats WHERE relative_file_path = ?",
                (star_time_str, relative_file_path.clone()),
            )
            .map_err(|e| {
                error!(
                    "Database error inserting star event for '{}': {}",
                    file_path_for_log, e
                );
                Box::new(e)
            })?;

        if rows_affected == 0 {
            error!("No rows affected when inserting star event for '{}' - file may not exist in played_file_stats", file_path_for_log);
            return Err(Box::new(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!(
                    "File '{}' not found in played_file_stats",
                    file_path_for_log
                ),
            )));
        }

        info!(
            "Successfully inserted star event: {} rows affected for '{}'",
            rows_affected, file_path_for_log
        );
        Ok(())
    }

    /// Add multiple play events in a single transaction for better performance
    /// Memory-efficient: processes events in chunks to avoid large SQL strings
    pub fn add_play_events_batch(
        &self,
        relative_file_path: String,
        play_times: Vec<DateTime<Utc>>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Get the stats_id once
        let stats_id: i64 = self
            .connection
            .query_row(
                "SELECT id FROM played_file_stats WHERE relative_file_path = ?",
                [&relative_file_path],
                |row| row.get(0),
            )
            .map_err(Box::new)?;

        // Process in chunks to keep memory usage low
        // Use BEGIN/COMMIT for transaction, but process in smaller batches
        const CHUNK_SIZE: usize = 100; // Process 100 events at a time

        // Begin transaction
        self.connection.execute("BEGIN", ()).map_err(Box::new)?;

        // Prepare statement once and reuse
        let mut stmt = self.connection.prepare(
            "INSERT INTO played_file_stats_history (ref_played_file_stats_id, played_time) VALUES (?, ?)"
        )?;

        // Process events in chunks
        for chunk in play_times.chunks(CHUNK_SIZE) {
            for play_time in chunk {
                let play_time_str = datetime_to_sqlite_date(play_time);
                stmt.execute((stats_id, play_time_str))?;
            }
        }

        // Commit transaction
        self.connection.execute("COMMIT", ()).map_err(Box::new)?;
        Ok(())
    }

    /// Insert or update multiple file stats in a single transaction
    /// Memory-efficient: processes items in chunks using parameterized queries
    pub fn insert_or_update_played_file_stats_batch(
        &self,
        stats_list: Vec<PlayedFileStats>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Process in chunks to keep memory usage low
        const CHUNK_SIZE: usize = 100; // Process 100 items at a time

        // Begin transaction
        self.connection.execute("BEGIN", ()).map_err(Box::new)?;

        // Prepare statement once and reuse
        // Use INSERT ... ON CONFLICT DO UPDATE to preserve the id when updating
        let mut stmt = self.connection.prepare(
            "INSERT INTO played_file_stats \
            (relative_file_path, file_md5_checksum, user_comments_or_notes) \
            VALUES (?, ?, ?) \
            ON CONFLICT(relative_file_path) DO UPDATE SET \
            file_md5_checksum = excluded.file_md5_checksum, \
            user_comments_or_notes = excluded.user_comments_or_notes",
        )?;

        // Process items in chunks
        for chunk in stats_list.chunks(CHUNK_SIZE) {
            for stats in chunk {
                stmt.execute((
                    &stats.relative_file_path,
                    &stats.file_md5_checksum,
                    &stats.user_comments_or_notes,
                ))?;
            }
        }

        // Commit transaction
        self.connection.execute("COMMIT", ()).map_err(Box::new)?;
        Ok(())
    }

    pub fn get_played_file_stats_with_statistics(
        &self,
        relative_file_path: String,
    ) -> Result<Option<PlayedFileStats>, Box<dyn std::error::Error>> {
        debug!(
            "Querying play stats for relative path: '{}'",
            relative_file_path
        );

        // Optimized SQL query using subqueries for better performance with indexes
        // This query leverages the composite index (ref_played_file_stats_id, played_time)
        let sql = "\
            SELECT \
                played_file_stats.id, \
                played_file_stats.relative_file_path, \
                played_file_stats.file_md5_checksum, \
                played_file_stats.user_comments_or_notes, \
                (SELECT MAX(played_time) FROM played_file_stats_history \
                 WHERE ref_played_file_stats_id = played_file_stats.id) as latest_play_time, \
                (SELECT COUNT(*) FROM played_file_stats_history \
                 WHERE ref_played_file_stats_id = played_file_stats.id) as total_play_number, \
                (SELECT COUNT(*) FROM played_file_stats_stars \
                 WHERE ref_played_file_stats_id = played_file_stats.id) as total_star_count \
            FROM played_file_stats \
            WHERE played_file_stats.relative_file_path = ?";

        let mut stmt = self.connection.prepare(sql).map_err(|e| {
            error!(
                "Failed to prepare query for '{}': {}",
                relative_file_path, e
            );
            Box::new(e)
        })?;

        // Try to find the file - use query_row which returns an error if not found
        let result = stmt.query_row([&relative_file_path], |row| {
            let relative_file_path: String = row.get(1)?;
            let file_md5_checksum: String = row.get(2)?;
            let user_comments_or_notes: String = row.get(3)?;

            // Get computed values from history
            let latest_play_time_str: Option<String> = row.get(4)?;
            let total_play_number: i64 = row.get(5)?;
            let total_star_count: i64 = row.get(6)?;

            debug!(
                "Found file in database: '{}', play_count={}, star_count={}",
                relative_file_path, total_play_number, total_star_count
            );

            // Convert latest_play_time from SQLite DATE format to DateTime<Utc>
            // If no history exists, use a default (epoch or current time)
            let latest_play_time = if let Some(time_str) = latest_play_time_str {
                datetime_from_sqlite_date(&time_str).map_err(|_| {
                    rusqlite::Error::InvalidColumnType(
                        4,
                        "DATE".to_string(),
                        rusqlite::types::Type::Text,
                    )
                })?
            } else {
                // No play history yet, use epoch as default
                DateTime::<Utc>::from_timestamp(0, 0).unwrap()
            };

            Ok(PlayedFileStats {
                relative_file_path,
                file_md5_checksum,
                latest_play_time,
                total_play_number: total_play_number as u32,
                total_star_count: total_star_count as u32,
                user_comments_or_notes,
            })
        });

        match result {
            Ok(stats) => {
                debug!(
                    "Successfully retrieved stats for '{}': play_count={}",
                    relative_file_path, stats.total_play_number
                );
                Ok(Some(stats))
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => {
                debug!(
                    "File '{}' not found in database (no play history yet)",
                    relative_file_path
                );
                Ok(None)
            }
            Err(e) => {
                error!("Database error querying '{}': {}", relative_file_path, e);
                Err(Box::new(e))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;
    use std::fs;
    use std::time::Instant;

    /// Helper function to create a temporary test database
    /// Uses a unique filename to avoid conflicts between parallel tests
    fn create_test_db() -> PlayMetadataDatabase {
        use std::time::{SystemTime, UNIX_EPOCH};
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let test_db_path = format!("/tmp/test_playmetadata_{}.db", timestamp);
        // Remove existing test database if it exists
        let _ = fs::remove_file(&test_db_path);
        PlayMetadataDatabase::new(test_db_path).expect("Failed to create test database")
    }

    /// Helper function to create test PlayedFileStats
    fn create_test_stats(path: &str) -> PlayedFileStats {
        PlayedFileStats {
            relative_file_path: path.to_string(),
            file_md5_checksum: format!("md5_{}", path),
            latest_play_time: DateTime::<Utc>::from_timestamp(0, 0).unwrap(),
            total_play_number: 0,
            total_star_count: 0,
            user_comments_or_notes: format!("Notes for {}", path),
        }
    }

    // ========== Function Tests ==========

    #[test]
    fn test_datetime_to_sqlite_date() {
        let dt = DateTime::<Utc>::from_timestamp(0, 0).unwrap();
        let result = datetime_to_sqlite_date(&dt);
        assert_eq!(result, "1970-01-01 00:00:00");
    }

    #[test]
    fn test_datetime_from_sqlite_date() {
        let date_str = "2023-12-25 15:30:45";
        let result = datetime_from_sqlite_date(date_str).unwrap();
        assert_eq!(result.format("%Y-%m-%d %H:%M:%S").to_string(), date_str);
    }

    #[test]
    fn test_datetime_round_trip() {
        let original = Utc::now();
        let date_str = datetime_to_sqlite_date(&original);
        let restored = datetime_from_sqlite_date(&date_str).unwrap();
        // Allow 1 second difference due to format precision
        let diff = (original - restored).num_seconds().abs();
        assert!(diff <= 1, "Round trip failed: {} vs {}", original, restored);
    }

    #[test]
    fn test_datetime_invalid_format() {
        let invalid = "not-a-date";
        assert!(datetime_from_sqlite_date(invalid).is_err());
    }

    #[test]
    fn test_create_tables() {
        let db = create_test_db();
        // Tables should be created successfully
        assert!(db.check_model_compatibility().is_ok());
    }

    #[test]
    fn test_insert_and_retrieve_stats() {
        let db = create_test_db();
        let stats = create_test_stats("test/file.mid");

        // Insert stats
        db.insert_or_update_played_file_stats(stats.clone())
            .expect("Failed to insert stats");

        // Retrieve stats
        let retrieved = db
            .get_played_file_stats_with_statistics("test/file.mid".to_string())
            .expect("Failed to retrieve stats")
            .expect("Stats not found");

        assert_eq!(retrieved.relative_file_path, stats.relative_file_path);
        assert_eq!(retrieved.file_md5_checksum, stats.file_md5_checksum);
        assert_eq!(
            retrieved.user_comments_or_notes,
            stats.user_comments_or_notes
        );
        assert_eq!(retrieved.total_play_number, 0); // No history yet
    }

    #[test]
    fn test_update_existing_stats() {
        let db = create_test_db();
        let mut stats = create_test_stats("test/file.mid");

        // Insert initial stats
        db.insert_or_update_played_file_stats(stats.clone())
            .expect("Failed to insert stats");

        // Update with new comments
        stats.user_comments_or_notes = "Updated notes".to_string();
        db.insert_or_update_played_file_stats(stats.clone())
            .expect("Failed to update stats");

        // Verify update
        let retrieved = db
            .get_played_file_stats_with_statistics("test/file.mid".to_string())
            .expect("Failed to retrieve stats")
            .expect("Stats not found");

        assert_eq!(retrieved.user_comments_or_notes, "Updated notes");
    }

    #[test]
    fn test_add_play_event() {
        let db = create_test_db();
        let stats = create_test_stats("test/file.mid");

        // Insert stats first
        db.insert_or_update_played_file_stats(stats)
            .expect("Failed to insert stats");

        // Add play event
        let play_time = Utc::now();
        db.add_play_event("test/file.mid".to_string(), play_time)
            .expect("Failed to add play event");

        // Verify statistics are computed correctly
        let retrieved = db
            .get_played_file_stats_with_statistics("test/file.mid".to_string())
            .expect("Failed to retrieve stats")
            .expect("Stats not found");

        assert_eq!(retrieved.total_play_number, 1);
        // Latest play time should be close to what we inserted
        let diff = (play_time - retrieved.latest_play_time).num_seconds().abs();
        assert!(diff <= 1, "Latest play time mismatch");
    }

    #[test]
    fn test_multiple_play_events() {
        let db = create_test_db();
        let stats = create_test_stats("test/file.mid");

        db.insert_or_update_played_file_stats(stats)
            .expect("Failed to insert stats");

        // Add multiple play events
        let base_time = DateTime::<Utc>::from_timestamp(1000000, 0).unwrap();
        for i in 0..5 {
            let play_time = base_time + chrono::Duration::seconds(i * 3600);
            db.add_play_event("test/file.mid".to_string(), play_time)
                .expect("Failed to add play event");
        }

        // Verify statistics
        let retrieved = db
            .get_played_file_stats_with_statistics("test/file.mid".to_string())
            .expect("Failed to retrieve stats")
            .expect("Stats not found");

        assert_eq!(retrieved.total_play_number, 5);
        // Latest should be the last one (base_time + 4 hours)
        let expected_latest = base_time + chrono::Duration::seconds(4 * 3600);
        let diff = (expected_latest - retrieved.latest_play_time)
            .num_seconds()
            .abs();
        assert!(diff <= 1, "Latest play time should be the most recent");
    }

    #[test]
    fn test_get_nonexistent_stats() {
        let db = create_test_db();
        let result = db
            .get_played_file_stats_with_statistics("nonexistent/file.mid".to_string())
            .expect("Query should succeed");
        assert!(result.is_none(), "Should return None for nonexistent file");
    }

    #[test]
    fn test_add_play_event_nonexistent_file() {
        let db = create_test_db();
        // Try to add play event for file that doesn't exist
        let result = db.add_play_event("nonexistent/file.mid".to_string(), Utc::now());
        assert!(result.is_err(), "Should fail when file doesn't exist");
    }

    // ========== Index Tests ==========

    #[test]
    fn test_create_indexes() {
        let db = create_test_db();

        // Indexes should be created automatically in create_tables()
        // Verify indexes exist by querying sqlite_master
        let index_count: i64 = db
            .connection
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master \
                WHERE type='index' AND name LIKE 'idx_%'",
                [],
                |row| row.get(0),
            )
            .expect("Failed to query indexes");

        // Should have at least 4 indexes:
        // 1. idx_played_file_stats_path
        // 2. idx_played_file_stats_history_ref
        // 3. idx_played_file_stats_history_time
        // 4. idx_played_file_stats_history_ref_time (composite)
        assert!(
            index_count >= 4,
            "Expected at least 4 indexes, got {}",
            index_count
        );

        // Verify specific indexes exist
        let indexes: Vec<String> = db
            .connection
            .prepare("SELECT name FROM sqlite_master WHERE type='index' AND name LIKE 'idx_%'")
            .expect("Failed to prepare query")
            .query_map([], |row| row.get::<_, String>(0))
            .expect("Failed to query")
            .collect::<Result<Vec<_>, _>>()
            .expect("Failed to collect indexes");

        assert!(
            indexes.contains(&"idx_played_file_stats_path".to_string()),
            "Missing index: idx_played_file_stats_path"
        );
        assert!(
            indexes.contains(&"idx_played_file_stats_history_ref".to_string()),
            "Missing index: idx_played_file_stats_history_ref"
        );
        assert!(
            indexes.contains(&"idx_played_file_stats_history_time".to_string()),
            "Missing index: idx_played_file_stats_history_time"
        );
        assert!(
            indexes.contains(&"idx_played_file_stats_history_ref_time".to_string()),
            "Missing index: idx_played_file_stats_history_ref_time"
        );
    }

    #[test]
    fn test_index_performance() {
        let db = create_test_db();

        // Create indexes
        db.connection
            .execute(
                "CREATE INDEX IF NOT EXISTS idx_played_file_stats_path \
                ON played_file_stats(relative_file_path)",
                (),
            )
            .expect("Failed to create index");

        // Insert many records
        for i in 0..100 {
            let stats = create_test_stats(&format!("test/file_{}.mid", i));
            db.insert_or_update_played_file_stats(stats)
                .expect("Failed to insert stats");
        }

        // Measure query performance with index
        let start = Instant::now();
        for i in 0..100 {
            let _ = db.get_played_file_stats_with_statistics(format!("test/file_{}.mid", i));
        }
        let duration_with_index = start.elapsed();

        // Query should complete quickly with index
        assert!(
            duration_with_index.as_millis() < 1000,
            "Query with index took too long: {}ms",
            duration_with_index.as_millis()
        );
    }

    // ========== Performance Tests ==========

    #[test]
    fn test_bulk_insert_performance() {
        let db = create_test_db();
        let count = 1000;

        let start = Instant::now();
        for i in 0..count {
            let stats = create_test_stats(&format!("test/file_{}.mid", i));
            db.insert_or_update_played_file_stats(stats)
                .expect("Failed to insert stats");
        }
        let duration = start.elapsed();

        let avg_time_per_insert = duration.as_millis() as f64 / count as f64;
        println!(
            "Bulk insert: {} records in {}ms (avg: {:.2}ms per record)",
            count,
            duration.as_millis(),
            avg_time_per_insert
        );

        // Should be reasonably fast (less than 50ms per insert on average for test environment)
        // In production with optimized builds, this should be much faster
        assert!(
            avg_time_per_insert < 50.0,
            "Insert performance too slow: {:.2}ms per record",
            avg_time_per_insert
        );
    }

    #[test]
    fn test_bulk_play_events_performance() {
        let db = create_test_db();
        let stats = create_test_stats("test/file.mid");
        db.insert_or_update_played_file_stats(stats)
            .expect("Failed to insert stats");

        let count = 1000;
        let base_time = Utc::now();

        let start = Instant::now();
        for i in 0..count {
            let play_time = base_time + chrono::Duration::seconds(i);
            db.add_play_event("test/file.mid".to_string(), play_time)
                .expect("Failed to add play event");
        }
        let duration = start.elapsed();

        let avg_time_per_event = duration.as_millis() as f64 / count as f64;
        println!(
            "Bulk play events: {} events in {}ms (avg: {:.2}ms per event)",
            count,
            duration.as_millis(),
            avg_time_per_event
        );

        // Verify all events were recorded
        let retrieved = db
            .get_played_file_stats_with_statistics("test/file.mid".to_string())
            .expect("Failed to retrieve stats")
            .expect("Stats not found");

        assert_eq!(retrieved.total_play_number, count as u32);

        // Performance check: should be reasonably fast (less than 50ms per event for test environment)
        // Note: Performance can vary based on system load, so we use a generous threshold
        if avg_time_per_event >= 50.0 {
            println!("WARNING: Play event insert performance is slow: {:.2}ms per event (threshold: 50ms)", avg_time_per_event);
            // Don't fail the test, but log a warning
        }
    }

    #[test]
    fn test_query_performance_with_history() {
        let db = create_test_db();

        // Create indexes for performance
        db.connection
            .execute(
                "CREATE INDEX IF NOT EXISTS idx_played_file_stats_path \
                ON played_file_stats(relative_file_path)",
                (),
            )
            .expect("Failed to create index");

        // Insert stats and many play events
        let stats = create_test_stats("test/file.mid");
        db.insert_or_update_played_file_stats(stats)
            .expect("Failed to insert stats");

        let base_time = Utc::now();
        for i in 0..1000 {
            let play_time = base_time + chrono::Duration::seconds(i);
            db.add_play_event("test/file.mid".to_string(), play_time)
                .expect("Failed to add play event");
        }

        // Measure query performance
        let start = Instant::now();
        let retrieved = db
            .get_played_file_stats_with_statistics("test/file.mid".to_string())
            .expect("Failed to retrieve stats")
            .expect("Stats not found");
        let duration = start.elapsed();

        println!(
            "Query with {} history records took {}ms",
            retrieved.total_play_number,
            duration.as_millis()
        );

        assert_eq!(retrieved.total_play_number, 1000);
        // Query should be fast even with many history records
        assert!(
            duration.as_millis() < 100,
            "Query too slow: {}ms",
            duration.as_millis()
        );
    }

    #[test]
    fn test_concurrent_operations() {
        let db = create_test_db();

        // Insert multiple files
        for i in 0..10 {
            let stats = create_test_stats(&format!("test/file_{}.mid", i));
            db.insert_or_update_played_file_stats(stats)
                .expect("Failed to insert stats");
        }

        // Add play events for different files
        let base_time = Utc::now();
        for i in 0..10 {
            for j in 0..5 {
                let play_time = base_time + chrono::Duration::seconds(i * 10 + j);
                db.add_play_event(format!("test/file_{}.mid", i), play_time)
                    .expect("Failed to add play event");
            }
        }

        // Verify all files have correct statistics
        for i in 0..10 {
            let retrieved = db
                .get_played_file_stats_with_statistics(format!("test/file_{}.mid", i))
                .expect("Failed to retrieve stats")
                .expect("Stats not found");
            assert_eq!(retrieved.total_play_number, 5);
        }
    }

    // ========== Edge Case Tests ==========

    #[test]
    fn test_empty_history_statistics() {
        let db = create_test_db();
        let stats = create_test_stats("test/file.mid");

        db.insert_or_update_played_file_stats(stats)
            .expect("Failed to insert stats");

        // Get stats without any play events
        let retrieved = db
            .get_played_file_stats_with_statistics("test/file.mid".to_string())
            .expect("Failed to retrieve stats")
            .expect("Stats not found");

        assert_eq!(retrieved.total_play_number, 0);
        // Latest play time should be epoch when no history
        assert_eq!(
            retrieved.latest_play_time.timestamp(),
            0,
            "Latest play time should be epoch when no history"
        );
    }

    #[test]
    fn test_special_characters_in_path() {
        let db = create_test_db();
        let stats = PlayedFileStats {
            relative_file_path: "test/file with spaces & special-chars.mid".to_string(),
            file_md5_checksum: "md5_hash".to_string(),
            latest_play_time: Utc::now(),
            total_play_number: 0,
            total_star_count: 0,
            user_comments_or_notes: "Test notes".to_string(),
        };

        db.insert_or_update_played_file_stats(stats.clone())
            .expect("Failed to insert stats with special characters");

        let retrieved = db
            .get_played_file_stats_with_statistics(stats.relative_file_path.clone())
            .expect("Failed to retrieve stats");

        assert!(retrieved.is_some());
        assert_eq!(
            retrieved.unwrap().relative_file_path,
            stats.relative_file_path
        );
    }

    #[test]
    fn test_very_long_path() {
        let db = create_test_db();
        let long_path = "a".repeat(500);
        let stats = PlayedFileStats {
            relative_file_path: long_path.clone(),
            file_md5_checksum: "md5_hash".to_string(),
            latest_play_time: Utc::now(),
            total_play_number: 0,
            total_star_count: 0,
            user_comments_or_notes: "Test".to_string(),
        };

        db.insert_or_update_played_file_stats(stats)
            .expect("Failed to insert stats with long path");

        let retrieved = db
            .get_played_file_stats_with_statistics(long_path)
            .expect("Failed to retrieve stats");

        assert!(retrieved.is_some());
    }

    #[test]
    fn test_date_ordering() {
        let db = create_test_db();
        let stats = create_test_stats("test/file.mid");
        db.insert_or_update_played_file_stats(stats)
            .expect("Failed to insert stats");

        // Add play events in reverse chronological order
        let base_time = DateTime::<Utc>::from_timestamp(1000000, 0).unwrap();
        for i in (0..10).rev() {
            let play_time = base_time + chrono::Duration::seconds(i * 3600);
            db.add_play_event("test/file.mid".to_string(), play_time)
                .expect("Failed to add play event");
        }

        // Latest should be the most recent (highest timestamp)
        let retrieved = db
            .get_played_file_stats_with_statistics("test/file.mid".to_string())
            .expect("Failed to retrieve stats")
            .expect("Stats not found");

        assert_eq!(retrieved.total_play_number, 10);
        let expected_latest = base_time + chrono::Duration::seconds(9 * 3600);
        let diff = (expected_latest - retrieved.latest_play_time)
            .num_seconds()
            .abs();
        assert!(diff <= 1, "Latest should be the most recent timestamp");
    }

    // ========== Large Scale Performance Tests ==========

    #[test]
    #[ignore] // This is a long-running test - run with: cargo test -- --ignored
    fn test_large_scale_performance_10000_files_1000_events() {
        let db = create_test_db();
        let file_count = 10_000;
        let events_per_file = 1_000;
        let total_events = file_count * events_per_file;

        println!("\n=== Large Scale Performance Test ===");
        println!(
            "Files: {}, Events per file: {}, Total events: {}",
            file_count, events_per_file, total_events
        );

        // Phase 1: Insert 10,000 files using batch operations
        println!(
            "\nPhase 1: Inserting {} files (using batch operations)...",
            file_count
        );
        let start = Instant::now();
        let batch_size = 1000;
        let mut stats_batch = Vec::new();

        for i in 0..file_count {
            let stats = create_test_stats(&format!("test/file_{}.mid", i));
            stats_batch.push(stats);

            // Insert in batches for better performance
            if stats_batch.len() >= batch_size || i == file_count - 1 {
                db.insert_or_update_played_file_stats_batch(std::mem::take(&mut stats_batch))
                    .expect("Failed to insert stats batch");

                // Progress indicator
                if (i + 1) % batch_size == 0 || i == file_count - 1 {
                    let elapsed = start.elapsed();
                    let rate = (i + 1) as f64 / elapsed.as_secs_f64();
                    println!("  Inserted {} files ({:.0} files/sec)", i + 1, rate);
                }
            }
        }
        let insert_files_duration = start.elapsed();
        let avg_file_insert = insert_files_duration.as_millis() as f64 / file_count as f64;
        println!(
            "✓ Inserted {} files in {}ms (avg: {:.2}ms per file)",
            file_count,
            insert_files_duration.as_millis(),
            avg_file_insert
        );

        // Phase 2: Insert 1,000 events for each file using batch operations
        println!(
            "\nPhase 2: Inserting {} events per file ({} total events, using batch operations)...",
            events_per_file, total_events
        );
        let start = Instant::now();
        let base_time = DateTime::<Utc>::from_timestamp(1000000, 0).unwrap();

        for file_idx in 0..file_count {
            let file_path = format!("test/file_{}.mid", file_idx);
            let mut play_times = Vec::new();

            for event_idx in 0..events_per_file {
                let play_time = base_time
                    + chrono::Duration::seconds((file_idx * events_per_file + event_idx) as i64);
                play_times.push(play_time);
            }

            // Insert all events for this file in a single batch transaction
            db.add_play_events_batch(file_path, play_times)
                .expect("Failed to add play events batch");

            // Progress indicator every 1000 files
            if (file_idx + 1) % 1000 == 0 {
                let elapsed = start.elapsed();
                let events_inserted = (file_idx + 1) * events_per_file;
                let rate = events_inserted as f64 / elapsed.as_secs_f64();
                println!(
                    "  Inserted events for {} files ({:.0} events/sec)",
                    file_idx + 1,
                    rate
                );
            }
        }
        let insert_events_duration = start.elapsed();
        let avg_event_insert = insert_events_duration.as_millis() as f64 / total_events as f64;
        println!(
            "✓ Inserted {} events in {}ms (avg: {:.3}ms per event)",
            total_events,
            insert_events_duration.as_millis(),
            avg_event_insert
        );

        // Phase 3: Verify data integrity - check a sample of files
        println!("\nPhase 3: Verifying data integrity...");
        let start = Instant::now();
        let sample_size = 100;
        let mut verified = 0;
        for i in 0..sample_size {
            let file_path = format!("test/file_{}.mid", i);
            let retrieved = db
                .get_played_file_stats_with_statistics(file_path)
                .expect("Failed to retrieve stats")
                .expect("Stats not found");

            assert_eq!(
                retrieved.total_play_number, events_per_file as u32,
                "File {} should have {} events",
                i, events_per_file
            );
            verified += 1;
        }
        let verify_duration = start.elapsed();
        println!(
            "✓ Verified {} files in {}ms (avg: {:.2}ms per query)",
            verified,
            verify_duration.as_millis(),
            verify_duration.as_millis() as f64 / verified as f64
        );

        // Phase 4: Performance test - query random files
        println!("\nPhase 4: Performance test - querying random files...");
        let query_count = 1000;
        let start = Instant::now();
        // Use a simple pseudo-random sequence for testing
        let mut seed: u64 = 12345;
        for _ in 0..query_count {
            // Simple LCG for pseudo-random numbers
            seed = seed.wrapping_mul(1103515245).wrapping_add(12345);
            let random_file_idx = (seed % file_count as u64) as usize;
            let file_path = format!("test/file_{}.mid", random_file_idx);
            let _retrieved = db
                .get_played_file_stats_with_statistics(file_path)
                .expect("Failed to retrieve stats")
                .expect("Stats not found");
        }
        let query_duration = start.elapsed();
        let avg_query_time = query_duration.as_millis() as f64 / query_count as f64;
        println!(
            "✓ Queried {} random files in {}ms (avg: {:.2}ms per query)",
            query_count,
            query_duration.as_millis(),
            avg_query_time
        );

        // Phase 5: Check index usage
        println!("\nPhase 5: Verifying index usage...");
        let index_count: i64 = db
            .connection
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master \
                WHERE type='index' AND name LIKE 'idx_%'",
                [],
                |row| row.get(0),
            )
            .expect("Failed to query indexes");
        println!("✓ Found {} indexes", index_count);
        assert!(index_count >= 4, "Expected at least 4 indexes");

        // Performance assertions
        println!("\n=== Performance Summary ===");
        println!("File insert: {:.2}ms per file", avg_file_insert);
        println!("Event insert: {:.3}ms per event", avg_event_insert);
        println!("Query: {:.2}ms per query", avg_query_time);

        // Assertions - these should be reasonable even for large datasets
        assert!(
            avg_file_insert < 100.0,
            "File insert too slow: {:.2}ms per file",
            avg_file_insert
        );
        assert!(
            avg_event_insert < 1.0,
            "Event insert too slow: {:.3}ms per event",
            avg_event_insert
        );
        assert!(
            avg_query_time < 10.0,
            "Query too slow: {:.2}ms per query (with indexes)",
            avg_query_time
        );

        // Phase 6: Durability test - close and reopen database
        println!("\nPhase 6: Testing data durability (close and reopen database)...");
        let db_path = db.db_path().to_string();

        // Get some reference data before closing
        let sample_files: Vec<String> = (0..10)
            .map(|i| format!("test/file_{}.mid", i * 1000))
            .collect();

        let mut reference_data = Vec::new();
        for file_path in &sample_files {
            let stats = db
                .get_played_file_stats_with_statistics(file_path.clone())
                .expect("Failed to retrieve stats")
                .expect("Stats not found");
            reference_data.push((file_path.clone(), stats));
        }

        // Count total files and events before closing
        let total_files_before: i64 = db
            .connection
            .query_row("SELECT COUNT(*) FROM played_file_stats", [], |row| {
                row.get(0)
            })
            .expect("Failed to count files");

        let total_events_before: i64 = db
            .connection
            .query_row(
                "SELECT COUNT(*) FROM played_file_stats_history",
                [],
                |row| row.get(0),
            )
            .expect("Failed to count events");

        println!(
            "  Before close: {} files, {} events",
            total_files_before, total_events_before
        );

        // Close the database (drop the connection)
        drop(db);

        // Ensure WAL checkpoint is complete (flush all changes to main database)
        // This is important for WAL mode to ensure all data is persisted
        {
            let temp_conn =
                Connection::open(&db_path).expect("Failed to reopen database for checkpoint");
            // Force a checkpoint to ensure all WAL data is written to main database
            let _ = temp_conn.execute("PRAGMA wal_checkpoint(TRUNCATE)", ());
            drop(temp_conn);
        }

        // Reopen the database
        println!("  Reopening database...");
        let db_reopened =
            PlayMetadataDatabase::new(db_path.clone()).expect("Failed to reopen database");

        // Verify total counts match
        let total_files_after: i64 = db_reopened
            .connection
            .query_row("SELECT COUNT(*) FROM played_file_stats", [], |row| {
                row.get(0)
            })
            .expect("Failed to count files after reopen");

        let total_events_after: i64 = db_reopened
            .connection
            .query_row(
                "SELECT COUNT(*) FROM played_file_stats_history",
                [],
                |row| row.get(0),
            )
            .expect("Failed to count events after reopen");

        println!(
            "  After reopen: {} files, {} events",
            total_files_after, total_events_after
        );

        assert_eq!(
            total_files_before, total_files_after,
            "File count mismatch: {} before, {} after",
            total_files_before, total_files_after
        );

        assert_eq!(
            total_events_before, total_events_after,
            "Event count mismatch: {} before, {} after",
            total_events_before, total_events_after
        );

        // Verify sample files still have correct data
        println!("  Verifying sample files...");
        for (file_path, original_stats) in &reference_data {
            let retrieved = db_reopened
                .get_played_file_stats_with_statistics(file_path.clone())
                .expect("Failed to retrieve stats after reopen")
                .expect("Stats not found after reopen");

            assert_eq!(
                retrieved.relative_file_path, original_stats.relative_file_path,
                "File path mismatch for {}",
                file_path
            );

            assert_eq!(
                retrieved.total_play_number, original_stats.total_play_number,
                "Event count mismatch for {}: {} vs {}",
                file_path, retrieved.total_play_number, original_stats.total_play_number
            );

            // Check latest play time (allow 1 second difference for rounding)
            let time_diff = (retrieved.latest_play_time - original_stats.latest_play_time)
                .num_seconds()
                .abs();
            assert!(
                time_diff <= 1,
                "Latest play time mismatch for {}: difference {} seconds",
                file_path,
                time_diff
            );
        }

        // Verify a random sample of files across the entire range
        println!("  Verifying random sample across entire dataset...");
        let mut seed: u64 = 54321; // Different seed for different random selection
        let verification_sample_size = 50;
        let mut verified_count = 0;

        for _ in 0..verification_sample_size {
            seed = seed.wrapping_mul(1103515245).wrapping_add(12345);
            let random_file_idx = (seed % file_count as u64) as usize;
            let file_path = format!("test/file_{}.mid", random_file_idx);

            let retrieved = db_reopened
                .get_played_file_stats_with_statistics(file_path)
                .expect("Failed to retrieve stats")
                .expect("Stats not found");

            assert_eq!(
                retrieved.total_play_number, events_per_file as u32,
                "File {} should have {} events after reopen",
                random_file_idx, events_per_file
            );
            verified_count += 1;
        }

        println!("✓ Verified {} random files after reopen", verified_count);
        println!("✓ All data persisted correctly - no data loss detected!");

        // Clean up test database file
        let _ = fs::remove_file(&db_path);

        println!("\n✓ All performance and durability tests passed!\n");
    }
}
