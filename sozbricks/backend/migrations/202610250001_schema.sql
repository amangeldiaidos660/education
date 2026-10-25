CREATE SCHEMA sozbricks;
REVOKE ALL ON SCHEMA sozbricks FROM PUBLIC;

CREATE TABLE sozbricks.profiles (
    user_id uuid PRIMARY KEY REFERENCES auth.users(id) ON DELETE CASCADE,
    display_name text NOT NULL CHECK (char_length(btrim(display_name)) BETWEEN 1 AND 60),
    hero text NOT NULL DEFAULT 'alikhan' CHECK (hero IN ('alikhan', 'aiaru')),
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE sozbricks.levels (
    id smallint PRIMARY KEY CHECK (id > 0),
    word text NOT NULL CHECK (char_length(word) BETWEEN 1 AND 32 AND word = normalize(word, NFC)),
    translation text NOT NULL CHECK (char_length(btrim(translation)) BETWEEN 1 AND 160),
    letter_count smallint GENERATED ALWAYS AS (char_length(word)) STORED,
    reward_lamports bigint NOT NULL CHECK (reward_lamports BETWEEN 0 AND 1000000000),
    speed integer NOT NULL CHECK (speed BETWEEN 1 AND 600),
    rules_version integer NOT NULL DEFAULT 1 CHECK (rules_version > 0),
    active boolean NOT NULL DEFAULT true
);

CREATE TABLE sozbricks.game_sessions (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id uuid NOT NULL REFERENCES sozbricks.profiles(user_id) ON DELETE CASCADE,
    level_id smallint NOT NULL REFERENCES sozbricks.levels(id),
    rules_version integer NOT NULL CHECK (rules_version > 0),
    seed bigint NOT NULL CHECK (seed BETWEEN 0 AND 4294967295),
    status text NOT NULL DEFAULT 'started' CHECK (status IN ('started', 'completed', 'expired', 'abandoned')),
    started_at timestamptz NOT NULL DEFAULT now(),
    expires_at timestamptz NOT NULL CHECK (expires_at > started_at),
    finished_at timestamptz,
    CHECK ((status = 'started' AND finished_at IS NULL) OR (status <> 'started' AND finished_at IS NOT NULL AND finished_at >= started_at)),
    UNIQUE (id, user_id, level_id)
);

CREATE TABLE sozbricks.game_results (
    session_id uuid PRIMARY KEY,
    user_id uuid NOT NULL,
    level_id smallint NOT NULL,
    won boolean NOT NULL,
    collected_word text NOT NULL CHECK (char_length(collected_word) <= 32),
    score bigint NOT NULL CHECK (score >= 0),
    duration_ticks integer NOT NULL CHECK (duration_ticks >= 0),
    recorded_at timestamptz NOT NULL DEFAULT now(),
    FOREIGN KEY (session_id, user_id, level_id) REFERENCES sozbricks.game_sessions(id, user_id, level_id) ON DELETE CASCADE,
    UNIQUE (session_id, user_id, level_id),
    UNIQUE (session_id, user_id, level_id, won)
);

CREATE TABLE sozbricks.level_progress (
    user_id uuid NOT NULL REFERENCES sozbricks.profiles(user_id) ON DELETE CASCADE,
    level_id smallint NOT NULL REFERENCES sozbricks.levels(id),
    best_score bigint NOT NULL DEFAULT 0 CHECK (best_score >= 0),
    completed_at timestamptz,
    first_win_session_id uuid,
    first_win_won boolean NOT NULL DEFAULT true CHECK (first_win_won),
    updated_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (user_id, level_id),
    CHECK ((completed_at IS NULL) = (first_win_session_id IS NULL)),
    FOREIGN KEY (first_win_session_id, user_id, level_id, first_win_won) REFERENCES sozbricks.game_results(session_id, user_id, level_id, won)
);

CREATE TABLE sozbricks.reward_ledger (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id uuid NOT NULL REFERENCES sozbricks.profiles(user_id) ON DELETE CASCADE,
    level_id smallint NOT NULL REFERENCES sozbricks.levels(id),
    session_id uuid NOT NULL UNIQUE,
    result_won boolean NOT NULL DEFAULT true CHECK (result_won),
    amount_lamports bigint NOT NULL CHECK (amount_lamports > 0 AND amount_lamports <= 1000000000),
    created_at timestamptz NOT NULL DEFAULT now(),
    UNIQUE (user_id, level_id),
    FOREIGN KEY (session_id, user_id, level_id, result_won) REFERENCES sozbricks.game_results(session_id, user_id, level_id, won)
);

CREATE INDEX game_sessions_user_started_idx ON sozbricks.game_sessions(user_id, started_at DESC, id DESC);
CREATE INDEX game_sessions_expiry_idx ON sozbricks.game_sessions(expires_at) WHERE status = 'started';
CREATE INDEX game_results_user_recorded_idx ON sozbricks.game_results(user_id, recorded_at DESC, session_id DESC);

ALTER TABLE sozbricks.profiles ENABLE ROW LEVEL SECURITY;
ALTER TABLE sozbricks.levels ENABLE ROW LEVEL SECURITY;
ALTER TABLE sozbricks.game_sessions ENABLE ROW LEVEL SECURITY;
ALTER TABLE sozbricks.game_results ENABLE ROW LEVEL SECURITY;
ALTER TABLE sozbricks.level_progress ENABLE ROW LEVEL SECURITY;
ALTER TABLE sozbricks.reward_ledger ENABLE ROW LEVEL SECURITY;
REVOKE ALL ON ALL TABLES IN SCHEMA sozbricks FROM PUBLIC;
ALTER DEFAULT PRIVILEGES IN SCHEMA sozbricks REVOKE ALL ON TABLES FROM PUBLIC;
