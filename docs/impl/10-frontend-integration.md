# 10 — Frontend Integration

## Overview

Build a React frontend that:

1. Connects to user wallets
2. Generates ZK proofs for withdrawals (via backend service)
3. Interacts with the DePLOB contract for deposits and withdrawals
4. Submits orders and cancellations directly to the TEE HTTP API
5. Manages deposit notes securely

---

## 10.1 Project Setup

```bash
cd frontend
npm create vite@latest . -- --template react-ts
npm install

npm install ethers@6 @tanstack/react-query zustand
npm install -D tailwindcss postcss autoprefixer
npx tailwindcss init -p
```

### Project Structure

```text
frontend/
├── src/
│   ├── components/
│   │   ├── Layout.tsx
│   │   ├── WalletConnect.tsx
│   │   ├── Deposit.tsx
│   │   ├── Withdraw.tsx
│   │   ├── CreateOrder.tsx
│   │   └── MyOrders.tsx
│   ├── hooks/
│   │   ├── useWallet.ts
│   │   ├── useContract.ts
│   │   └── useNotes.ts
│   ├── utils/
│   │   ├── deposit.ts        ← note generation + commitment formula
│   │   ├── merkleIndexer.ts  ← reconstruct on-chain Merkle tree
│   │   ├── withdraw.ts       ← proof API calls
│   │   └── order.ts          ← TEE API calls
│   ├── contracts/
│   │   ├── DePLOB.json       ← ABI from forge artifacts
│   │   └── addresses.ts
│   ├── store/
│   │   └── noteStore.ts
│   ├── App.tsx
│   └── main.tsx
└── package.json
```

---

## 10.2 Wallet Connection

`src/hooks/useWallet.ts`:

```typescript
import { useState, useCallback, useEffect } from 'react';
import { BrowserProvider, Signer } from 'ethers';

interface WalletState {
  provider: BrowserProvider | null;
  signer: Signer | null;
  address: string | null;
  chainId: number | null;
  isConnecting: boolean;
  error: string | null;
}

export function useWallet() {
  const [state, setState] = useState<WalletState>({
    provider: null, signer: null, address: null,
    chainId: null, isConnecting: false, error: null,
  });

  const connect = useCallback(async () => {
    if (typeof window.ethereum === 'undefined') {
      setState(s => ({ ...s, error: 'Please install MetaMask' }));
      return;
    }
    setState(s => ({ ...s, isConnecting: true, error: null }));
    try {
      const provider = new BrowserProvider(window.ethereum);
      await provider.send('eth_requestAccounts', []);
      const signer = await provider.getSigner();
      const address = await signer.getAddress();
      const network = await provider.getNetwork();
      setState({ provider, signer, address, chainId: Number(network.chainId),
                 isConnecting: false, error: null });
    } catch (err: any) {
      setState(s => ({ ...s, isConnecting: false, error: err.message }));
    }
  }, []);

  const disconnect = useCallback(() => {
    setState({ provider: null, signer: null, address: null,
               chainId: null, isConnecting: false, error: null });
  }, []);

  useEffect(() => {
    if (typeof window.ethereum === 'undefined') return;
    const handleAccountsChanged = (accounts: string[]) => {
      if (accounts.length === 0) disconnect();
      else if (state.address !== accounts[0]) connect();
    };
    window.ethereum.on('accountsChanged', handleAccountsChanged);
    window.ethereum.on('chainChanged', () => window.location.reload());
    return () => {
      window.ethereum.removeListener('accountsChanged', handleAccountsChanged);
    };
  }, [state.address, connect, disconnect]);

  return { ...state, connect, disconnect, isConnected: !!state.address };
}
```

---

## 10.3 Contract Hook

`src/hooks/useContract.ts`:

```typescript
import { useMemo } from 'react';
import { Contract } from 'ethers';
import { useWallet } from './useWallet';
import DePLOBABI from '../contracts/DePLOB.json';
import { DEPLOB_ADDRESS } from '../contracts/addresses';

export function useDePLOB() {
  const { signer, provider } = useWallet();
  return useMemo(() => {
    if (!provider) return null;
    return new Contract(DEPLOB_ADDRESS, DePLOBABI.abi, signer || provider);
  }, [signer, provider]);
}

export function useERC20(tokenAddress: string) {
  const { signer, provider } = useWallet();
  return useMemo(() => {
    if (!provider || !tokenAddress) return null;
    const ERC20_ABI = [
      'function balanceOf(address) view returns (uint256)',
      'function allowance(address owner, address spender) view returns (uint256)',
      'function approve(address spender, uint256 amount) returns (bool)',
      'function symbol() view returns (string)',
      'function decimals() view returns (uint8)',
    ];
    return new Contract(tokenAddress, ERC20_ABI, signer || provider);
  }, [signer, provider, tokenAddress]);
}
```

