//! Play metadata manager - handles background thread for play count queries
//! and recording play events with low priority to not interfere with playback

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use chrono::Utc;
use log::{debug, error, info, warn};

use crate::playmetadata::PlayMetadataDatabase;

/// Commands sent to the background metadata thread
#[derive(Debug, Clone)]
pub enum MetadataCommand {
    /// Request play counts for a list of file paths
    /// First Vec contains high-priority (visible) files, second Vec contains low-priority (all other) files
    QueryPlayCounts(Vec<PathBuf>, Vec<PathBuf>),
    /// Record a play event for a file
    RecordPlayEvent(PathBuf),
    /// Record a star event for a file
    RecordStarEvent(PathBuf),
    /// Update the database path (when folder changes)
    UpdateDatabasePath(PathBuf),
}

/// File metadata (play count and star count)
#[derive(Debug, Clone)]
pub struct FileMetadata {
    pub play_count: u32,
    pub star_count: u32,
}

/// Results sent back from the background thread
#[derive(Debug, Clone)]
pub enum MetadataResult {
    /// Play counts and star counts for files (path -> metadata)
    PlayCounts(HashMap<PathBuf, FileMetadata>),
}

/// Play metadata manager - coordinates background thread for metadata operations
pub struct PlayMetadataManager {
    /// Sender to send commands to background thread
    command_sender: Sender<MetadataCommand>,
    /// Receiver for results from background thread
    result_receiver: Receiver<MetadataResult>,
    /// Current database path
    database_path: Arc<Mutex<Option<PathBuf>>>,
    /// Flag to track if database is ready (initialized)
    database_ready: Arc<Mutex<bool>>,
}

impl PlayMetadataManager {
    /// Create a new metadata manager with background thread
    pub fn new() -> Self {
        let (command_sender, command_receiver) = channel();
        let (result_sender, result_receiver) = channel();
        let database_path = Arc::new(Mutex::new(None));

        // Spawn background thread with low priority
        let db_path_clone = Arc::clone(&database_path);
        let database_ready = Arc::new(Mutex::new(false));
        let database_ready_clone = Arc::clone(&database_ready);
        thread::Builder::new()
            .name("playmetadata-worker".to_string())
            .spawn(move || {
                Self::background_thread_worker(
                    command_receiver,
                    result_sender,
                    db_path_clone,
                    database_ready_clone,
                );
            })
            .expect("Failed to spawn metadata background thread");

        Self {
            command_sender,
            result_receiver,
            database_path,
            database_ready,
        }
    }

