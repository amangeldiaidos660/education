CREATE TABLE sozbricks.guest_imports (
    user_id uuid PRIMARY KEY REFERENCES sozbricks.profiles(user_id) ON DELETE CASCADE,
    import_id uuid NOT NULL UNIQUE,
    payload_hash bytea NOT NULL CHECK (octet_length(payload_hash) = 32),
    payload jsonb NOT NULL,
    imported_at timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE sozbricks.guest_progress (
    user_id uuid NOT NULL REFERENCES sozbricks.guest_imports(user_id) ON DELETE CASCADE,
    level_id smallint NOT NULL REFERENCES sozbricks.levels(id),
    completed boolean NOT NULL,
    best_score bigint NOT NULL CHECK (best_score BETWEEN 0 AND 100000000),
    PRIMARY KEY (user_id, level_id)
);

ALTER TABLE sozbricks.game_sessions
    ADD COLUMN request_id uuid,
    ADD COLUMN word_snapshot text,
    ADD COLUMN speed_snapshot integer,
    ADD COLUMN reward_snapshot bigint,
    ADD CONSTRAINT session_request_unique UNIQUE (user_id, request_id),
    ADD CONSTRAINT session_snapshot_valid CHECK (
        (word_snapshot IS NULL AND speed_snapshot IS NULL AND reward_snapshot IS NULL)
        OR (word_snapshot IS NOT NULL AND speed_snapshot IS NOT NULL AND reward_snapshot IS NOT NULL
            AND speed_snapshot BETWEEN 1 AND 600 AND reward_snapshot BETWEEN 0 AND 1000000000 AND char_length(word_snapshot) BETWEEN 1 AND 32)
    );

ALTER TABLE sozbricks.game_results ADD COLUMN result_hash bytea,
    ADD CONSTRAINT result_hash_valid CHECK (result_hash IS NULL OR octet_length(result_hash) = 32);
CREATE UNIQUE INDEX sessions_one_started_per_user ON sozbricks.game_sessions(user_id) WHERE status = 'started';

ALTER TABLE sozbricks.guest_imports ENABLE ROW LEVEL SECURITY;
ALTER TABLE sozbricks.guest_progress ENABLE ROW LEVEL SECURITY;
REVOKE ALL ON sozbricks.guest_imports, sozbricks.guest_progress FROM PUBLIC;
