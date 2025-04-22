-- Add migration script here
    -- Add total_files_skipped column to compression_stats table
    ALTER TABLE compression_stats
    ADD COLUMN total_files_skipped INTEGER NOT NULL DEFAULT 0;