    /// Background thread worker - processes commands with low priority
    fn background_thread_worker(
        command_receiver: Receiver<MetadataCommand>,
        result_sender: Sender<MetadataResult>,
        database_path: Arc<Mutex<Option<PathBuf>>>,
        database_ready: Arc<Mutex<bool>>,
    ) {
        let mut database: Option<PlayMetadataDatabase> = None;
        let mut folder_path: Option<PathBuf> = None; // Store folder path separately

        loop {
            // Use try_recv with timeout to allow periodic checks and low CPU usage
            match command_receiver.recv_timeout(Duration::from_millis(100)) {
                Ok(command) => {
                    match command {
                        MetadataCommand::UpdateDatabasePath(new_path) => {
                            debug!(
                                "Background thread: Received UpdateDatabasePath command for {:?}",
                                new_path
                            );
                            // Close old database if exists
                            drop(database);
                            database = None;
                            *database_ready.lock().unwrap() = false;

                            // Extract folder path (parent of database file)
                            let folder = new_path.parent().map(|p| p.to_path_buf());

                            debug!(
                                "Background thread: Attempting to open database at {:?}",
                                new_path
                            );
                            // Try to open new database
                            match PlayMetadataDatabase::new(new_path.to_string_lossy().to_string())
                            {
                                Ok(db) => {
                                    info!(
                                        "Metadata database successfully opened at {:?}",
                                        new_path
                                    );
                                    database = Some(db);
                                    folder_path = folder.clone();
                                    *database_path.lock().unwrap() = Some(new_path);
                                    *database_ready.lock().unwrap() = true;
                                    debug!(
                                        "Background thread: Database ready, folder_path={:?}",
                                        folder
                                    );
                                }
                                Err(e) => {
                                    error!(
                                        "Failed to initialize metadata database at {:?}",
                                        new_path
                                    );
                                    error!("  Error details: {}", e);
                                    error!(
                                        "  Play count tracking will be disabled for this folder"
                                    );
                                    error!("  The application will continue to work, but play statistics will not be recorded");
                                    // Don't report error to user - just continue without database
                                    folder_path = None;
                                    *database_path.lock().unwrap() = None;
                                    *database_ready.lock().unwrap() = false;
                                }
                            }
                        }
                        MetadataCommand::QueryPlayCounts(visible_paths, _other_paths) => {
                            debug!(
                                "Background thread: Received query for {} visible files",
                                visible_paths.len()
                            );
                            if visible_paths.is_empty() {
                                debug!("Background thread: No visible files to query, skipping");
                                continue;
                            }

                            // Check if database is ready
                            let is_ready = *database_ready.lock().unwrap();
                            if !is_ready {
                                warn!("Background thread: Database not ready yet, skipping query for {} files", visible_paths.len());
                                // Send empty results so UI doesn't wait forever
                                let _ =
                                    result_sender.send(MetadataResult::PlayCounts(HashMap::new()));
                                continue;
                            }

                            // Only process visible files - ignore other_paths to avoid fetching metadata for non-displayed files
                            if let Some(ref db) = database {
                                if let Some(ref folder) = folder_path {
                                    debug!("Background thread: Database and folder available, folder={:?}", folder);
                                    let mut file_metadata = HashMap::new();

                                    // Helper function to query a single path
                                    let mut query_path = |path: &PathBuf| {
                                        // Get relative path from folder path
                                        if let Ok(relative_path) = path.strip_prefix(folder) {
                                            // Normalize path separators for cross-platform compatibility
                                            let relative_str =
                                                relative_path.to_string_lossy().replace('\\', "/");

                                            debug!("Background thread: Querying metadata for relative path: {}", relative_str);

                                            // Query with error handling (fail silently)
                                            match db.get_played_file_stats_with_statistics(
                                                relative_str.clone(),
                                            ) {
                                                Ok(Some(stats)) => {
                                                    debug!("Background thread: Found metadata for {}: play_count={}, star_count={}", relative_str, stats.total_play_number, stats.total_star_count);
                                                    file_metadata.insert(
                                                        path.clone(),
                                                        FileMetadata {
                                                            play_count: stats.total_play_number,
                                                            star_count: stats.total_star_count,
                                                        },
                                                    );
                                                }
                                                Ok(None) => {
                                                    // File not in database yet - counts are 0
                                                    debug!("Background thread: No stats found for {}, using counts 0", relative_str);
                                                    file_metadata.insert(
                                                        path.clone(),
                                                        FileMetadata {
                                                            play_count: 0,
                                                            star_count: 0,
                                                        },
                                                    );
                                                }
                                                Err(e) => {
                                                    warn!("Background thread: Error querying metadata for {:?} (relative: {}): {}", path, relative_str, e);
                                                    // Fail silently - don't break the app
                                                }
                                            }
                                        } else {
                                            warn!("Background thread: Could not strip prefix: path={:?}, folder={:?}", path, folder);
                                        }
                                    };

                                    // Only process visible files (no delay - they're the only ones we fetch)
                                    debug!("Background thread: Processing {} visible files only (skipping non-displayed files)", visible_paths.len());
                                    for (idx, path) in visible_paths.iter().enumerate() {
                                        debug!(
                                            "Background thread: Processing file {}/{}: {:?}",
                                            idx + 1,
                                            visible_paths.len(),
                                            path
                                        );
                                        query_path(path);
                                        // Small delay to keep CPU usage low
                                        thread::sleep(Duration::from_millis(1));
                                    }

                                    debug!("Background thread: Sending {} metadata results back (only visible files)", file_metadata.len());
                                    if file_metadata.len() <= 5 {
                                        for (path, metadata) in &file_metadata {
                                            debug!("Background thread:   Result: {:?} -> play={}, star={}", path, metadata.play_count, metadata.star_count);
                                        }
                                    }
                                    // Send results back (ignore if receiver is dropped)
                                    if let Err(e) = result_sender
                                        .send(MetadataResult::PlayCounts(file_metadata))
                                    {
                                        warn!("Background thread: Failed to send results (receiver may be dropped): {}", e);
                                    } else {
                                        debug!("Background thread: Successfully sent results");
                                    }
                                } else {
                                    warn!("Background thread: No folder path available for query - database may not be properly initialized");
                                    let _ = result_sender
                                        .send(MetadataResult::PlayCounts(HashMap::new()));
                                }
                            } else {
                                warn!("Background thread: No database available for query - metadata database was not initialized");
                                // No database - return empty counts
                                let _ =
                                    result_sender.send(MetadataResult::PlayCounts(HashMap::new()));
                            }
                        }
                        MetadataCommand::RecordPlayEvent(path) => {
                            if let Some(ref db) = database {
                                if let Some(ref folder) = folder_path {
                                    // Get relative path from folder path
                                    if let Ok(relative_path) = path.strip_prefix(folder) {
                                        // Normalize path separators
                                        let relative_str =
                                            relative_path.to_string_lossy().replace('\\', "/");

                                        debug!(
                                            "Recording play event for relative path: {}",
                                            relative_str
                                        );

                                        // First, ensure the file is in the database
                                        // Use a dummy stats entry if needed
                                        match db.insert_or_update_played_file_stats(
                                            crate::playmetadata::PlayedFileStats {
                                                relative_file_path: relative_str.clone(),
                                                file_md5_checksum: String::new(), // Not computed for performance
                                                latest_play_time: Utc::now(),
                                                total_play_number: 0, // Will be computed from history
                                                total_star_count: 0, // Will be computed from star history
                                                user_comments_or_notes: String::new(),
                                            },
                                        ) {
                                            Ok(_) => {
                                                debug!(
                                                    "Inserted/updated file stats for {}",
                                                    relative_str
                                                );
                                            }
                                            Err(e) => {
                                                error!("Error inserting/updating file stats for {}: {}", relative_str, e);
                                            }
                                        }

                                        // Record the play event
                                        let play_event_recorded = match db
                                            .add_play_event(relative_str.clone(), Utc::now())
                                        {
                                            Ok(_) => {
                                                info!("Recorded play event for {}", relative_str);
                                                true
                                            }
                                            Err(e) => {
                                                error!(
                                                    "Error recording play event for {}: {}",
                                                    relative_str, e
                                                );
                                                false
                                            }
                                        };

                                        // If play event was recorded successfully, immediately query the updated count
                                        if play_event_recorded {
                                            match db.get_played_file_stats_with_statistics(
                                                relative_str.clone(),
                                            ) {
                                                Ok(Some(stats)) => {
                                                    info!(
                                                        "Metadata updated: {} -> play={}, star={}",
                                                        relative_str,
                                                        stats.total_play_number,
                                                        stats.total_star_count
                                                    );
                                                    let mut file_metadata = HashMap::new();
                                                    file_metadata.insert(
                                                        path.clone(),
                                                        FileMetadata {
                                                            play_count: stats.total_play_number,
                                                            star_count: stats.total_star_count,
                                                        },
                                                    );
                                                    // Send updated metadata back immediately
                                                    if let Err(e) = result_sender.send(
                                                        MetadataResult::PlayCounts(file_metadata),
                                                    ) {
                                                        warn!(
                                                            "Failed to send updated metadata: {}",
                                                            e
                                                        );
                                                    } else {
                                                        debug!("Sent updated metadata back to main thread");
                                                    }
                                                }
                                                Ok(None) => {
                                                    // File not found (shouldn't happen, but handle gracefully)
                                                    warn!("File not found after recording play event: {}", relative_str);
                                                }
                                                Err(e) => {
                                                    error!("Error querying updated play count for {}: {}", relative_str, e);
                                                }
                                            }
                                        } else {
                                            warn!("Play event was not recorded, skipping count refresh");
                                        }
                                    } else {
                                        debug!("Could not strip prefix for play event: path={:?}, folder={:?}", path, folder);
                                    }
                                }
                            }
                            // If no database, silently ignore (don't break the app)
                        }
                        MetadataCommand::RecordStarEvent(path) => {
                            if let Some(ref db) = database {
                                if let Some(ref folder) = folder_path {
                                    // Get relative path from folder path
                                    if let Ok(relative_path) = path.strip_prefix(folder) {
                                        // Normalize path separators
                                        let relative_str =
                                            relative_path.to_string_lossy().replace('\\', "/");

                                        debug!(
                                            "Recording star event for relative path: {}",
                                            relative_str
                                        );

                                        // First, ensure the file is in the database
                                        match db.insert_or_update_played_file_stats(
                                            crate::playmetadata::PlayedFileStats {
                                                relative_file_path: relative_str.clone(),
                                                file_md5_checksum: String::new(), // Not computed for performance
                                                latest_play_time: Utc::now(),
                                                total_play_number: 0, // Will be computed from history
                                                total_star_count: 0, // Will be computed from star history
                                                user_comments_or_notes: String::new(),
                                            },
                                        ) {
                                            Ok(_) => {
                                                debug!("Inserted/updated file stats for star event: {}", relative_str);
                                            }
                                            Err(e) => {
                                                error!("Error inserting/updating file stats for star event {}: {}", relative_str, e);
                                            }
                                        }

                                        // Record the star event
                                        match db.add_star_event(relative_str.clone(), Utc::now()) {
                                            Ok(_) => {
                                                info!(
                                                    "Successfully recorded star event for {}",
                                                    relative_str
                                                );

                                                // Immediately query the updated star count
                                                match db.get_played_file_stats_with_statistics(
                                                    relative_str.clone(),
                                                ) {
                                                    Ok(Some(stats)) => {
                                                        info!("Star count updated: {} -> play={}, star={}", relative_str, stats.total_play_number, stats.total_star_count);
                                                        // Send updated metadata back immediately
                                                        let mut file_metadata = HashMap::new();
                                                        file_metadata.insert(
                                                            path.clone(),
                                                            FileMetadata {
                                                                play_count: stats.total_play_number,
                                                                star_count: stats.total_star_count,
                                                            },
                                                        );
                                                        if let Err(e) = result_sender.send(
                                                            MetadataResult::PlayCounts(
                                                                file_metadata,
                                                            ),
                                                        ) {
                                                            warn!("Failed to send updated star metadata: {}", e);
                                                        } else {
                                                            debug!("Sent updated star metadata back to main thread");
                                                        }
                                                    }
                                                    Ok(None) => {
                                                        warn!("File not found after recording star event: {}", relative_str);
                                                    }
                                                    Err(e) => {
                                                        error!("Error querying updated star count for {}: {}", relative_str, e);
                                                    }
                                                }
                                            }
                                            Err(e) => {
                                                error!(
                                                    "Error recording star event for {}: {}",
                                                    relative_str, e
                                                );
                                            }
                                        }
                                    } else {
                                        debug!("Could not strip prefix for star event: path={:?}, folder={:?}", path, folder);
                                    }
                                }
                            }
                            // If no database, silently ignore (don't break the app)
                        }
                    }
                }
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                    // Timeout - check if we should continue or exit
                    // Continue looping
                }
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                    // Sender dropped - exit thread
                    debug!("Metadata command channel closed, exiting background thread");
                    break;
                }
            }
        }
    }

    /// Set the database path (called when folder is selected)
    pub fn set_database_path(&self, path: PathBuf) {
        debug!("Setting metadata database path to: {:?}", path);
        if let Err(e) = self
            .command_sender
            .send(MetadataCommand::UpdateDatabasePath(path))
        {
            error!("Failed to send database path update command: {}", e);
            error!("Metadata database will not be initialized");
        }
    }

    /// Request play counts for displayed files
    /// visible_paths: Files that are currently visible/expanded in the UI (high priority)
    /// all_paths: All files in the view (low priority, processed after visible ones)
    pub fn query_play_counts(&self, visible_paths: Vec<PathBuf>, all_paths: Vec<PathBuf>) {
        debug!(
            "MetadataManager: Sending query for {} visible files, {} other files",
            visible_paths.len(),
            all_paths.len()
        );
        if visible_paths.len() <= 5 {
            for path in &visible_paths {
                debug!("  Querying visible file: {:?}", path);
            }
        }
        if let Err(e) = self
            .command_sender
            .send(MetadataCommand::QueryPlayCounts(visible_paths, all_paths))
        {
            error!("MetadataManager: Failed to send query command: {}", e);
        }
    }

    /// Query play count for a single file (for currently playing file)
    pub fn query_current_file_play_count(&self, path: PathBuf) {
        debug!(
            "MetadataManager: Querying play count for current playing file: {:?}",
            path
        );
        // Query as high priority visible file
        if let Err(e) = self
            .command_sender
            .send(MetadataCommand::QueryPlayCounts(vec![path], Vec::new()))
        {
            error!("MetadataManager: Failed to send current file query: {}", e);
        }
    }

    /// Record a play event for a file
    pub fn record_play_event(&self, path: PathBuf) {
        debug!("MetadataManager: Recording play event for file: {:?}", path);
        if let Err(e) = self
            .command_sender
            .send(MetadataCommand::RecordPlayEvent(path))
        {
            error!(
                "MetadataManager: Failed to send record play event command: {}",
                e
            );
        }
    }

    /// Record a star event for a file
    pub fn record_star_event(&self, path: PathBuf) {
        info!("MetadataManager: Recording star event for file: {:?}", path);
        if let Err(e) = self
            .command_sender
            .send(MetadataCommand::RecordStarEvent(path))
        {
            error!(
                "MetadataManager: Failed to send record star event command: {}",
                e
            );
        }
    }

    /// Check for and process any results from background thread
    /// Returns file metadata (play counts and star counts) if available
    pub fn process_results(&self) -> Option<HashMap<PathBuf, FileMetadata>> {
        // Use try_recv to avoid blocking
        match self.result_receiver.try_recv() {
            Ok(MetadataResult::PlayCounts(metadata)) => {
                debug!(
                    "MetadataManager: Received {} metadata results from background thread",
                    metadata.len()
                );
                Some(metadata)
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => {
                // No results yet - this is normal
                None
            }
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                error!("MetadataManager: Result channel disconnected - background thread may have died");
                None
            }
        }
    }
}

impl Default for PlayMetadataManager {
    fn default() -> Self {
        Self::new()
    }
}
