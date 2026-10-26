DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'sozbricks_runtime') THEN
        CREATE ROLE sozbricks_runtime NOLOGIN NOSUPERUSER NOCREATEDB NOCREATEROLE NOINHERIT NOBYPASSRLS;
    END IF;
    IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'sozbricks_runtime'
        AND (rolsuper OR rolbypassrls OR rolcreatedb OR rolcreaterole)) THEN
        RAISE EXCEPTION 'sozbricks_runtime must be an unprivileged role';
    END IF;
END
$$;

GRANT sozbricks_runtime TO CURRENT_USER;
GRANT USAGE ON SCHEMA sozbricks TO sozbricks_runtime;
GRANT SELECT ON sozbricks.levels TO sozbricks_runtime;
CREATE POLICY runtime_levels ON sozbricks.levels FOR SELECT TO sozbricks_runtime USING (true);

GRANT SELECT, INSERT ON sozbricks.profiles, sozbricks.game_sessions, sozbricks.game_results,
    sozbricks.level_progress, sozbricks.reward_ledger, sozbricks.guest_imports, sozbricks.guest_progress TO sozbricks_runtime;
GRANT UPDATE ON sozbricks.profiles, sozbricks.game_sessions, sozbricks.level_progress TO sozbricks_runtime;

DO $$
DECLARE table_name text;
BEGIN
    FOREACH table_name IN ARRAY ARRAY['profiles','game_sessions','game_results','level_progress','reward_ledger','guest_imports','guest_progress'] LOOP
        EXECUTE format('CREATE POLICY runtime_owner ON sozbricks.%I TO sozbricks_runtime USING (user_id = nullif(current_setting(''app.user_id'', true), '''')::uuid) WITH CHECK (user_id = nullif(current_setting(''app.user_id'', true), '''')::uuid)', table_name);
    END LOOP;
END
$$;