`src/contracts/addresses.ts`:

```typescript
export const DEPLOB_ADDRESS: string =
  import.meta.env.VITE_DEPLOB_ADDRESS ?? '0x0000000000000000000000000000000000000000';
```

`src/contracts/DePLOB.json` — copy from Foundry artifacts:

```bash
cp contracts/out/DePLOB.sol/DePLOB.json frontend/src/contracts/DePLOB.json
```

---

## 10.4 Note Storage

`src/store/noteStore.ts`:

```typescript
import { create } from 'zustand';
import { persist } from 'zustand/middleware';
import type { DepositNote } from '../utils/deposit';

export interface OpenOrder {
  orderId: string;          // hex-encoded — returned by POST /v1/orders
  depositCommitment: string;
  side: 'buy' | 'sell';
  price: string;
  quantity: string;
  tokenIn: string;
  tokenOut: string;
  createdAt: number;
}

interface NoteStore {
  depositNotes: DepositNote[];
  addDepositNote: (note: DepositNote) => void;
  removeDepositNote: (commitment: string) => void;

  openOrders: OpenOrder[];
  addOpenOrder: (order: OpenOrder) => void;
  removeOpenOrder: (orderId: string) => void;
}

export const useNoteStore = create<NoteStore>()(
  persist(
    (set) => ({
      depositNotes: [],
      addDepositNote: (note) =>
        set((s) => ({ depositNotes: [...s.depositNotes, note] })),
      removeDepositNote: (commitment) =>
        set((s) => ({
          depositNotes: s.depositNotes.filter((n) => n.commitment !== commitment),
        })),

      openOrders: [],
      addOpenOrder: (order) =>
        set((s) => ({ openOrders: [...s.openOrders, order] })),
      removeOpenOrder: (orderId) =>
        set((s) => ({
          openOrders: s.openOrders.filter((o) => o.orderId !== orderId),
        })),
    }),
    { name: 'deplob-notes' }
  )
);
```

---

## 10.5 Utility: `deposit.ts`

`src/utils/deposit.ts` — note generation and commitment computation.

### DepositNote type

```typescript
export interface DepositNote {
  nullifierNote: string; // hex-encoded 32 bytes e.g. "0xabc123..."
  secret: string;        // hex-encoded 32 bytes
  token: string;         // checksummed token address
  amount: bigint;        // deposit amount in token's base unit
  commitment: string;    // hex-encoded 32-byte commitment hash
  leafIndex: number;     // position in on-chain Merkle tree (set after deposit tx)
  blockNumber: number;   // block number of deposit tx (for Merkle re-sync)
}
```

### Commitment formula

The TypeScript formula must match `CommitmentPreimage::commitment()` in Rust (`deplob-core`):

```text
commitment = keccak256(
  nullifierNote[32]                           -- raw 32 bytes
  || secret[32]                               -- raw 32 bytes
  || zeroPadValue(token, 32)[32]              -- 12 zero bytes + 20-byte address
  || zeroPadValue(toBeHex(amount, 16), 32)[32]-- 16 zero bytes + 16-byte u128 big-endian
)
```

`zeroPadValue` in ethers v6 prepends zeros (left-pads), matching Rust's
`address_to_bytes32` (copies address into last 20 bytes) and `u128_to_bytes32`
(copies u128 BE into last 16 bytes).

### Full implementation

```typescript
import { ethers } from 'ethers';
import type { DepositNote } from './deposit';

export function generateNote(
  token: string,
  amount: bigint,
): Omit<DepositNote, 'leafIndex' | 'blockNumber'> {
  const nullifierNoteBytes = crypto.getRandomValues(new Uint8Array(32));
  const secretBytes = crypto.getRandomValues(new Uint8Array(32));
  const commitment = computeCommitment(nullifierNoteBytes, secretBytes, token, amount);
  return {
    nullifierNote: ethers.hexlify(nullifierNoteBytes),
    secret: ethers.hexlify(secretBytes),
    token: ethers.getAddress(token),
    amount,
    commitment,
  };
}

export function computeCommitment(
  nullifierNote: Uint8Array,
  secret: Uint8Array,
  token: string,
  amount: bigint,
): string {
  const tokenPadded  = ethers.zeroPadValue(ethers.getAddress(token), 32);
  const amountPadded = ethers.zeroPadValue(ethers.toBeHex(amount, 16), 32);
  const data = ethers.concat([
    nullifierNote,
    secret,
    ethers.getBytes(tokenPadded),
    ethers.getBytes(amountPadded),
  ]);
  return ethers.keccak256(data);
}

// nullifier = keccak256(nullifierNote)
export function computeNullifier(nullifierNote: string): string {
  return ethers.keccak256(nullifierNote);
}
```

