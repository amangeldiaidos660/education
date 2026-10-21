import assert from 'node:assert/strict';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import {
  address, appendTransactionMessageInstructions, createKeyPairSignerFromBytes,
  createTransactionMessage, generateKeyPairSigner, getBase64EncodedWireTransaction,
  getSignatureFromTransaction, pipe, setTransactionMessageFeePayerSigner,
  setTransactionMessageLifetimeUsingBlockhash, signature as signatureFromString, signTransactionMessageWithSigners,
  type Instruction,
} from '@solana/kit';
import { getCreateAccountInstruction } from '@solana-program/system';
import {
  extension, findAssociatedTokenPda, getCreateAssociatedTokenIdempotentInstruction,
  getInitializeMetadataPointerInstruction, getInitializeMint2Instruction,
  getInitializeTokenMetadataInstruction, getMintSize, getMintToCheckedInstruction,
  getTransferCheckedInstruction, TOKEN_2022_PROGRAM_ADDRESS,
} from '@solana-program/token-2022';
import {
  assertDevnet, assertFinal, decimals, inspect, issued, metadata, metadataUri,
  rpc, transferred,
} from './shared.js';

await assertDevnet();
assert(process.env.SOLANA_KEYPAIR, 'Set SOLANA_KEYPAIR to your local devnet keypair path');
assert(process.env.RECIPIENT, 'Set RECIPIENT to a different wallet public address');
const secret = JSON.parse(await readFile(process.env.SOLANA_KEYPAIR, 'utf8'));
assert(Array.isArray(secret) && secret.length === 64 && secret.every(n => Number.isInteger(n) && n >= 0 && n <= 255), 'Invalid keypair');
const payer = await createKeyPairSignerFromBytes(Uint8Array.from(secret));
const recipient = address(process.env.RECIPIENT);
assert.notEqual(recipient, payer.address, 'Use distinct wallets');
const local = new URL('.local/', import.meta.url);
await mkdir(local, { recursive: true });
const progressFile = new URL('progress.json', local);
type Progress = { owner: string; recipient: string; mint: string; transactions: Record<string, string> };
let progress: Progress;
let mintSigner;
try {
  progress = JSON.parse(await readFile(progressFile, 'utf8'));
  assert.equal(progress.owner, payer.address);
  assert.equal(progress.recipient, recipient);
} catch (error) {
  if ((error as NodeJS.ErrnoException).code !== 'ENOENT') throw error;
  mintSigner = await generateKeyPairSigner();
  progress = { owner: payer.address, recipient, mint: mintSigner.address, transactions: {} };
}
const mint = address(progress.mint);
const [source] = await findAssociatedTokenPda({ owner: payer.address, mint, tokenProgram: TOKEN_2022_PROGRAM_ADDRESS });
const [destination] = await findAssociatedTokenPda({ owner: recipient, mint, tokenProgram: TOKEN_2022_PROGRAM_ADDRESS });
console.log('Network: Solana Devnet');
console.log('Wallet:', payer.address);
console.log('Recipient:', recipient);
console.log('Mint:', mint);

async function send(label: string, instructions: Instruction[]) {
  if (progress.transactions[label]) {
    const status = await rpc.getSignatureStatuses([signatureFromString(progress.transactions[label])], { searchTransactionHistory: true }).send();
    assert(status.value[0]?.confirmationStatus === 'finalized' && status.value[0].err === null, `Inspect saved transaction ${label} before retrying`);
    console.log(`${label}: already finalized ${progress.transactions[label]}`);
    return;
  }
  const { value: lifetime } = await rpc.getLatestBlockhash({ commitment: 'confirmed' }).send();
  const message = pipe(
    createTransactionMessage({ version: 0 }),
    tx => setTransactionMessageFeePayerSigner(payer, tx),
    tx => setTransactionMessageLifetimeUsingBlockhash(lifetime, tx),
    tx => appendTransactionMessageInstructions(instructions, tx),
  );
  const transaction = await signTransactionMessageWithSigners(message);
  const signature = getSignatureFromTransaction(transaction);
  progress.transactions[label] = signature;
  await writeFile(progressFile, JSON.stringify(progress, null, 2));
  await rpc.sendTransaction(getBase64EncodedWireTransaction(transaction), { encoding: 'base64', preflightCommitment: 'confirmed', maxRetries: 5n }).send();
  for (let attempt = 0; attempt < 120; attempt++) {
    const { value: [status] } = await rpc.getSignatureStatuses([signature], { searchTransactionHistory: true }).send();
    assert(!status?.err, JSON.stringify(status?.err));
    if (status?.confirmationStatus === 'finalized') {
      console.log(`${label}: finalized ${signature}`);
      return;
    }
    await new Promise(resolve => setTimeout(resolve, 1000));
  }
  throw new Error(`Confirmation timeout: ${signature}. Check the signature before retrying.`);
}

