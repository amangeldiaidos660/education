ALTER TABLE sozbricks.profiles DROP CONSTRAINT profiles_user_id_fkey;
ALTER TABLE sozbricks.profiles
    ADD COLUMN auth_user_id uuid UNIQUE REFERENCES auth.users(id) ON DELETE SET NULL,
    ADD COLUMN google_subject text UNIQUE CHECK (char_length(google_subject) BETWEEN 1 AND 255);
UPDATE sozbricks.profiles SET auth_user_id=user_id;
UPDATE sozbricks.profiles p SET google_subject=i.identity_data->>'sub'
FROM auth.identities i
WHERE i.user_id=p.auth_user_id AND i.provider='google' AND nullif(i.identity_data->>'sub','') IS NOT NULL;

REVOKE INSERT, UPDATE ON sozbricks.profiles FROM sozbricks_runtime;
GRANT UPDATE(display_name,hero,updated_at) ON sozbricks.profiles TO sozbricks_runtime;

ALTER TABLE sozbricks.wallet_bindings ADD COLUMN reconfirmation_required boolean NOT NULL DEFAULT false;

CREATE FUNCTION sozbricks.require_wallet_reconfirmation() RETURNS trigger
LANGUAGE plpgsql SECURITY DEFINER SET search_path=pg_catalog AS $$
BEGIN
    IF OLD.auth_user_id IS DISTINCT FROM NEW.auth_user_id THEN
        UPDATE sozbricks.wallet_bindings SET reconfirmation_required=true WHERE user_id=NEW.user_id;
        DELETE FROM sozbricks.wallet_challenges WHERE user_id=NEW.user_id;
    END IF;
    RETURN NEW;
END
$$;
REVOKE ALL ON FUNCTION sozbricks.require_wallet_reconfirmation() FROM PUBLIC;
CREATE TRIGGER account_auth_changed AFTER UPDATE OF auth_user_id ON sozbricks.profiles
FOR EACH ROW EXECUTE FUNCTION sozbricks.require_wallet_reconfirmation();

CREATE FUNCTION sozbricks.resolve_player(auth_id uuid, subject text, player_name text) RETURNS uuid
LANGUAGE plpgsql SECURITY DEFINER SET search_path=pg_catalog AS $$
DECLARE player uuid; previous_auth uuid;
BEGIN
    IF subject IS NULL OR char_length(subject) NOT BETWEEN 1 AND 255 OR NOT EXISTS (
        SELECT 1 FROM auth.identities i JOIN auth.users u ON u.id=i.user_id
        WHERE i.user_id=auth_id AND i.provider='google' AND i.identity_data->>'sub'=subject
    ) THEN
        RAISE EXCEPTION 'Verified Google identity is missing' USING ERRCODE='23503';
    END IF;
    PERFORM pg_advisory_xact_lock(hashtextextended(subject,0));
    SELECT user_id,auth_user_id INTO player,previous_auth FROM sozbricks.profiles WHERE google_subject=subject FOR UPDATE;
    IF player IS NOT NULL AND previous_auth IS NOT NULL AND previous_auth<>auth_id THEN
        RAISE EXCEPTION 'Google identity is already linked' USING ERRCODE='23505';
    END IF;
    IF player IS NULL THEN
        SELECT user_id INTO player FROM sozbricks.profiles WHERE auth_user_id=auth_id AND (google_subject IS NULL OR google_subject=subject) FOR UPDATE;
    END IF;
    IF player IS NULL THEN
        INSERT INTO sozbricks.profiles(user_id,auth_user_id,google_subject,display_name)
        VALUES(auth_id,auth_id,subject,player_name) RETURNING user_id INTO player;
    ELSE
        UPDATE sozbricks.profiles SET auth_user_id=auth_id,google_subject=subject
        WHERE user_id=player AND (auth_user_id IS DISTINCT FROM auth_id OR google_subject IS DISTINCT FROM subject);
    END IF;
    RETURN player;
END
$$;
REVOKE ALL ON FUNCTION sozbricks.resolve_player(uuid,text,text) FROM PUBLIC;

GRANT SELECT(user_id,auth_user_id) ON sozbricks.profiles TO sozbricks_payout_worker;
CREATE POLICY worker_profiles ON sozbricks.profiles FOR SELECT TO sozbricks_payout_worker USING(true);
