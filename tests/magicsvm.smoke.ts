import { MagicSVM, Keypair, SystemProgram, Transaction } from "@magicblock-labs/magicsvm";

// Runtime smoke test: MagicSVM owns base and ER ledgers. Program-specific
// delegation tests belong in the deployed/mb-stack suite because this project
// pins the documented Anchor 1.0 / SDK 0.16 compatibility line.
const svm = new MagicSVM();
const payer = Keypair.generate();
const recipient = Keypair.generate();
svm.airdrop(payer.publicKey, 1_000_000_000);
const transaction = new Transaction().add(SystemProgram.transfer({ fromPubkey: payer.publicKey, toPubkey: recipient.publicKey, lamports: 1_000_000 }));
transaction.feePayer = payer.publicKey;
transaction.recentBlockhash = svm.latestBlockhash();
transaction.sign(payer);
svm.sendTransaction(transaction);
if (svm.getBalance(recipient.publicKey) !== 1_000_000) throw new Error("MagicSVM base ledger transfer failed");
console.log("MagicSVM base-ledger smoke test passed.");

