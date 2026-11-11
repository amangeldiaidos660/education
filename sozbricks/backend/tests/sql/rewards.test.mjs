import assert from 'node:assert/strict';
import test from 'node:test';
import { readFile, readdir } from 'node:fs/promises';
import { randomUUID } from 'node:crypto';
import { PGlite } from '@electric-sql/pglite';

const root = new URL('../../', import.meta.url);
const migrationDir = new URL('migrations/', root);
const game = await readFile(new URL('src/game_api.rs', root), 'utf8');
const payouts = await readFile(new URL('src/payouts.rs', root), 'utf8');
const queries = text => [...text.matchAll(/sqlx::query(?:_scalar)?\("([^"\n]+)"\)/g)].map(match => match[1]);
const eligibility = queries(game).find(sql => sql.startsWith('SELECT EXISTS(SELECT 1 FROM sozbricks.reward_campaign c'));
const claim = queries(payouts).find(sql => sql.startsWith('WITH candidate'));
const persist = queries(payouts).find(sql => sql.startsWith('UPDATE sozbricks.reward_payouts SET signature='));

test('migrations preserve history; eligibility, RLS and durable payout leases', async () => {
  const db = new PGlite();
  try {
    await db.exec('CREATE SCHEMA auth; CREATE TABLE auth.users(id uuid PRIMARY KEY);');
    const files = (await readdir(migrationDir)).filter(f => f.endsWith('.sql')).sort();
    for (const file of files.slice(0, -1)) await db.exec(await readFile(new URL(file, migrationDir), 'utf8'));
    const user = randomUUID(), other = randomUUID();
    for (const id of [user, other]) {
      await db.query('INSERT INTO auth.users VALUES($1)', [id]);
      await db.query("INSERT INTO sozbricks.profiles(user_id,display_name) VALUES($1,'Test')", [id]);
    }
    async function session(level, started = 'now()') {
      const id = randomUUID();
      await db.query(`INSERT INTO sozbricks.game_sessions(id,user_id,level_id,rules_version,seed,expires_at,status,finished_at,started_at) VALUES($1,$2,$3,1,17,now()+interval '1 hour','completed',now(),${started})`, [id,user,level]);
      await db.query("INSERT INTO sozbricks.game_results(session_id,user_id,level_id,won,collected_word,score,duration_ticks) VALUES($1,$2,$3,true,'АТ',1000,4200)", [id,user,level]);
      return id;
    }
    const old = await session(1, "now()-interval '1 day'");
    await db.query('INSERT INTO sozbricks.reward_ledger(user_id,level_id,session_id,amount_lamports) VALUES($1,1,$2,100000000)', [user,old]);
    await db.exec(await readFile(new URL(files.at(-1), migrationDir), 'utf8'));
    assert.equal((await db.query('SELECT payable FROM sozbricks.reward_ledger')).rows[0].payable, false);
    assert.equal((await db.query('SELECT * FROM sozbricks.reward_payouts')).rows.length, 0);
    await db.query('INSERT INTO sozbricks.level_progress(user_id,level_id,completed_at,first_win_session_id) VALUES($1,1,now(),$2)', [user,old]);
    const fresh = await session(2);
    assert.equal(Object.values((await db.query(eligibility,[user,2,fresh])).rows[0])[0],false);
    await db.exec("UPDATE sozbricks.reward_campaign SET activated_at=now()-interval '1 minute',program_address='test' WHERE id");
    assert.equal(Object.values((await db.query(eligibility,[user,1,old])).rows[0])[0],false);
    const replay = await session(1);
    assert.equal(Object.values((await db.query(eligibility,[user,1,replay])).rows[0])[0],false);
    assert.equal(Object.values((await db.query(eligibility,[user,2,fresh])).rows[0])[0],true);
    await db.query("INSERT INTO sozbricks.guest_imports(user_id,import_id,payload_hash,payload) VALUES($1,$2,decode(repeat('00',32),'hex'),'{}')",[user,randomUUID()]);
    await db.query('INSERT INTO sozbricks.guest_progress(user_id,level_id,completed,best_score) VALUES($1,3,true,10)',[user]);
    const imported = await session(3);
    assert.equal(Object.values((await db.query(eligibility,[user,3,imported])).rows[0])[0],false);
    await db.exec('SET ROLE sozbricks_runtime');
    await db.query("SELECT set_config('app.user_id',$1,false)",[user]);
    await db.query('INSERT INTO sozbricks.reward_ledger(user_id,level_id,session_id,amount_lamports,payable) VALUES($1,2,$2,100000000,true)',[user,fresh]);
    const reward = (await db.query('SELECT reward_id FROM sozbricks.reward_payouts')).rows[0].reward_id;
    await db.query('INSERT INTO sozbricks.reward_ledger(user_id,level_id,session_id,amount_lamports,payable) VALUES($1,2,$2,100000000,true) ON CONFLICT(user_id,level_id) DO NOTHING',[user,fresh]);
    assert.equal((await db.query('SELECT * FROM sozbricks.reward_payouts')).rows.length,1);
    await assert.rejects(db.exec("UPDATE sozbricks.reward_payouts SET status='paid'"));
    await db.query("SELECT set_config('app.user_id',$1,false)",[other]);
    assert.equal((await db.query('SELECT * FROM sozbricks.reward_payouts')).rows.length,0);
    await db.exec('RESET ROLE; SET ROLE sozbricks_payout_worker');
    await assert.rejects(db.exec('SELECT * FROM auth.users'));
    assert.equal((await db.query(claim,[randomUUID()])).rows.length,0);
    await db.exec('RESET ROLE');
    await db.query('INSERT INTO sozbricks.wallet_bindings(user_id,address) VALUES($1,$2)',[user,'615vW8VZFB8fLGrPTcxQ2FWBWkevahkjX5kCbtoosyQw']);
    await db.exec('SET ROLE sozbricks_payout_worker');
    const lease = randomUUID();
    const claimed = (await db.query(claim,[lease])).rows[0];
    assert.equal(claimed.reward_id,reward);
    assert.equal(claimed.recipient,'615vW8VZFB8fLGrPTcxQ2FWBWkevahkjX5kCbtoosyQw');
    assert.equal((await db.query(claim,[randomUUID()])).rows.length,0);
    assert.equal((await db.query(persist,[reward,randomUUID(),'sig','wire',100,'receipt'])).affectedRows,0);
    assert.equal((await db.query(persist,[reward,lease,'sig','wire',100,'receipt'])).affectedRows,1);
    await db.query('INSERT INTO sozbricks.payout_attempts(signature,reward_id,signed_transaction,last_valid_height) VALUES($1,$2,$3,$4)',['sig',reward,'wire',100]);
    await db.exec('RESET ROLE');
    await db.query("UPDATE sozbricks.reward_payouts SET lease_until=now()-interval '1 second' WHERE reward_id=$1",[reward]);
    await db.query("UPDATE sozbricks.wallet_bindings SET address=$2 WHERE user_id=$1",[user,'HMg9Wz3DLAwU8GAYh91WhRMWQzdhpAsBpUYkZcf9tMtX']);
    await db.exec('SET ROLE sozbricks_payout_worker');
    const restored = (await db.query(claim,[randomUUID()])).rows[0];
    assert.equal(restored.recipient,claimed.recipient);
    assert.equal(restored.signature,'sig');
    assert.equal(restored.signed_transaction,'wire');
    assert.equal((await db.query('SELECT * FROM sozbricks.payout_attempts')).rows.length,1);
  } finally { await db.close(); }
});