---

## 10.6 Utility: `merkleIndexer.ts`

`src/utils/merkleIndexer.ts` — reconstructs the on-chain Merkle tree from
`Deposit` events and generates inclusion proofs.

### Zero values

The tree uses the same zero precomputation as `MerkleTreeWithHistory.sol` and
`deplob-core/merkle.rs`:

```text
zeros[0] = 0x0000...0000  (ethers.ZeroHash)
zeros[i] = keccak256(zeros[i-1] || zeros[i-1])
```

### MerkleProof type

```typescript
export interface MerkleProof {
  root: string;           // current root hex
  siblings: string[];     // length 20, hex strings
  pathIndices: number[];  // length 20, 0 = left child, 1 = right child
}
```

### MerkleIndexer implementation

```typescript
import { ethers, Contract } from 'ethers';

const TREE_DEPTH = 20;

function buildZeros(): string[] {
  const zeros: string[] = [ethers.ZeroHash];
  for (let i = 1; i <= TREE_DEPTH; i++) {
    zeros.push(
      ethers.keccak256(
        ethers.concat([ethers.getBytes(zeros[i - 1]), ethers.getBytes(zeros[i - 1])]),
      ),
    );
  }
  return zeros;
}

const ZEROS = buildZeros();

export class MerkleIndexer {
  private leaves: string[] = [];
  private synced = false;

  constructor(
    private contract: Contract,
    private provider: ethers.Provider,
  ) {}

  /** Fetch all on-chain Deposit events and rebuild local leaf array. */
  async sync(fromBlock = 0): Promise<void> {
    const filter = this.contract.filters.Deposit();
    const events = await this.contract.queryFilter(filter, fromBlock);
    // Sort ascending by leafIndex (should already be ordered but be safe)
    const sorted = [...events].sort(
      (a: any, b: any) => Number(a.args.leafIndex) - Number(b.args.leafIndex),
    );
    this.leaves = sorted.map((e: any) => e.args.commitment as string);
    this.synced = true;
  }

  /** Generate an inclusion proof for the leaf at `leafIndex`. */
  getProof(leafIndex: number): MerkleProof {
    if (!this.synced) throw new Error('Call sync() first');

    const size = 2 ** TREE_DEPTH;
    const layer: string[] = Array(size)
      .fill(null)
      .map((_, i) => (i < this.leaves.length ? this.leaves[i] : ZEROS[0]));

    const siblings: string[] = [];
    const pathIndices: number[] = [];
    let currentLayer = [...layer];
    let idx = leafIndex;

    for (let level = 0; level < TREE_DEPTH; level++) {
      const isRight = idx % 2 === 1;
      const siblingIdx = isRight ? idx - 1 : idx + 1;
      siblings.push(currentLayer[siblingIdx] ?? ZEROS[level]);
      pathIndices.push(isRight ? 1 : 0);

      const nextLayer: string[] = [];
      for (let i = 0; i < currentLayer.length; i += 2) {
        const left  = currentLayer[i];
        const right = currentLayer[i + 1] ?? ZEROS[level];
        nextLayer.push(
          ethers.keccak256(
            ethers.concat([ethers.getBytes(left), ethers.getBytes(right)]),
          ),
        );
      }
      currentLayer = nextLayer;
      idx = Math.floor(idx / 2);
    }

    return { root: currentLayer[0], siblings, pathIndices };
  }
}
```

---

## 10.7 Utility: `withdraw.ts`

`src/utils/withdraw.ts` — calls the backend proof service.

```typescript
import type { MerkleProof } from './merkleIndexer';

const BACKEND_URL = import.meta.env.VITE_BACKEND_URL ?? 'http://localhost:3001';

export interface WithdrawInput {
  nullifierNote: string;  // hex 32 bytes
  secret: string;         // hex 32 bytes
  token: string;          // token address
  amount: string;         // amount as decimal string (bigint.toString())
  merkleProof: MerkleProof;
  recipient: string;      // withdrawal recipient address
}

export interface ProofResult {
  proof: string;        // hex-encoded SP1 Groth16 proof
  publicValues: string; // hex-encoded ABI-encoded public values
}

export async function callProveApi(input: WithdrawInput): Promise<ProofResult> {
  const response = await fetch(`${BACKEND_URL}/api/prove/withdraw`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(input),
  });
  if (!response.ok) {
    const { error } = await response.json();
    throw new Error(error ?? 'Proof generation failed');
  }
  return response.json();
}
```

