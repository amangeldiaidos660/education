import assert from 'node:assert/strict';
import test from 'node:test';
import { readFile, readdir } from 'node:fs/promises';
import { randomUUID } from 'node:crypto';
import { PGlite } from '@electric-sql/pglite';

const root = new URL('../../', import.meta.url);
const migrationDir = new URL('migrations/', root);
const worker = await readFile(new URL('src/payouts.rs', root), 'utf8');
const admin = await readFile(new URL('src/admin.rs', root), 'utf8');
const adminQueries = [...admin.matchAll(/sqlx::query_scalar\("([^"\n]+)"\)/g)].map(m => m[1]);
const claim = [...worker.matchAll(/sqlx::query\("([^"\n]+)"\)/g)].map(m => m[1]).find(sql => sql.startsWith('WITH candidate'));

test('deleted Google login restores the same player, wallet and paid rewards without granting access by email', async () => {
  const db = new PGlite();
  try {
    await db.exec('CREATE SCHEMA auth; CREATE TABLE auth.users(id uuid PRIMARY KEY,email text); CREATE TABLE auth.identities(user_id uuid REFERENCES auth.users(id) ON DELETE CASCADE,provider text,identity_data jsonb);');
    const files = (await readdir(migrationDir)).filter(f => f.endsWith('.sql')).sort();
    for (const file of files.filter(f => f < '202611220006')) await db.exec(await readFile(new URL(file, migrationDir), 'utf8'));
    const original = randomUUID(), restored = randomUUID(), outsider = randomUUID();
    const wallet = 'HjTpY66f5KqNYAPZ4Hsz9EBg4nRS6M3d8omEZTYEbYoY';
    async function login(id, subject) {
      await db.query("INSERT INTO auth.users VALUES($1,'same@example.com')", [id]);
      await db.query("INSERT INTO auth.identities VALUES($1,'google',jsonb_build_object('sub',$2::text))", [id,subject]);
    }
    await login(original, 'original-google-sub');
    await db.query("INSERT INTO sozbricks.profiles(user_id,display_name,hero) VALUES($1,'Original','aiaru')", [original]);
    await db.query('INSERT INTO sozbricks.wallet_bindings(user_id,address) VALUES($1,$2)', [original,wallet]);
    const rewards=[];
    for (const level of [1,2]) {
      const session=randomUUID();
      await db.query("INSERT INTO sozbricks.game_sessions(id,user_id,level_id,rules_version,seed,status,expires_at,finished_at) VALUES($1,$2,$3,1,17,'completed',now()+interval '1 hour',now())", [session,original,level]);
      await db.query("INSERT INTO sozbricks.game_results(session_id,user_id,level_id,won,collected_word,score,duration_ticks) VALUES($1,$2,$3,true,'АТ',100,120)", [session,original,level]);
      await db.query('INSERT INTO sozbricks.level_progress(user_id,level_id,completed_at,first_win_session_id) VALUES($1,$2,now(),$3)', [original,level,session]);
      const row=await db.query('INSERT INTO sozbricks.reward_ledger(user_id,level_id,session_id,amount_lamports,payable) VALUES($1,$2,$3,100000000,true) RETURNING id', [original,level,session]);
      rewards.push(row.rows[0].id);
    }
    await db.query("UPDATE sozbricks.reward_payouts SET status='paid',recipient=$2,signature='paid-proof',signed_transaction='wire',last_valid_height=100,paid_at=now() WHERE reward_id=$1", [rewards[0],wallet]);
    await db.query("INSERT INTO sozbricks.payout_attempts(signature,reward_id,signed_transaction,last_valid_height) VALUES('paid-proof',$1,'wire',100)", [rewards[0]]);
    for (const file of files.filter(f => f >= '202611220006')) await db.exec(await readFile(new URL(file, migrationDir), 'utf8'));
    assert.equal((await db.query('SELECT google_subject FROM sozbricks.profiles')).rows[0].google_subject,'original-google-sub');
    await db.query('DELETE FROM auth.users WHERE id=$1',[original]);
    assert.equal((await db.query('SELECT auth_user_id FROM sozbricks.profiles')).rows[0].auth_user_id,null);
    assert.equal((await db.query('SELECT reconfirmation_required FROM sozbricks.wallet_bindings')).rows[0].reconfirmation_required,true);
    assert.equal((await db.query('SELECT * FROM sozbricks.level_progress')).rows.length,2);
    assert.equal((await db.query("SELECT * FROM sozbricks.reward_payouts WHERE status='paid'")).rows.length,1);
    assert.equal((await db.query('SELECT * FROM sozbricks.payout_attempts')).rows.length,1);
    await login(restored,'original-google-sub');
    const resolved=(await db.query('SELECT sozbricks.resolve_player($1,$2,$3) AS id',[restored,'original-google-sub','New name'])).rows[0].id;
    assert.equal(resolved,original);
    assert.equal((await db.query('SELECT display_name FROM sozbricks.profiles')).rows[0].display_name,'Original');
    assert.equal((await db.query('SELECT address FROM sozbricks.wallet_bindings')).rows[0].address,wallet);
    await db.exec('SET ROLE sozbricks_payout_worker');
    assert.equal((await db.query(claim,[randomUUID()])).rows.length,0);
    await db.exec('RESET ROLE');
    await login(outsider,'different-google-sub');
    await assert.rejects(db.query('SELECT sozbricks.resolve_player($1,$2,$3)',[outsider,'original-google-sub','Steal']));
    const different=(await db.query('SELECT sozbricks.resolve_player($1,$2,$3) AS id',[outsider,'different-google-sub','Other'])).rows[0].id;
    assert.notEqual(different,original);
    await db.exec('SET ROLE sozbricks_runtime');
    await db.query("SELECT set_config('app.user_id',$1,false)",[different]);
    assert.equal((await db.query('SELECT * FROM sozbricks.wallet_bindings')).rows.length,0);
    assert.equal((await db.query('SELECT * FROM sozbricks.reward_ledger')).rows.length,0);
    await assert.rejects(db.query('SELECT sozbricks.resolve_player($1,$2,$3)',[restored,'original-google-sub','Steal']));
    await db.query("SELECT set_config('app.user_id',$1,false)",[original]);
    await assert.rejects(db.query('UPDATE sozbricks.profiles SET google_subject=$1',['forged']));
    await assert.rejects(db.exec('DELETE FROM sozbricks.profiles'));
    await db.exec('RESET ROLE');
    const newSession=randomUUID();
    await db.query("INSERT INTO sozbricks.game_sessions(id,user_id,level_id,rules_version,seed,status,expires_at,finished_at) VALUES($1,$2,1,1,17,'completed',now()+interval '1 hour',now())",[newSession,original]);
    await db.query("INSERT INTO sozbricks.game_results(session_id,user_id,level_id,won,collected_word,score,duration_ticks) VALUES($1,$2,1,true,'АТ',100,120)",[newSession,original]);
    await assert.rejects(db.query('INSERT INTO sozbricks.reward_ledger(user_id,level_id,session_id,amount_lamports,payable) VALUES($1,1,$2,100000000,true)',[original,newSession]));
    await db.query('UPDATE sozbricks.wallet_bindings SET reconfirmation_required=false WHERE user_id=$1',[original]);
    const accountQuery=adminQueries.find(q => q.includes("'player_id',p.user_id"));
    const accounts=(await db.query(accountQuery,[25,0])).rows.map(row => Object.values(row)[0]);
    assert.equal(accounts.length,2);
    assert.equal(accounts.find(row => row.player_id===original).paid_lamports,100000000);
    assert.equal(accounts.find(row => row.player_id===original).pending_lamports,100000000);
    assert.equal(accounts.find(row => row.player_id===original).recovery_ready,true);
    assert.ok(accounts.every(row => !('google_subject' in row) && !('auth_user_id' in row)));
    const rewardQuery=adminQueries.find(q => q.includes("'reward_id',r.id"));
    assert.equal((await db.query(rewardQuery,[outsider,25,0])).rows.length,0);
    assert.equal((await db.query(rewardQuery,[original,25,0])).rows.length,2);
    const summaryQuery=adminQueries.find(q => q.includes("'accounts',(SELECT"));
    const summary=Object.values((await db.query(summaryQuery)).rows[0])[0];
    assert.equal(summary.accounts,2);
    assert.equal(summary.paid_lamports,100000000);
    await db.exec('SET ROLE sozbricks_payout_worker');
    const pending=(await db.query(claim,[randomUUID()])).rows;
    assert.equal(pending.length,1);
    assert.equal(pending[0].reward_id,rewards[1]);
    assert.equal(pending[0].recipient,wallet);
  } finally { await db.close(); }
});
