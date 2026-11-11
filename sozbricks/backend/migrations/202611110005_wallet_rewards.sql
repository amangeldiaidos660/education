CREATE TABLE sozbricks.reward_campaign (
    id boolean PRIMARY KEY DEFAULT true CHECK (id),
    activated_at timestamptz,
    program_address text
);
INSERT INTO sozbricks.reward_campaign(id) VALUES(true);

ALTER TABLE sozbricks.reward_ledger ADD COLUMN payable boolean NOT NULL DEFAULT false;

CREATE TABLE sozbricks.wallet_bindings (
    user_id uuid PRIMARY KEY REFERENCES sozbricks.profiles(user_id),
    address text NOT NULL UNIQUE CHECK (length(address) BETWEEN 32 AND 44),
    verified_at timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE sozbricks.wallet_challenges (
    user_id uuid PRIMARY KEY REFERENCES sozbricks.profiles(user_id),
    id uuid NOT NULL UNIQUE,
    address text NOT NULL,
    message text NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    expires_at timestamptz NOT NULL DEFAULT now() + interval '5 minutes',
    consumed boolean NOT NULL DEFAULT false
);

CREATE TABLE sozbricks.reward_payouts (
    reward_id uuid PRIMARY KEY REFERENCES sozbricks.reward_ledger(id),
    user_id uuid NOT NULL REFERENCES sozbricks.profiles(user_id),
    recipient text,
    status text NOT NULL DEFAULT 'awaiting_wallet' CHECK (status IN ('awaiting_wallet','queued','submitted','paid','retry','blocked')),
    signature text,
    signed_transaction text,
    last_valid_height bigint,
    receipt_address text,
    lease_id uuid,
    lease_until timestamptz,
    next_attempt_at timestamptz NOT NULL DEFAULT now(),
    error_code text,
    paid_at timestamptz,
    updated_at timestamptz NOT NULL DEFAULT now(),
    CHECK ((signature IS NULL) = (signed_transaction IS NULL)),
    CHECK ((signature IS NULL) = (last_valid_height IS NULL))
);
CREATE INDEX payout_pending ON sozbricks.reward_payouts(next_attempt_at) WHERE status NOT IN ('paid','blocked');

CREATE TABLE sozbricks.payout_attempts (
    signature text PRIMARY KEY,
    reward_id uuid NOT NULL REFERENCES sozbricks.reward_payouts(reward_id),
    signed_transaction text NOT NULL,
    last_valid_height bigint NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now()
);

ALTER TABLE sozbricks.reward_campaign ENABLE ROW LEVEL SECURITY;
ALTER TABLE sozbricks.wallet_bindings ENABLE ROW LEVEL SECURITY;
ALTER TABLE sozbricks.wallet_challenges ENABLE ROW LEVEL SECURITY;
ALTER TABLE sozbricks.reward_payouts ENABLE ROW LEVEL SECURITY;
ALTER TABLE sozbricks.payout_attempts ENABLE ROW LEVEL SECURITY;

GRANT SELECT ON sozbricks.reward_campaign, sozbricks.reward_payouts TO sozbricks_runtime;
GRANT SELECT, INSERT, UPDATE ON sozbricks.wallet_bindings, sozbricks.wallet_challenges TO sozbricks_runtime;
CREATE POLICY campaign_read ON sozbricks.reward_campaign FOR SELECT TO sozbricks_runtime USING (true);
CREATE POLICY wallet_owner ON sozbricks.wallet_bindings TO sozbricks_runtime USING (user_id = nullif(current_setting('app.user_id',true),'')::uuid) WITH CHECK (user_id = nullif(current_setting('app.user_id',true),'')::uuid);
CREATE POLICY challenge_owner ON sozbricks.wallet_challenges TO sozbricks_runtime USING (user_id = nullif(current_setting('app.user_id',true),'')::uuid) WITH CHECK (user_id = nullif(current_setting('app.user_id',true),'')::uuid);
CREATE POLICY payout_owner ON sozbricks.reward_payouts FOR SELECT TO sozbricks_runtime USING (user_id = nullif(current_setting('app.user_id',true),'')::uuid);

CREATE FUNCTION sozbricks.enqueue_reward() RETURNS trigger LANGUAGE plpgsql SECURITY DEFINER SET search_path = pg_catalog AS $$
BEGIN
    IF NEW.payable THEN
        INSERT INTO sozbricks.reward_payouts(reward_id,user_id) VALUES(NEW.id,NEW.user_id);
    END IF;
    RETURN NEW;
END
$$;
REVOKE ALL ON FUNCTION sozbricks.enqueue_reward() FROM PUBLIC;
CREATE TRIGGER enqueue_reward AFTER INSERT ON sozbricks.reward_ledger FOR EACH ROW EXECUTE FUNCTION sozbricks.enqueue_reward();

DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname='sozbricks_payout_worker') THEN
        CREATE ROLE sozbricks_payout_worker NOLOGIN NOSUPERUSER NOCREATEDB NOCREATEROLE NOINHERIT NOBYPASSRLS;
    END IF;
    IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname='sozbricks_payout_worker' AND (rolsuper OR rolbypassrls OR rolcreatedb OR rolcreaterole)) THEN
        RAISE EXCEPTION 'sozbricks_payout_worker must be unprivileged';
    END IF;
END
$$;
GRANT sozbricks_payout_worker TO CURRENT_USER;
GRANT USAGE ON SCHEMA sozbricks TO sozbricks_payout_worker;
GRANT SELECT ON sozbricks.reward_campaign, sozbricks.reward_ledger, sozbricks.wallet_bindings TO sozbricks_payout_worker;
GRANT SELECT, UPDATE ON sozbricks.reward_payouts TO sozbricks_payout_worker;
GRANT SELECT, INSERT ON sozbricks.payout_attempts TO sozbricks_payout_worker;
CREATE POLICY worker_campaign ON sozbricks.reward_campaign FOR SELECT TO sozbricks_payout_worker USING(true);
CREATE POLICY worker_rewards ON sozbricks.reward_ledger FOR SELECT TO sozbricks_payout_worker USING(payable);
CREATE POLICY worker_wallets ON sozbricks.wallet_bindings FOR SELECT TO sozbricks_payout_worker USING(true);
CREATE POLICY worker_payouts ON sozbricks.reward_payouts TO sozbricks_payout_worker USING(true) WITH CHECK(true);
CREATE POLICY worker_attempts ON sozbricks.payout_attempts TO sozbricks_payout_worker USING(true) WITH CHECK(true);