---

## 10.8 Utility: `order.ts` (TEE API)

`src/utils/order.ts` — orders and cancellations go directly to the TEE HTTP server.

```typescript
import type { DepositNote } from './deposit';
import type { MerkleProof } from './merkleIndexer';

const TEE_URL = import.meta.env.VITE_TEE_URL ?? 'http://localhost:3000';

export interface OrderParams {
  price: bigint;
  quantity: bigint;
  side: 'buy' | 'sell';
  tokenIn: string;   // hex address
  tokenOut: string;  // hex address
}

export async function submitOrder(
  depositNote: DepositNote,
  merkleProof: MerkleProof,
  order: OrderParams,
): Promise<string> {
  const response = await fetch(`${TEE_URL}/v1/orders`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      deposit_nullifier_note: depositNote.nullifierNote,
      deposit_secret:         depositNote.secret,
      deposit_token:          depositNote.token,
      deposit_amount:         depositNote.amount.toString(),
      merkle_root:            merkleProof.root,
      merkle_siblings:        merkleProof.siblings,
      merkle_path_indices:    merkleProof.pathIndices,
      order: {
        price:     order.price.toString(),
        quantity:  order.quantity.toString(),
        side:      order.side,
        token_in:  order.tokenIn,
        token_out: order.tokenOut,
      },
    }),
  });

  if (!response.ok) {
    const { error } = await response.json();
    throw new Error(error);
  }

  const { order_id } = await response.json();
  return order_id; // save this to cancel later
}

export async function cancelOrder(
  orderId: string,
  depositNote: DepositNote,
): Promise<void> {
  const response = await fetch(`${TEE_URL}/v1/orders/${orderId}`, {
    method: 'DELETE',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      deposit_nullifier_note: depositNote.nullifierNote,
    }),
  });

  if (!response.ok) {
    const { error } = await response.json();
    throw new Error(error);
  }
}
```

---

## 10.9 Deposit Component

`src/components/Deposit.tsx`:

```tsx
import { useState } from 'react';
import { ethers } from 'ethers';
import { useWallet } from '../hooks/useWallet';
import { useDePLOB, useERC20 } from '../hooks/useContract';
import { useNoteStore } from '../store/noteStore';
import { generateNote } from '../utils/deposit';
import { DEPLOB_ADDRESS } from '../contracts/addresses';

interface DepositProps {
  tokenAddress: string;
  tokenSymbol: string;
  decimals: number;
}

export function Deposit({ tokenAddress, tokenSymbol, decimals }: DepositProps) {
  const [amount, setAmount] = useState('');
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const { signer, address } = useWallet();
  const deplob = useDePLOB();
  const token = useERC20(tokenAddress);
  const { addDepositNote } = useNoteStore();

  const handleDeposit = async () => {
    if (!signer || !deplob || !token || !address || !amount) return;
    setIsLoading(true);
    setError(null);
    try {
      const amountWei = ethers.parseUnits(amount, decimals);

      // 1. Generate random note (nullifierNote + secret + commitment)
      const note = generateNote(tokenAddress, amountWei);

      // 2. Check ERC20 allowance; approve if needed
      const allowance: bigint = await token.allowance(address, DEPLOB_ADDRESS);
      if (allowance < amountWei) {
        const approveTx = await token.approve(DEPLOB_ADDRESS, amountWei);
        await approveTx.wait();
      }

      // 3. Call deplob.deposit(commitment, token, amount)
      const tx = await deplob.deposit(note.commitment, tokenAddress, amountWei);
      const receipt = await tx.wait();

      // 4. Parse leafIndex from Deposit(commitment, leafIndex, timestamp) event
      const depositEvent = receipt.logs
        .map((log: any) => {
          try { return deplob.interface.parseLog(log); } catch { return null; }
        })
        .find((e: any) => e?.name === 'Deposit');
      if (!depositEvent) throw new Error('Deposit event not found in receipt');
      const leafIndex = Number(depositEvent.args.leafIndex);

      // 5. Persist note to localStorage
      addDepositNote({ ...note, leafIndex, blockNumber: receipt.blockNumber });

      alert(`Deposited! Leaf index: ${leafIndex}. Your deposit note has been saved.`);
      setAmount('');
    } catch (err: any) {
      setError(err.message ?? 'Deposit failed');
    } finally {
      setIsLoading(false);
    }
  };

  return (
    <div className="p-6 bg-white rounded-lg shadow">
      <h2 className="text-xl font-bold mb-4">Deposit</h2>
      <div className="space-y-4">
        <input
          type="number"
          value={amount}
          onChange={(e) => setAmount(e.target.value)}
          placeholder={`Amount (${tokenSymbol})`}
          className="block w-full px-3 py-2 border border-gray-300 rounded-md"
        />
        <button
          onClick={handleDeposit}
          disabled={isLoading || !signer || !amount}
          className="w-full px-4 py-2 bg-blue-500 text-white rounded hover:bg-blue-600 disabled:opacity-50"
        >
          {isLoading ? 'Depositing...' : `Deposit ${tokenSymbol}`}
        </button>
        {error && <p className="text-red-500 text-sm">{error}</p>}
      </div>
    </div>
  );
}
```

