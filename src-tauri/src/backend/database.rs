// VoiceMemoLiberator - Voice memo transcription and management tool
// Copyright (C) 2026 APPSTART LLC
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.

use anyhow::Result;
use rusqlite::{Connection, params};
use std::path::Path;

use super::models::{Recording, Transcript, RecordingWithTranscript, Stats, YearCount, AudioLengthBucket, Slice, Label};

pub struct Database {
    conn: Connection,
}

impl Database {
    pub fn new<P: AsRef<Path>>(db_path: P) -> Result<Self> {
        let conn = Connection::open(db_path)?;
        let db = Database { conn };
        db.init_schema()?;
        Ok(db)
    }

    fn init_schema(&self) -> Result<()> {
        // Create recordings table
        self.conn.execute(
            r#"
            CREATE TABLE IF NOT EXISTS recordings (
                id             INTEGER PRIMARY KEY,
                apple_id       INTEGER UNIQUE,
                created_at     INTEGER,
                duration_sec   REAL,
                title          TEXT,
                original_path  TEXT UNIQUE,
                copied_path    TEXT UNIQUE,
                file_size      INTEGER,
                mime_type      TEXT DEFAULT 'audio/m4a',
                year           INTEGER
            )
            "#,
            [],
        )?;

        // Create transcripts table
        self.conn.execute(
            r#"
            CREATE TABLE IF NOT EXISTS transcripts (
                id            INTEGER PRIMARY KEY,
                recording_id  INTEGER REFERENCES recordings(id) ON DELETE CASCADE,
                model         TEXT,
                started_at    INTEGER,
                finished_at   INTEGER,
                word_count    INTEGER,
                text_path     TEXT UNIQUE,
                success       INTEGER DEFAULT 0,
                error_message TEXT
            )
            "#,
            [],
        )?;

        // Create slices table
        self.conn.execute(
            r#"
            CREATE TABLE IF NOT EXISTS slices (
                id                          INTEGER PRIMARY KEY,
                original_audio_file_name    TEXT UNIQUE,
                title                       TEXT,
                transcribed                 INTEGER DEFAULT 0,
                audio_file_size             INTEGER,
                audio_file_type             TEXT,
                estimated_time_to_transcribe INTEGER,
                transcription               TEXT,
                transcription_time_taken    INTEGER,
                transcription_word_count    INTEGER,
                transcription_model         TEXT,
                recording_date              INTEGER
            )
            "#,
            [],
        )?;

        // Migration: Add transcription_model column if it doesn't exist (for existing databases)
        let _ = self.conn.execute(
            "ALTER TABLE slices ADD COLUMN transcription_model TEXT",
            [],
        );

        // Migration: Add recording_date column if it doesn't exist (for existing databases)
        let _ = self.conn.execute(
            "ALTER TABLE slices ADD COLUMN recording_date INTEGER",
            [],
        );

        // Create labels table for label definitions
        self.conn.execute(
            r#"
            CREATE TABLE IF NOT EXISTS labels (
                id       INTEGER PRIMARY KEY,
                name     TEXT NOT NULL,
                color    TEXT NOT NULL DEFAULT '#228be6',
                keywords TEXT NOT NULL DEFAULT ''
            )
            "#,
            [],
        )?;

        // Add title column to existing slices tables (migration)
        let _ = self.conn.execute(
            "ALTER TABLE slices ADD COLUMN title TEXT",
            [],
        ); // Ignore error if column already exists

        // Add audio_time_length_seconds column to existing slices tables (migration)
        let _ = self.conn.execute(
            "ALTER TABLE slices ADD COLUMN audio_time_length_seconds REAL",
            [],
        ); // Ignore error if column already exists

        // Create indexes
        self.conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_transcripts_recording ON transcripts(recording_id)",
            [],
        )?;

