-- Migration to add a table for storing revoked JWT JTIs (JWT IDs)

CREATE TABLE IF NOT EXISTS revoked_tokens (
    jti TEXT PRIMARY KEY NOT NULL,  -- The unique JWT ID
    expiry INTEGER NOT NULL         -- The original expiry timestamp (Unix epoch seconds) of the token
);

-- Index to efficiently query for expired tokens during cleanup
CREATE INDEX IF NOT EXISTS idx_revoked_tokens_expiry ON revoked_tokens(expiry); 