### Deposit flow summary

1. `generateNote(token, amount)` — random `nullifierNote` + `secret`, compute `commitment`
2. Check ERC20 allowance → call `token.approve()` if needed
3. `deplob.deposit(commitment, token, amount)` → wait for tx
4. Parse `leafIndex` from `Deposit(commitment, leafIndex, timestamp)` event
5. `addDepositNote({ ...note, leafIndex, blockNumber })` → persisted to localStorage

---

## 10.10 Withdraw Component

`src/components/Withdraw.tsx`:

```tsx
import { useState } from 'react';
import { ethers } from 'ethers';
import { useWallet } from '../hooks/useWallet';
import { useDePLOB } from '../hooks/useContract';
import { useNoteStore } from '../store/noteStore';
import { MerkleIndexer } from '../utils/merkleIndexer';
import { callProveApi } from '../utils/withdraw';
import { computeNullifier } from '../utils/deposit';

export function Withdraw() {
  const [selectedCommitment, setSelectedCommitment] = useState('');
  const [recipient, setRecipient] = useState('');
  const [isLoading, setIsLoading] = useState(false);
  const [status, setStatus] = useState('');
  const [error, setError] = useState<string | null>(null);

  const { provider } = useWallet();
  const deplob = useDePLOB();
  const { depositNotes, removeDepositNote } = useNoteStore();

  const handleWithdraw = async () => {
    if (!provider || !deplob || !selectedCommitment || !recipient) return;
    setIsLoading(true);
    setError(null);
    try {
      const note = depositNotes.find((n) => n.commitment === selectedCommitment);
      if (!note) throw new Error('Note not found');

      // 1. Rebuild Merkle tree from on-chain Deposit events
      setStatus('Syncing Merkle tree...');
      const indexer = new MerkleIndexer(deplob, provider);
      await indexer.sync();
      const merkleProof = indexer.getProof(note.leafIndex);

      // 2. Request SP1 Groth16 proof from backend (slow: ~10-30 min on real hardware)
      setStatus('Generating ZK proof (this may take a while)...');
      const { proof, publicValues } = await callProveApi({
        nullifierNote: note.nullifierNote,
        secret: note.secret,
        token: note.token,
        amount: note.amount.toString(),
        merkleProof,
        recipient,
      });

      // 3. Submit withdrawal on-chain
      setStatus('Submitting withdrawal...');
      const nullifier = computeNullifier(note.nullifierNote);
      const tx = await deplob.withdraw(
        nullifier, recipient, note.token, note.amount,
        merkleProof.root, proof,
      );
      await tx.wait();

      // 4. Remove spent note from store
      removeDepositNote(note.commitment);
      setStatus('');
      alert('Withdrawal complete!');
      setSelectedCommitment('');
      setRecipient('');
    } catch (err: any) {
      setError(err.message ?? 'Withdrawal failed');
      setStatus('');
    } finally {
      setIsLoading(false);
    }
  };

  return (
    <div className="p-6 bg-white rounded-lg shadow">
      <h2 className="text-xl font-bold mb-4">Withdraw</h2>
      <div className="space-y-4">
        <select
          value={selectedCommitment}
          onChange={(e) => setSelectedCommitment(e.target.value)}
          className="block w-full px-3 py-2 border border-gray-300 rounded-md"
        >
          <option value="">Select a deposit...</option>
          {depositNotes.map((note) => (
            <option key={note.commitment} value={note.commitment}>
              {ethers.formatUnits(note.amount, 18)} {note.token.slice(0, 8)}...
              (leaf #{note.leafIndex})
            </option>
          ))}
        </select>
        <input
          type="text"
          value={recipient}
          onChange={(e) => setRecipient(e.target.value)}
          placeholder="Recipient address (0x...)"
          className="block w-full px-3 py-2 border border-gray-300 rounded-md"
        />
        <button
          onClick={handleWithdraw}
          disabled={isLoading || !selectedCommitment || !recipient}
          className="w-full px-4 py-2 bg-green-500 text-white rounded hover:bg-green-600 disabled:opacity-50"
        >
          {isLoading ? (status || 'Processing...') : 'Withdraw'}
        </button>
        {error && <p className="text-red-500 text-sm">{error}</p>}
      </div>
    </div>
  );
}
```