        self.conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_slices_filename ON slices(original_audio_file_name)",
            [],
        )?;

        Ok(())
    }

    #[allow(dead_code)]
    pub fn insert_recording(&self, recording: &Recording) -> Result<i64> {
        let _rows = self.conn.execute(
            r#"
            INSERT INTO recordings (
                apple_id, created_at, duration_sec, title, original_path, 
                copied_path, file_size, mime_type, year
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
            "#,
            params![
                recording.apple_id,
                recording.created_at,
                recording.duration_sec,
                recording.title,
                recording.original_path,
                recording.copied_path,
                recording.file_size,
                recording.mime_type,
                recording.year,
            ],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    #[allow(dead_code)]
    pub fn get_recording_by_apple_id(&self, apple_id: i64) -> Result<Option<Recording>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, apple_id, created_at, duration_sec, title, original_path, copied_path, file_size, mime_type, year FROM recordings WHERE apple_id = ?1"
        )?;

        let recording = stmt.query_row(params![apple_id], |row| {
            Ok(Recording {
                id: Some(row.get(0)?),
                apple_id: row.get(1)?,
                created_at: row.get(2)?,
                duration_sec: row.get(3)?,
                title: row.get(4)?,
                original_path: row.get(5)?,
                copied_path: row.get(6)?,
                file_size: row.get(7)?,
                mime_type: row.get(8)?,
                year: row.get(9)?,
            })
        });

        match recording {
            Ok(r) => Ok(Some(r)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    pub fn list_recordings(&self, limit: Option<u32>, offset: Option<u32>) -> Result<Vec<RecordingWithTranscript>> {
        let limit_clause = limit.map(|l| format!("LIMIT {}", l)).unwrap_or_default();
        let offset_clause = offset.map(|o| format!("OFFSET {}", o)).unwrap_or_default();

        let query = format!(
            r#"
            SELECT 
                r.id, r.apple_id, r.created_at, r.duration_sec, r.title, 
                r.original_path, r.copied_path, r.file_size, r.mime_type, r.year,
                COUNT(t.id) as transcript_count,
                CASE WHEN COUNT(CASE WHEN t.success = 1 THEN 1 END) > 0 THEN 1 ELSE 0 END as has_successful_transcript,
                (SELECT text_path FROM transcripts WHERE recording_id = r.id AND success = 1 ORDER BY finished_at DESC LIMIT 1) as latest_transcript_path
            FROM recordings r
            LEFT JOIN transcripts t ON r.id = t.recording_id
            GROUP BY r.id
            ORDER BY r.created_at DESC
            {} {}
            "#,
            limit_clause, offset_clause
        );

        let mut stmt = self.conn.prepare(&query)?;
        let rows = stmt.query_map([], |row| {
            let transcript_path: Option<String> = row.get(12)?;
            let latest_transcript_text = transcript_path
                .and_then(|path| std::fs::read_to_string(path).ok());

            Ok(RecordingWithTranscript {
                recording: Recording {
                    id: Some(row.get(0)?),
                    apple_id: row.get(1)?,
                    created_at: row.get(2)?,
                    duration_sec: row.get(3)?,
                    title: row.get(4)?,
                    original_path: row.get(5)?,
                    copied_path: row.get(6)?,
                    file_size: row.get(7)?,
                    mime_type: row.get(8)?,
                    year: row.get(9)?,
                },
                transcript_count: row.get(10)?,
                has_successful_transcript: row.get::<_, i32>(11)? == 1,
                latest_transcript_text,
            })
        })?;

        let mut recordings = Vec::new();
        for row in rows {
            recordings.push(row?);
        }
        Ok(recordings)
    }

    pub fn insert_transcript(&self, transcript: &Transcript) -> Result<i64> {
        self.conn.execute(
            r#"
            INSERT INTO transcripts (
                recording_id, model, started_at, finished_at, 
                word_count, text_path, success, error_message
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
            "#,
            params![
                transcript.recording_id,
                transcript.model,
                transcript.started_at,
                transcript.finished_at,
                transcript.word_count,
                transcript.text_path,
                transcript.success as i32,
                transcript.error_message,
            ],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn update_transcript(&self, id: i64, transcript: &Transcript) -> Result<()> {
        self.conn.execute(
            r#"
            UPDATE transcripts SET
                finished_at = ?1,
                word_count = ?2,
                text_path = ?3,
                success = ?4,
                error_message = ?5
            WHERE id = ?6
            "#,
            params![
                transcript.finished_at,
                transcript.word_count,
                transcript.text_path,
                transcript.success as i32,
                transcript.error_message,
                id,
            ],
        )?;
        Ok(())
    }

    pub fn get_stats(&self) -> Result<Stats> {
        // Total files from slices table
        let total_files: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM slices",
            [],
            |row| row.get(0),
        )?;

        // Total transcribed from slices table
        let total_transcribed: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM slices WHERE transcribed = 1",
            [],
            |row| row.get(0),
        )?;

        // Average transcription time per 10 minutes of audio from slices table
        let avg_transcribe_sec_10m: Option<f64> = self.conn.query_row(
            r#"
            SELECT AVG(transcription_time_taken / (audio_file_size / 1048576.0 / 60.0) * 10.0) 
            FROM slices 
            WHERE transcribed = 1 AND transcription_time_taken IS NOT NULL AND audio_file_size > 0
            "#,
            [],
            |row| row.get(0),
        ).unwrap_or(None);

        // Total audio bytes from slices table
        let total_audio_bytes: i64 = self.conn.query_row(
            "SELECT COALESCE(SUM(audio_file_size), 0) FROM slices",
            [],
            |row| row.get(0),
        )?;

        // Largest file bytes from slices table
        let largest_file_bytes: i64 = self.conn.query_row(
            "SELECT COALESCE(MAX(audio_file_size), 0) FROM slices",
            [],
            |row| row.get(0),
        )?;

        // Average file bytes from slices table
        let avg_file_bytes: f64 = self.conn.query_row(
            "SELECT COALESCE(AVG(audio_file_size), 0.0) FROM slices",
            [],
            |row| row.get(0),
        )?;

        // Count by year - extract from Apple's ZCLOUDRECORDING table if available
        let count_by_year = self.get_count_by_year_from_apple_db().unwrap_or_else(|_| Vec::new());

        // Count by audio length
        let count_by_audio_length = self.get_count_by_audio_length().unwrap_or_else(|_| Vec::new());

        Ok(Stats {
            total_files,
            total_transcribed,
            avg_transcribe_sec_10m,
            total_audio_bytes,
            largest_file_bytes,
            avg_file_bytes,
            count_by_year,
            count_by_audio_length,
        })
    }

    /// Get the average transcription speed in bytes per second of processing time.
    /// This is calculated from historical transcription data.
    /// Returns bytes_per_second (how many bytes of audio file can be transcribed per second of processing time).
    pub fn get_transcription_speed(&self) -> Result<f64> {
        // Calculate: SUM(audio_file_size) / SUM(transcription_time_taken)
        // This gives us bytes per second of processing time
        let result: Option<f64> = self.conn.query_row(
            r#"
            SELECT
                CASE
                    WHEN SUM(transcription_time_taken) > 0
                    THEN CAST(SUM(audio_file_size) AS REAL) / CAST(SUM(transcription_time_taken) AS REAL)
                    ELSE NULL
                END
            FROM slices
            WHERE transcribed = 1
              AND transcription_time_taken IS NOT NULL
              AND transcription_time_taken > 0
              AND audio_file_size > 0
            "#,
            [],
            |row| row.get(0),
        ).unwrap_or(None);

        // Default to a reasonable estimate if no historical data:
        // Assume ~1MB per 30 seconds of processing = ~34000 bytes/second
        Ok(result.unwrap_or(34000.0))
    }

    pub fn search_recordings(&self, query: &str, limit: Option<u32>, offset: Option<u32>) -> Result<Vec<RecordingWithTranscript>> {
        let limit_clause = limit.map(|l| format!("LIMIT {}", l)).unwrap_or_default();
        let offset_clause = offset.map(|o| format!("OFFSET {}", o)).unwrap_or_default();

        let sql = format!(
            r#"
            SELECT DISTINCT
                r.id, r.apple_id, r.created_at, r.duration_sec, r.title, 
                r.original_path, r.copied_path, r.file_size, r.mime_type, r.year,
                COUNT(t.id) as transcript_count,
                CASE WHEN COUNT(CASE WHEN t.success = 1 THEN 1 END) > 0 THEN 1 ELSE 0 END as has_successful_transcript,
                (SELECT text_path FROM transcripts WHERE recording_id = r.id AND success = 1 ORDER BY finished_at DESC LIMIT 1) as latest_transcript_path
            FROM recordings r
            LEFT JOIN transcripts t ON r.id = t.recording_id
            WHERE EXISTS (
                SELECT 1 FROM transcripts t2 
                WHERE t2.recording_id = r.id 
                AND t2.text_path IS NOT NULL 
                AND EXISTS (
                    SELECT 1 FROM transcripts t3 
                    WHERE t3.id = t2.id 
                    AND (
                        SELECT CASE 
                            WHEN t3.text_path IS NOT NULL 
                            THEN (
                                SELECT CASE 
                                    WHEN LENGTH(TRIM(COALESCE((
                                        SELECT substr(hex(RANDOMBLOB(4)), 1, 8) -- This is a placeholder for file content reading
                                    ), ''))) > 0 
                                    THEN 1 
                                    ELSE 0 
                                END
                            )
                            ELSE 0 
                        END
                    ) = 1
                )
            )
            OR r.title LIKE ?1
            GROUP BY r.id
            ORDER BY r.created_at DESC
            {} {}
            "#,
            limit_clause, offset_clause
        );

        let search_pattern = format!("%{}%", query);
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map([&search_pattern], |row| {
            let transcript_path: Option<String> = row.get(12)?;
            let latest_transcript_text = transcript_path
                .and_then(|path| std::fs::read_to_string(path).ok());

            Ok(RecordingWithTranscript {
                recording: Recording {
                    id: Some(row.get(0)?),
                    apple_id: row.get(1)?,
                    created_at: row.get(2)?,
                    duration_sec: row.get(3)?,
                    title: row.get(4)?,
                    original_path: row.get(5)?,
                    copied_path: row.get(6)?,
                    file_size: row.get(7)?,
                    mime_type: row.get(8)?,
                    year: row.get(9)?,
                },
                transcript_count: row.get(10)?,
                has_successful_transcript: row.get::<_, i32>(11)? == 1,
                latest_transcript_text,
            })
        })?;

        let mut recordings = Vec::new();
        for row in rows {
            recordings.push(row?);
        }
        Ok(recordings)
    }

    pub fn insert_slice(&self, slice: &Slice) -> Result<i64> {
        self.conn.execute(
            r#"
            INSERT OR IGNORE INTO slices (
                original_audio_file_name, title, transcribed, audio_file_size, audio_file_type,
                estimated_time_to_transcribe, audio_time_length_seconds, transcription, transcription_time_taken,
                transcription_word_count, transcription_model, recording_date
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
            "#,
            params![
                slice.original_audio_file_name,
                slice.title,
                slice.transcribed as i32,
                slice.audio_file_size,
                slice.audio_file_type,
                slice.estimated_time_to_transcribe,
                slice.audio_time_length_seconds,
                slice.transcription,
                slice.transcription_time_taken,
                slice.transcription_word_count,
                slice.transcription_model,
                slice.recording_date,
            ],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn slice_exists(&self, filename: &str) -> Result<bool> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM slices WHERE original_audio_file_name = ?1",
            params![filename],
            |row| row.get(0),
        )?;
        Ok(count > 0)
    }

    // Copy ZCLOUDRECORDING table from Apple's database to CiderPress-db
    pub fn copy_zcloudrecording_table(&self, apple_db_path: &str) -> Result<u32> {
        // Attach the Apple database
        self.conn.execute(
            &format!("ATTACH DATABASE '{}' AS apple_db", apple_db_path),
            [],
        )?;

        // Copy table structure if it doesn't exist
        self.conn.execute(
            r#"
            CREATE TABLE IF NOT EXISTS ZCLOUDRECORDING AS 
            SELECT * FROM apple_db.ZCLOUDRECORDING WHERE 0
            "#,
            [],
        )?;

        // Insert new rows that don't already exist
        let rows_copied = self.conn.execute(
            r#"
            INSERT OR IGNORE INTO ZCLOUDRECORDING 
            SELECT * FROM apple_db.ZCLOUDRECORDING 
            WHERE Z_PK NOT IN (SELECT Z_PK FROM ZCLOUDRECORDING)
            "#,
            [],
        )?;

        // Detach the Apple database
        self.conn.execute("DETACH DATABASE apple_db", [])?;

        Ok(rows_copied as u32)
    }

    /// Get the recording date (as Unix timestamp) for a given filename from ZCLOUDRECORDING
    /// The ZPATH column contains the relative path including the filename
    /// Apple's ZDATE is seconds since Jan 1, 2001 - we convert to Unix timestamp
    pub fn get_recording_date_for_filename(&self, filename: &str) -> Result<Option<i64>> {
        // Apple epoch offset: seconds from Unix epoch (1970-01-01) to Apple epoch (2001-01-01)
        const APPLE_EPOCH_OFFSET: i64 = 978307200;

        // Try to find the recording with matching filename in ZPATH
        let result: Result<i64, _> = self.conn.query_row(
            r#"
            SELECT CAST(ZDATE + ? AS INTEGER) as unix_timestamp
            FROM ZCLOUDRECORDING
            WHERE ZPATH LIKE '%' || ?
            LIMIT 1
            "#,
            params![APPLE_EPOCH_OFFSET, filename],
            |row| row.get(0),
        );

        match result {
            Ok(timestamp) => Ok(Some(timestamp)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => {
                // Table might not exist yet, return None
                if e.to_string().contains("no such table") {
                    Ok(None)
                } else {
                    Err(e.into())
                }
            }
        }
    }

    pub fn list_all_slices(&self) -> Result<Vec<Slice>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, original_audio_file_name, title, transcribed, audio_file_size, audio_file_type,
                    estimated_time_to_transcribe, audio_time_length_seconds, transcription, transcription_time_taken,
                    transcription_word_count, transcription_model, recording_date
             FROM slices
             ORDER BY id"
        )?;

        let slice_iter = stmt.query_map([], |row| {
            Ok(Slice {
                id: Some(row.get("id")?),
                original_audio_file_name: row.get("original_audio_file_name")?,
                title: row.get("title")?,
                transcribed: row.get::<_, i32>("transcribed")? != 0,
                audio_file_size: row.get("audio_file_size")?,
                audio_file_type: row.get("audio_file_type")?,
                estimated_time_to_transcribe: row.get("estimated_time_to_transcribe")?,
                audio_time_length_seconds: row.get("audio_time_length_seconds")?,
                transcription: row.get("transcription")?,
                transcription_time_taken: row.get("transcription_time_taken")?,
                transcription_word_count: row.get("transcription_word_count")?,
                transcription_model: row.get("transcription_model")?,
                recording_date: row.get("recording_date")?,
            })
        })?;

        let mut slices = Vec::new();
        for slice in slice_iter {
            slices.push(slice?);
        }
        Ok(slices)
    }

    pub fn clear_all_slices(&self) -> Result<()> {
        self.conn.execute("DELETE FROM slices", [])?;
        Ok(())
    }

    pub fn update_slice_transcription(
        &self,
        slice_id: i64,
        transcription: &str,
        transcription_time_taken: i32,
        word_count: i32,
        model_name: &str,
    ) -> Result<()> {
        self.conn.execute(
            r#"
            UPDATE slices SET
                transcribed = 1,
                transcription = ?1,
                transcription_time_taken = ?2,
                transcription_word_count = ?3,
                transcription_model = ?4
            WHERE id = ?5
            "#,
            params![
                transcription,
                transcription_time_taken,
                word_count,
                model_name,
                slice_id,
            ],
        )?;
        Ok(())
    }

    pub fn update_slice_name(&self, slice_id: i64, new_name: &str) -> Result<()> {
        // Check if the new name already exists (excluding the current slice)
        let existing_count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM slices WHERE original_audio_file_name = ?1 AND id != ?2",
            params![new_name, slice_id],
            |row| row.get(0),
        )?;
        
        if existing_count > 0 {
            return Err(anyhow::anyhow!("A slice with the filename '{}' already exists", new_name));
        }
        
        // Check if the slice exists
        let slice_exists: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM slices WHERE id = ?1",
            params![slice_id],
            |row| row.get(0),
        )?;
        
        if slice_exists == 0 {
            return Err(anyhow::anyhow!("Slice with ID {} not found", slice_id));
        }
        
        // Perform the update
        let rows_affected = self.conn.execute(
            "UPDATE slices SET original_audio_file_name = ?1 WHERE id = ?2",
            params![new_name, slice_id],
        )?;
        
        if rows_affected == 0 {
            return Err(anyhow::anyhow!("Failed to update slice name: no rows affected"));
        }
        
        Ok(())
    }

    pub fn update_slice(&self, slice_id: i64, slice: &Slice) -> Result<()> {
        // Check if the new name already exists (excluding the current slice)
        let existing_count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM slices WHERE original_audio_file_name = ?1 AND id != ?2",
            params![slice.original_audio_file_name, slice_id],
            |row| row.get(0),
        )?;
        
        if existing_count > 0 {
            return Err(anyhow::anyhow!("A slice with the filename '{}' already exists", slice.original_audio_file_name));
        }
        
        // Check if the slice exists
        let slice_exists: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM slices WHERE id = ?1",
            params![slice_id],
            |row| row.get(0),
        )?;
        
        if slice_exists == 0 {
            return Err(anyhow::anyhow!("Slice with ID {} not found", slice_id));
        }
        
        // Perform the update
        let rows_affected = self.conn.execute(
            r#"
            UPDATE slices SET
                original_audio_file_name = ?1,
                title = ?2,
                transcribed = ?3,
                audio_file_size = ?4,
                audio_file_type = ?5,
                estimated_time_to_transcribe = ?6,
                audio_time_length_seconds = ?7,
                transcription = ?8,
                transcription_time_taken = ?9,
                transcription_word_count = ?10,
                transcription_model = ?11,
                recording_date = ?12
            WHERE id = ?13
            "#,
            params![
                slice.original_audio_file_name,
                slice.title,
                slice.transcribed as i32,
                slice.audio_file_size,
                slice.audio_file_type,
                slice.estimated_time_to_transcribe,
                slice.audio_time_length_seconds,
                slice.transcription,
                slice.transcription_time_taken,
                slice.transcription_word_count,
                slice.transcription_model,
                slice.recording_date,
                slice_id,
            ],
        )?;
        
        if rows_affected == 0 {
            return Err(anyhow::anyhow!("Failed to update slice: no rows affected"));
        }
        
        Ok(())
    }

    pub fn update_slice_audio_duration(&self, slice_id: i64, duration_seconds: f64) -> Result<()> {
        let rows_affected = self.conn.execute(
            "UPDATE slices SET audio_time_length_seconds = ?1 WHERE id = ?2",
            params![duration_seconds, slice_id],
        )?;

        if rows_affected == 0 {
            return Err(anyhow::anyhow!("Slice with ID {} not found", slice_id));
        }

        Ok(())
    }

    /// Clear audio durations that are obviously corrupted (> 24 hours).
    /// These were caused by a unit conversion bug in get_audio_duration.
    pub fn clear_corrupt_audio_durations(&self) -> Result<u32> {
        let rows_affected = self.conn.execute(
            "UPDATE slices SET audio_time_length_seconds = NULL WHERE audio_time_length_seconds > 86400",
            [],
        )?;
        Ok(rows_affected as u32)
    }

    pub fn get_slices_without_duration(&self) -> Result<Vec<Slice>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, original_audio_file_name, title, transcribed, audio_file_size, audio_file_type,
                    estimated_time_to_transcribe, audio_time_length_seconds, transcription, transcription_time_taken,
                    transcription_word_count, transcription_model, recording_date
             FROM slices
             WHERE audio_time_length_seconds IS NULL
             ORDER BY id"
        )?;

        let slice_iter = stmt.query_map([], |row| {
            Ok(Slice {
                id: Some(row.get("id")?),
                original_audio_file_name: row.get("original_audio_file_name")?,
                title: row.get("title")?,
                transcribed: row.get::<_, i32>("transcribed")? != 0,
                audio_file_size: row.get("audio_file_size")?,
                audio_file_type: row.get("audio_file_type")?,
                estimated_time_to_transcribe: row.get("estimated_time_to_transcribe")?,
                audio_time_length_seconds: row.get("audio_time_length_seconds")?,
                transcription: row.get("transcription")?,
                transcription_time_taken: row.get("transcription_time_taken")?,
                transcription_word_count: row.get("transcription_word_count")?,
                transcription_model: row.get("transcription_model")?,
                recording_date: row.get("recording_date")?,
            })
        })?;

        let mut slices = Vec::new();
        for slice in slice_iter {
            slices.push(slice?);
        }
        Ok(slices)
    }

    /// Backfill recording_date for slices that have NULL recording_date
    /// by looking up the date from ZCLOUDRECORDING table
    pub fn backfill_recording_dates(&self) -> Result<u32> {
        // Get all slices with NULL recording_date
        let mut stmt = self.conn.prepare(
            "SELECT id, original_audio_file_name FROM slices WHERE recording_date IS NULL"
        )?;

        let slices_to_update: Vec<(i64, String)> = stmt.query_map([], |row| {
            Ok((row.get(0)?, row.get(1)?))
        })?.filter_map(|r| r.ok()).collect();

        let mut updated_count = 0u32;

        for (slice_id, filename) in slices_to_update {
            if let Ok(Some(recording_date)) = self.get_recording_date_for_filename(&filename) {
                let rows_affected = self.conn.execute(
                    "UPDATE slices SET recording_date = ?1 WHERE id = ?2",
                    params![recording_date, slice_id],
                )?;
                if rows_affected > 0 {
                    updated_count += 1;
                }
            }
        }

        Ok(updated_count)
    }

    fn get_count_by_year_from_apple_db(&self) -> Result<Vec<YearCount>> {
        // Try to get year data from the ZCLOUDRECORDING table if it exists
        let mut stmt = self.conn.prepare(
            r#"
            SELECT
                CAST(strftime('%Y', datetime(ZDATE + 978307200, 'unixepoch')) AS INTEGER) as year,
                COUNT(*) as count
            FROM ZCLOUDRECORDING
            WHERE ZDATE IS NOT NULL
            GROUP BY year
            ORDER BY year
            "#
        )?;

        let rows = stmt.query_map([], |row| {
            Ok(YearCount {
                year: row.get(0)?,
                count: row.get(1)?,
            })
        })?;

        let mut count_by_year = Vec::new();
        for row in rows {
            count_by_year.push(row?);
        }

        Ok(count_by_year)
    }

    fn get_count_by_audio_length(&self) -> Result<Vec<AudioLengthBucket>> {
        // Group audio files by duration buckets using the audio_time_length_seconds field
        let mut stmt = self.conn.prepare(
            r#"
            SELECT
                CASE
                    WHEN audio_time_length_seconds IS NULL THEN 'Unknown'
                    WHEN audio_time_length_seconds < 30 THEN '< 30s'
                    WHEN audio_time_length_seconds < 60 THEN '30s-1m'
                    WHEN audio_time_length_seconds < 300 THEN '1-5m'
                    WHEN audio_time_length_seconds < 900 THEN '5-15m'
                    WHEN audio_time_length_seconds < 1800 THEN '15-30m'
                    WHEN audio_time_length_seconds < 3600 THEN '30m-1h'
                    ELSE '1h+'
                END as bucket,
                COUNT(*) as count
            FROM slices
            GROUP BY bucket
            ORDER BY
                CASE bucket
                    WHEN '< 30s' THEN 1
                    WHEN '30s-1m' THEN 2
                    WHEN '1-5m' THEN 3
                    WHEN '5-15m' THEN 4
                    WHEN '15-30m' THEN 5
                    WHEN '30m-1h' THEN 6
                    WHEN '1h+' THEN 7
                    ELSE 8
                END
            "#
        )?;

        let rows = stmt.query_map([], |row| {
            Ok(AudioLengthBucket {
                label: row.get(0)?,
                count: row.get(1)?,
            })
        })?;

        let mut buckets = Vec::new();
        for row in rows {
            buckets.push(row?);
        }

        Ok(buckets)
    }

    pub fn update_recording_title_by_slice(&self, slice_id: i64, new_title: &str) -> Result<()> {
        // Update the title directly in the slices table
        let rows_affected = self.conn.execute(
            "UPDATE slices SET title = ?1 WHERE id = ?2",
            params![new_title, slice_id],
        )?;

        if rows_affected == 0 {
            return Err(anyhow::anyhow!(
                "No slice found with ID: {}",
                slice_id
            ));
        }

        Ok(())
    }

    pub fn auto_populate_titles(&self) -> Result<u32> {
        use std::collections::HashMap;
        use regex::Regex;

        // Get all slices with their current titles
        let slices = self.list_all_slices()?;

        // Track titles to handle duplicates
        let mut title_counts: HashMap<String, u32> = HashMap::new();
        let mut updated_count = 0u32;

        // Regex to extract dates like "20251117" from filenames
        let date_pattern = Regex::new(r"(\d{8})").unwrap();

        for slice in slices {
            // Skip if title is already set
            if slice.title.is_some() && !slice.title.as_ref().unwrap().trim().is_empty() {
                // Count existing titles for deduplication
                let title = slice.title.as_ref().unwrap().clone();
                *title_counts.entry(title).or_insert(0) += 1;
                continue;
            }

            // Extract title from filename
            let filename = &slice.original_audio_file_name;

            // Try to extract date from filename
            let mut title = if let Some(captures) = date_pattern.captures(filename) {
                if let Some(date_str) = captures.get(1) {
                    let date = date_str.as_str();
                    if date.len() == 8 {
                        // Format as YYYY-MM-DD
                        format!("{}-{}-{}", &date[0..4], &date[4..6], &date[6..8])
                    } else {
                        // Fallback to filename without extension
                        filename.trim_end_matches(".m4a")
                            .trim_end_matches(".wav")
                            .trim_end_matches(".mp3")
                            .to_string()
                    }
                } else {
                    filename.trim_end_matches(".m4a")
                        .trim_end_matches(".wav")
                        .trim_end_matches(".mp3")
                        .to_string()
                }
            } else {
                // No date found, use filename without extension
                filename.trim_end_matches(".m4a")
                    .trim_end_matches(".wav")
                    .trim_end_matches(".mp3")
                    .to_string()
            };

            // Handle duplicates by appending (2), (3), etc.
            let base_title = title.clone();
            let mut counter = 2;
            while title_counts.contains_key(&title) {
                title = format!("{} ({})", base_title, counter);
                counter += 1;
            }

            // Mark this title as used
            *title_counts.entry(title.clone()).or_insert(0) += 1;

            // Update the slice title directly if we have a slice ID
            if let Some(slice_id) = slice.id {
                let rows_affected = self.conn.execute(
                    "UPDATE slices SET title = ?1 WHERE id = ?2",
                    params![&title, slice_id],
                )?;

                if rows_affected > 0 {
                    updated_count += 1;
                } else {
                    tracing::warn!(
                        "Failed to auto-populate title for slice {}: no rows affected",
                        slice_id
                    );
                }
            }
        }

        Ok(updated_count)
    }

    // ==================== Label CRUD operations ====================

    pub fn list_labels(&self) -> Result<Vec<Label>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name, color, keywords FROM labels ORDER BY id"
        )?;

        let label_iter = stmt.query_map([], |row| {
            Ok(Label {
                id: Some(row.get("id")?),
                name: row.get("name")?,
                color: row.get("color")?,
                keywords: row.get("keywords")?,
            })
        })?;

        let mut labels = Vec::new();
        for label in label_iter {
            labels.push(label?);
        }
        Ok(labels)
    }

    pub fn create_label(&self, label: &Label) -> Result<i64> {
        self.conn.execute(
            "INSERT INTO labels (name, color, keywords) VALUES (?1, ?2, ?3)",
            params![&label.name, &label.color, &label.keywords],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn update_label(&self, id: i64, label: &Label) -> Result<()> {
        let rows_affected = self.conn.execute(
            "UPDATE labels SET name = ?1, color = ?2, keywords = ?3 WHERE id = ?4",
            params![&label.name, &label.color, &label.keywords, id],
        )?;

        if rows_affected == 0 {
            return Err(anyhow::anyhow!("No label found with ID: {}", id));
        }
        Ok(())
    }

    pub fn delete_label(&self, id: i64) -> Result<()> {
        let rows_affected = self.conn.execute(
            "DELETE FROM labels WHERE id = ?1",
            params![id],
        )?;

        if rows_affected == 0 {
            return Err(anyhow::anyhow!("No label found with ID: {}", id));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_test_database() -> (Database, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let db = Database::new(&db_path).unwrap();
        (db, temp_dir)
    }

    fn create_test_slice(name: &str) -> Slice {
        Slice {
            id: None,
            original_audio_file_name: name.to_string(),
            title: None,
            transcribed: false,
            audio_file_size: 1024,
            audio_file_type: "m4a".to_string(),
            estimated_time_to_transcribe: 30,
            audio_time_length_seconds: None,
            transcription: None,
            transcription_time_taken: None,
            transcription_word_count: None,
        }
    }

    #[test]
    fn test_update_slice_name_success() {
        let (db, _temp_dir) = create_test_database();
        
        // Insert a test slice
        let slice = create_test_slice("original_name.m4a");
        let slice_id = db.insert_slice(&slice).unwrap();
        
        // Update the slice name
        let new_name = "updated_name.m4a";
        let result = db.update_slice_name(slice_id, new_name);
        
        assert!(result.is_ok(), "Should successfully update slice name");
        
        // Verify the name was updated
        let slices = db.list_all_slices().unwrap();
        let updated_slice = slices.iter().find(|s| s.id == Some(slice_id)).unwrap();
        assert_eq!(updated_slice.original_audio_file_name, new_name);
    }

    #[test]
    fn test_update_slice_name_duplicate_filename() {
        let (db, _temp_dir) = create_test_database();
        
        // Insert two test slices
        let slice1 = create_test_slice("slice1.m4a");
        let slice2 = create_test_slice("slice2.m4a");
        
        let slice1_id = db.insert_slice(&slice1).unwrap();
        let _slice2_id = db.insert_slice(&slice2).unwrap();
        
        // Try to update slice1 to have the same name as slice2
        let result = db.update_slice_name(slice1_id, "slice2.m4a");
        
        assert!(result.is_err(), "Should fail when trying to use duplicate filename");
        let error_message = result.unwrap_err().to_string();
        assert!(error_message.contains("already exists"), "Error should mention that filename already exists");
    }

    #[test]
    fn test_update_slice_name_nonexistent_slice() {
        let (db, _temp_dir) = create_test_database();
        
        // Try to update a slice that doesn't exist
        let result = db.update_slice_name(999, "new_name.m4a");
        
        assert!(result.is_err(), "Should fail when slice doesn't exist");
        let error_message = result.unwrap_err().to_string();
        assert!(error_message.contains("not found"), "Error should mention that slice was not found");
    }

    #[test]
    fn test_update_slice_name_same_name_same_slice() {
        let (db, _temp_dir) = create_test_database();
        
        // Insert a test slice
        let slice = create_test_slice("test_name.m4a");
        let slice_id = db.insert_slice(&slice).unwrap();
        
        // Update the slice to have the same name (should be allowed)
        let result = db.update_slice_name(slice_id, "test_name.m4a");
        
        assert!(result.is_ok(), "Should allow updating slice to its current name");
    }

    #[test]
    fn test_update_slice_name_preserves_other_fields() {
        let (db, _temp_dir) = create_test_database();
        
        // Insert a test slice with transcription data
        let mut slice = create_test_slice("original_name.m4a");
        slice.transcribed = true;
        slice.transcription = Some("Test transcription".to_string());
        slice.transcription_time_taken = Some(60);
        slice.transcription_word_count = Some(10);
        
        let slice_id = db.insert_slice(&slice).unwrap();
        
        // Update the slice name
        let new_name = "updated_name.m4a";
        let result = db.update_slice_name(slice_id, new_name);
        
        assert!(result.is_ok(), "Should successfully update slice name");
        
        // Verify the name was updated but other fields preserved
        let slices = db.list_all_slices().unwrap();
        let updated_slice = slices.iter().find(|s| s.id == Some(slice_id)).unwrap();
        
        assert_eq!(updated_slice.original_audio_file_name, new_name);
        assert_eq!(updated_slice.transcribed, true);
        assert_eq!(updated_slice.transcription, Some("Test transcription".to_string()));
        assert_eq!(updated_slice.transcription_time_taken, Some(60));
        assert_eq!(updated_slice.transcription_word_count, Some(10));
        assert_eq!(updated_slice.audio_file_size, 1024);
        assert_eq!(updated_slice.audio_file_type, "m4a");
    }

    #[test]
    fn test_update_slice_success() {
        let (db, _temp_dir) = create_test_database();
        
        // Insert a test slice
        let slice = create_test_slice("original_name.m4a");
        let slice_id = db.insert_slice(&slice).unwrap();
        
        // Create updated slice data
        let mut updated_slice = slice.clone();
        updated_slice.original_audio_file_name = "updated_name.m4a".to_string();
        updated_slice.transcription = Some("Updated transcription text".to_string());
        updated_slice.transcribed = true;
        updated_slice.transcription_word_count = Some(3);
        updated_slice.transcription_time_taken = Some(45);
        
        // Update the slice
        let result = db.update_slice(slice_id, &updated_slice);
        
        assert!(result.is_ok(), "Should successfully update slice");
        
        // Verify all fields were updated
        let slices = db.list_all_slices().unwrap();
        let updated = slices.iter().find(|s| s.id == Some(slice_id)).unwrap();
        
        assert_eq!(updated.original_audio_file_name, "updated_name.m4a");
        assert_eq!(updated.transcription, Some("Updated transcription text".to_string()));
        assert_eq!(updated.transcribed, true);
        assert_eq!(updated.transcription_word_count, Some(3));
        assert_eq!(updated.transcription_time_taken, Some(45));
        // Other fields should remain the same
        assert_eq!(updated.audio_file_size, 1024);
        assert_eq!(updated.audio_file_type, "m4a");
        assert_eq!(updated.estimated_time_to_transcribe, 30);
    }

    #[test]
    fn test_update_slice_duplicate_filename() {
        let (db, _temp_dir) = create_test_database();
        
        // Insert two test slices
        let slice1 = create_test_slice("slice1.m4a");
        let slice2 = create_test_slice("slice2.m4a");
        
        let slice1_id = db.insert_slice(&slice1).unwrap();
        let _slice2_id = db.insert_slice(&slice2).unwrap();
        
        // Try to update slice1 to have the same name as slice2
        let mut updated_slice = slice1.clone();
        updated_slice.original_audio_file_name = "slice2.m4a".to_string();
        
        let result = db.update_slice(slice1_id, &updated_slice);
        
        assert!(result.is_err(), "Should fail when trying to use duplicate filename");
        let error_message = result.unwrap_err().to_string();
        assert!(error_message.contains("already exists"), "Error should mention that filename already exists");
    }

    #[test]
    fn test_update_slice_nonexistent_slice() {
        let (db, _temp_dir) = create_test_database();
        
        // Try to update a slice that doesn't exist
        let slice = create_test_slice("new_name.m4a");
        let result = db.update_slice(999, &slice);
        
        assert!(result.is_err(), "Should fail when slice doesn't exist");
        let error_message = result.unwrap_err().to_string();
        assert!(error_message.contains("not found"), "Error should mention that slice was not found");
    }

    #[test]
    fn test_update_slice_transcription_only() {
        let (db, _temp_dir) = create_test_database();
        
        // Insert a test slice
        let slice = create_test_slice("test_slice.m4a");
        let slice_id = db.insert_slice(&slice).unwrap();
        
        // Update only the transcription fields
        let mut updated_slice = slice.clone();
        updated_slice.transcription = Some("New transcription content".to_string());
        updated_slice.transcribed = true;
        updated_slice.transcription_word_count = Some(3);
        updated_slice.transcription_time_taken = Some(60);
        
        let result = db.update_slice(slice_id, &updated_slice);
        
        assert!(result.is_ok(), "Should successfully update transcription");
        
        // Verify transcription fields were updated, others preserved
        let slices = db.list_all_slices().unwrap();
        let updated = slices.iter().find(|s| s.id == Some(slice_id)).unwrap();
        
        assert_eq!(updated.transcription, Some("New transcription content".to_string()));
        assert_eq!(updated.transcribed, true);
        assert_eq!(updated.transcription_word_count, Some(3));
        assert_eq!(updated.transcription_time_taken, Some(60));
        assert_eq!(updated.original_audio_file_name, "test_slice.m4a"); // Should remain unchanged
    }
} 