import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { signature } from '@solana/kit';
import { assertDevnet, assertFinal, inspect, metadata, rpc } from './shared.js';

await assertDevnet();
const result = JSON.parse(await readFile(new URL('results.json', import.meta.url), 'utf8'));
assert.equal(result.network, 'devnet');
const balances = await inspect(result.mint, result.source, result.destination, result.owner, result.recipient);
assertFinal(balances);
for (const [label, value] of Object.entries(result.transactions)) {
  const tx = await rpc.getTransaction(signature(value as string), { encoding: 'json', commitment: 'finalized', maxSupportedTransactionVersion: 0 }).send();
  assert(tx?.meta && tx.meta.err === null, `Transaction ${label} must succeed`);
  console.log(`${label}: finalized, slot=${tx.slot}, fee=${tx.meta.fee} lamports`);
}
const response = await fetch(result.metadataUri);
assert(response.ok, `Metadata URI returned HTTP ${response.status}`);
const published = await response.json() as typeof metadata;
assert.deepEqual(published, metadata);
console.log(`PASS: ${result.name} (${result.symbol}), decimals=${result.decimals}`);
console.log('PASS: MetadataPointer + TokenMetadata, mint authority and token account owners');
console.log('PASS: supply=1000000; sender=999875; recipient=125 AIDOS');
console.log(`PASS: all ${Object.keys(result.transactions).length} transactions finalized; published metadata JSON matches local file`);