### Withdraw flow summary

1. User selects a deposit note + enters recipient address
2. `indexer.sync()` — queries all `Deposit` events and rebuilds local depth-20 tree
3. `indexer.getProof(note.leafIndex)` → `{ root, siblings[20], pathIndices[20] }`
4. `callProveApi(...)` → backend spawns `withdraw-script` (SP1 Groth16, ~10–30 min)
5. `computeNullifier(nullifierNote)` = `keccak256(nullifierNote)` computed client-side
6. `deplob.withdraw(nullifier, recipient, token, amount, root, proof)` — on-chain tx
7. `removeDepositNote(commitment)` — remove spent note from localStorage

---

## 10.11 Create Order Component

Orders are now submitted directly to the TEE — no smart contract call, no ZK proof.

`src/components/CreateOrder.tsx`:

```tsx
import { useState } from 'react';
import { ethers } from 'ethers';
import { useWallet } from '../hooks/useWallet';
import { useDePLOB } from '../hooks/useContract';
import { useNoteStore } from '../store/noteStore';
import { submitOrder } from '../utils/order';
import { MerkleIndexer } from '../utils/merkleIndexer';

export function CreateOrder() {
  const [selectedDeposit, setSelectedDeposit] = useState<string | null>(null);
  const [price, setPrice] = useState('');
  const [quantity, setQuantity] = useState('');
  const [side, setSide] = useState<'buy' | 'sell'>('buy');
  const [tokenOut, setTokenOut] = useState('');
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const { provider } = useWallet();
  const deplob = useDePLOB();
  const { depositNotes, addOpenOrder } = useNoteStore();

  const handleCreateOrder = async () => {
    if (!selectedDeposit || !price || !quantity || !tokenOut) return;
    if (!provider || !deplob) return;

    const depositNote = depositNotes.find((n) => n.commitment === selectedDeposit);
    if (!depositNote) return;

    setIsLoading(true);
    setError(null);

    try {
      // Get Merkle proof for the deposit
      const indexer = new MerkleIndexer(deplob, provider);
      await indexer.sync();
      const merkleProof = indexer.getProof(depositNote.leafIndex);

      const orderId = await submitOrder(depositNote, merkleProof, {
        price: ethers.parseUnits(price, 6),
        quantity: ethers.parseUnits(quantity, 18),
        side,
        tokenIn: depositNote.token,
        tokenOut,
      });

      // Save order for later cancellation
      addOpenOrder({
        orderId,
        depositCommitment: depositNote.commitment,
        side,
        price,
        quantity,
        tokenIn: depositNote.token,
        tokenOut,
        createdAt: Date.now(),
      });

      alert(`Order submitted! ID: ${orderId.slice(0, 10)}...`);
      setPrice('');
      setQuantity('');
      setSelectedDeposit(null);
    } catch (err: any) {
      setError(err.message ?? 'Order creation failed');
    } finally {
      setIsLoading(false);
    }
  };

  return (
    <div className="p-6 bg-white rounded-lg shadow">
      <h2 className="text-xl font-bold mb-4">Create Order</h2>

      <div className="space-y-4">
        <select
          value={selectedDeposit ?? ''}
          onChange={(e) => setSelectedDeposit(e.target.value)}
          className="mt-1 block w-full px-3 py-2 border border-gray-300 rounded-md"
        >
          <option value="">Select a deposit...</option>
          {depositNotes.map((note) => (
            <option key={note.commitment} value={note.commitment}>
              {ethers.formatUnits(note.amount, 18)} tokens
            </option>
          ))}
        </select>

        <div className="flex gap-4">
          {(['buy', 'sell'] as const).map((s) => (
            <button
              key={s}
              onClick={() => setSide(s)}
              className={`flex-1 py-2 rounded ${
                side === s
                  ? s === 'buy' ? 'bg-green-500 text-white' : 'bg-red-500 text-white'
                  : 'bg-gray-200'
              }`}
            >
              {s.charAt(0).toUpperCase() + s.slice(1)}
            </button>
          ))}
        </div>

        <div className="grid grid-cols-2 gap-4">
          <input
            type="number" value={price} onChange={(e) => setPrice(e.target.value)}
            placeholder="Price" className="px-3 py-2 border border-gray-300 rounded-md"
          />
          <input
            type="number" value={quantity} onChange={(e) => setQuantity(e.target.value)}
            placeholder="Quantity" className="px-3 py-2 border border-gray-300 rounded-md"
          />
        </div>

        <button
          onClick={handleCreateOrder}
          disabled={isLoading || !selectedDeposit || !price || !quantity}
          className="w-full px-4 py-2 bg-purple-500 text-white rounded hover:bg-purple-600 disabled:opacity-50"
        >
          {isLoading ? 'Sending to TEE...' : 'Create Order'}
        </button>

        {error && <p className="text-red-500 text-sm">{error}</p>}
      </div>
    </div>
  );
}
```

