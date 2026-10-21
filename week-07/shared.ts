import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import {
  address, createSolanaRpc, unwrapOption,
} from '@solana/kit';
import {
  fetchMint, fetchToken, TOKEN_2022_PROGRAM_ADDRESS,
} from '@solana-program/token-2022';

export const rpc = createSolanaRpc('https://api.devnet.solana.com');
export const decimals = 6;
export const issued = 1_000_000n * 10n ** BigInt(decimals);
export const transferred = 125n * 10n ** BigInt(decimals);
export const metadata = JSON.parse(await readFile(new URL('metadata.json', import.meta.url), 'utf8'));
export const metadataUri = 'https://raw.githubusercontent.com/amangeldiaidos660/education/refs/heads/codex/week-07-spl-tokens/week-07/metadata.json';

export async function assertDevnet() {
  assert.equal(await rpc.getGenesisHash().send(), 'EtWTRABZaYq6iMfeYKouRu166VU2xqa1wcaWoxPkrZBG', 'Expected Solana Devnet');
}

export async function inspect(mintAddress: string, source: string, destination: string, owner: string, recipient: string) {
  const mint = await fetchMint(rpc, address(mintAddress), { commitment: 'finalized' });
  const from = await fetchToken(rpc, address(source), { commitment: 'finalized' });
  const to = await fetchToken(rpc, address(destination), { commitment: 'finalized' });
  assert.equal(mint.programAddress, TOKEN_2022_PROGRAM_ADDRESS);
  assert.equal(mint.data.decimals, decimals);
  assert.equal(unwrapOption(mint.data.mintAuthority), owner);
  assert.equal(unwrapOption(mint.data.freezeAuthority), null);
  for (const account of [from, to]) {
    assert.equal(account.programAddress, TOKEN_2022_PROGRAM_ADDRESS);
    assert.equal(account.data.mint, mintAddress);
  }
  assert.equal(from.data.owner, owner);
  assert.equal(to.data.owner, recipient);
  const extensions = unwrapOption(mint.data.extensions) ?? [];
  const pointer = extensions.find(e => e.__kind === 'MetadataPointer');
  const tokenMetadata = extensions.find(e => e.__kind === 'TokenMetadata');
  assert(pointer?.__kind === 'MetadataPointer');
  assert.equal(unwrapOption(pointer.metadataAddress), mintAddress);
  assert(tokenMetadata?.__kind === 'TokenMetadata');
  assert.equal(tokenMetadata.mint, mintAddress);
  assert.equal(tokenMetadata.name, metadata.name);
  assert.equal(tokenMetadata.symbol, metadata.symbol);
  assert.equal(tokenMetadata.uri, metadataUri);
  assert.equal(unwrapOption(tokenMetadata.updateAuthority), owner);
  return { supply: mint.data.supply, source: from.data.amount, destination: to.data.amount };
}

export function assertFinal(balances: Awaited<ReturnType<typeof inspect>>) {
  assert.equal(balances.supply, issued);
  assert.equal(balances.source, issued - transferred);
  assert.equal(balances.destination, transferred);
}