if (!progress.transactions.create) {
  assert(mintSigner, 'Missing mint signer: inspect local progress');
  const pointer = extension('MetadataPointer', { authority: payer.address, metadataAddress: mint });
  const data = extension('TokenMetadata', {
    updateAuthority: payer.address, mint, name: metadata.name, symbol: metadata.symbol,
    uri: metadataUri, additionalMetadata: new Map(),
  });
  const rent = await rpc.getMinimumBalanceForRentExemption(BigInt(getMintSize([pointer, data]))).send();
  await send('create', [
    getCreateAccountInstruction({ payer, newAccount: mintSigner, lamports: rent, space: getMintSize([pointer]), programAddress: TOKEN_2022_PROGRAM_ADDRESS }),
    getInitializeMetadataPointerInstruction({ mint, authority: payer.address, metadataAddress: mint }),
    getInitializeMint2Instruction({ mint, decimals, mintAuthority: payer.address, freezeAuthority: null }),
    getInitializeTokenMetadataInstruction({ metadata: mint, updateAuthority: payer.address, mint, mintAuthority: payer, name: metadata.name, symbol: metadata.symbol, uri: metadataUri }),
  ]);
} else {
  await send('create', []);
}
await send('accounts', [
  getCreateAssociatedTokenIdempotentInstruction({ payer, ata: source, owner: payer.address, mint, tokenProgram: TOKEN_2022_PROGRAM_ADDRESS }),
  getCreateAssociatedTokenIdempotentInstruction({ payer, ata: destination, owner: recipient, mint, tokenProgram: TOKEN_2022_PROGRAM_ADDRESS }),
]);
await send('mint', [getMintToCheckedInstruction({ mint, token: source, mintAuthority: payer, amount: issued, decimals })]);
if (!progress.transactions.transfer) {
  const before = await inspect(mint, source, destination, payer.address, recipient);
  assert.equal(before.supply, issued);
  assert.equal(before.source, issued);
  assert.equal(before.destination, 0n);
  console.log('Before transfer: supply=1000000, sender=1000000, recipient=0 AIDOS');
}
await send('transfer', [getTransferCheckedInstruction({ source, mint, destination, authority: payer, amount: transferred, decimals })]);
const balances = await inspect(mint, source, destination, payer.address, recipient);
assertFinal(balances);
const result = {
  network: 'devnet', verifiedAt: new Date().toISOString(), owner: payer.address, recipient,
  mint, source, destination, tokenProgram: TOKEN_2022_PROGRAM_ADDRESS,
  name: metadata.name, symbol: metadata.symbol, decimals, metadataUri,
  issuedRaw: issued.toString(), transferredRaw: transferred.toString(),
  balances: { supply: balances.supply.toString(), source: balances.source.toString(), destination: balances.destination.toString() },
  transactions: progress.transactions,
  explorer: `https://explorer.solana.com/address/${mint}?cluster=devnet`,
};
await writeFile(new URL('results.json', import.meta.url), JSON.stringify(result, null, 2) + '\n');
console.log('After transfer: supply=1000000, sender=999875, recipient=125 AIDOS');
console.log('MetadataPointer + TokenMetadata: verified');
console.log('PASS: mint, metadata, owners, decimals, supply and balances verified on-chain');
console.log('Explorer:', result.explorer);