---

## 10.12 My Orders / Cancel Component

`src/components/MyOrders.tsx`:

```tsx
import { useNoteStore } from '../store/noteStore';
import { cancelOrder } from '../utils/order';

export function MyOrders() {
  const { openOrders, depositNotes, removeOpenOrder } = useNoteStore();

  const handleCancel = async (orderId: string, depositCommitment: string) => {
    const depositNote = depositNotes.find((n) => n.commitment === depositCommitment);
    if (!depositNote) {
      alert('Deposit note not found — cannot cancel');
      return;
    }
    try {
      await cancelOrder(orderId, depositNote);
      removeOpenOrder(orderId);
      alert('Order cancelled.');
    } catch (err: any) {
      alert(`Cancel failed: ${err.message}`);
    }
  };

  if (openOrders.length === 0) return <p>No open orders.</p>;

  return (
    <div className="p-6 bg-white rounded-lg shadow">
      <h2 className="text-xl font-bold mb-4">My Open Orders</h2>
      <ul className="space-y-2">
        {openOrders.map((order) => (
          <li key={order.orderId} className="flex items-center justify-between p-3 border rounded">
            <span className="text-sm">
              {order.side.toUpperCase()} {order.quantity} @ {order.price}
            </span>
            <button
              onClick={() => handleCancel(order.orderId, order.depositCommitment)}
              className="px-3 py-1 bg-red-500 text-white text-sm rounded"
            >
              Cancel
            </button>
          </li>
        ))}
      </ul>
    </div>
  );
}
```

---

## 10.13 Proof Generation API (withdraw only)

`backend/src/routes/prove.ts`:

The backend proof service now only handles **withdrawal proofs**. Order creation
and cancellation require no ZK proof (handled by the TEE).

```typescript
import express from 'express';
import { spawn } from 'child_process';
import fs from 'fs/promises';
import path from 'path';

const router = express.Router();

// Withdrawal proof — SP1 Groth16/Plonk proof
router.post('/withdraw', async (req, res) => {
  try {
    const { nullifierNote, secret, token, amount, merkleProof, recipient } = req.body;

    const inputPath = `/tmp/withdraw_input_${Date.now()}.json`;
    await fs.writeFile(inputPath, JSON.stringify({
      nullifier_note: nullifierNote, secret, token, amount,
      merkle_proof: merkleProof, recipient,
    }));

    const proverPath = path.join(__dirname, '../../sp1-programs/withdraw/script');
    await new Promise<void>((resolve, reject) => {
      const proc = spawn('cargo', ['run', '--release', '--bin', 'withdraw-script'], {
        cwd: proverPath,
        env: { ...process.env, WITHDRAW_INPUT: inputPath, GENERATE_PROOF: 'groth16' },
      });
      proc.stderr.on('data', (d) => console.error(d.toString()));
      proc.on('close', (code) => code === 0 ? resolve() : reject(new Error(`exit ${code}`)));
    });

    const proof = await fs.readFile(path.join(proverPath, 'withdraw_proof.bin'));
    const publicValues = await fs.readFile(path.join(proverPath, 'withdraw_public_values.bin'));

    res.json({
      proof: '0x' + proof.toString('hex'),
      publicValues: '0x' + publicValues.toString('hex'),
    });
  } catch (err: any) {
    res.status(500).json({ error: err.message });
  }
});

// Note: /deposit, /create-order, /cancel-order proof endpoints removed.
// Deposit needs no proof. Orders go directly to TEE HTTP API.

export default router;
```

### 10.13.1 Backend project structure

```text
backend/
├── src/
│   ├── index.ts          Express app entry point
│   └── routes/
│       └── prove.ts      POST /api/prove/withdraw
├── package.json
└── tsconfig.json
```

`backend/src/index.ts`:

```typescript
import express from 'express';
import cors from 'cors';
import proveRouter from './routes/prove';

const app = express();
app.use(cors());
app.use(express.json());
app.use('/api/prove', proveRouter);

const PORT = process.env.PORT ?? 3001;
app.listen(PORT, () => console.log(`Proof server running on :${PORT}`));
```

`backend/package.json`:

```json
{
  "name": "deplob-backend",
  "version": "0.1.0",
  "scripts": {
    "dev": "ts-node src/index.ts",
    "build": "tsc",
    "start": "node dist/index.js"
  },
  "dependencies": {
    "cors": "^2.8.5",
    "express": "^4.18.0"
  },
  "devDependencies": {
    "@types/cors": "^2.8.17",
    "@types/express": "^4.17.21",
    "@types/node": "^20.0.0",
    "ts-node": "^10.9.0",
    "typescript": "^5.0.0"
  }
}
```

---

## 10.14 Main App

`src/App.tsx`:

```tsx
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { WalletConnect } from './components/WalletConnect';
import { Deposit } from './components/Deposit';
import { Withdraw } from './components/Withdraw';
import { CreateOrder } from './components/CreateOrder';
import { MyOrders } from './components/MyOrders';

const queryClient = new QueryClient();

function App() {
  return (
    <QueryClientProvider client={queryClient}>
      <div className="min-h-screen bg-gray-100">
        <header className="bg-white shadow">
          <div className="max-w-7xl mx-auto px-4 py-4 flex justify-between items-center">
            <h1 className="text-2xl font-bold">DePLOB</h1>
            <WalletConnect />
          </div>
        </header>
        <main className="max-w-7xl mx-auto px-4 py-8 grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-6">
          <Deposit tokenAddress="0x..." tokenSymbol="WETH" decimals={18} />
          <Withdraw />
          <CreateOrder />
          <MyOrders />
        </main>
      </div>
    </QueryClientProvider>
  );
}

export default App;
```

---

## 10.15 Environment Variables

```bash
# frontend/.env.local
VITE_TEE_URL=http://localhost:3000       # TEE matching engine URL
VITE_BACKEND_URL=http://localhost:3001   # Backend proof service URL
VITE_DEPLOB_ADDRESS=0x...               # Deployed DePLOB contract
VITE_CHAIN_ID=1337                      # Local anvil
```

---

## 10.16 Run All Services

```bash
# 1. Start local chain
anvil

# 2. Deploy contracts and note the DePLOB address
cd contracts
forge script script/Deploy.s.sol --rpc-url http://localhost:8545 --broadcast
# Update VITE_DEPLOB_ADDRESS in frontend/.env.local

# 3. Copy ABI to frontend
cp out/DePLOB.sol/DePLOB.json ../frontend/src/contracts/DePLOB.json

# 4. Start TEE server (port 3000)
cd ..
cargo run -p deplob-tee

# 5. Start backend proof service (port 3001)
cd backend && npm install && npx ts-node src/index.ts

# 6. Start frontend (port 5173)
cd ../frontend && npm install && npm run dev

# Build frontend for production
npm run build
```

---

## 10.17 Checklist

- [ ] Wallet connection works (MetaMask)
- [ ] Token approval and deposit flow works end-to-end
- [ ] `computeCommitment()` TypeScript output matches Rust `CommitmentPreimage::commitment()` for same inputs
- [ ] `MerkleIndexer.getProof()` root matches `DePLOB.getLastRoot()` after deposit
- [ ] Withdraw flow works (ZK proof generated, nullifier spent on-chain)
- [ ] Backend proof service spawns `withdraw-script` and returns hex proof
- [ ] Create order POSTs to TEE and `order_id` saved locally
- [ ] Cancel order DELETEs from TEE and removes from open orders list
- [ ] No `createOrder`/`cancelOrder` contract calls anywhere in frontend
- [ ] Proof API only exposes `/api/prove/withdraw` endpoint
- [ ] Notes stored securely in localStorage (consider encrypting with wallet signature)
- [ ] Error handling and loading states shown for all async operations
- [ ] Mobile responsive layout
