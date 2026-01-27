# Step 10: Frontend Integration

## Overview

Build a React frontend that:

1. Connects to user wallets
2. Generates ZK proofs (via backend service)
3. Interacts with DePLOB contracts
4. Manages deposit notes securely

## 10.1 Project Setup

```bash
# Create React app with Vite
cd frontend
npm create vite@latest . -- --template react-ts
npm install

# Install dependencies
npm install ethers@6 @tanstack/react-query zustand
npm install -D tailwindcss postcss autoprefixer
npx tailwindcss init -p
```

### Project Structure

```
frontend/
├── src/
│   ├── components/
│   │   ├── Layout.tsx
│   │   ├── WalletConnect.tsx
│   │   ├── Deposit.tsx
│   │   ├── Withdraw.tsx
│   │   ├── OrderBook.tsx
│   │   ├── CreateOrder.tsx
│   │   └── MyOrders.tsx
│   ├── hooks/
│   │   ├── useWallet.ts
│   │   ├── useContract.ts
│   │   ├── useNotes.ts
│   │   └── useProof.ts
│   ├── utils/
│   │   ├── deposit.ts
│   │   ├── withdraw.ts
│   │   ├── order.ts
│   │   ├── encryption.ts
│   │   └── merkleIndexer.ts
│   ├── contracts/
│   │   ├── DePLOB.json        # ABI
│   │   └── addresses.ts
│   ├── store/
│   │   └── noteStore.ts
│   ├── App.tsx
│   └── main.tsx
├── public/
└── package.json
```

## 10.2 Wallet Connection

`src/hooks/useWallet.ts`:

```typescript
import { useState, useCallback, useEffect } from 'react';
import { ethers, BrowserProvider, Signer } from 'ethers';

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
    provider: null,
    signer: null,
    address: null,
    chainId: null,
    isConnecting: false,
    error: null,
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

      setState({
        provider,
        signer,
        address,
        chainId: Number(network.chainId),
        isConnecting: false,
        error: null,
      });
    } catch (err: any) {
      setState(s => ({
        ...s,
        isConnecting: false,
        error: err.message || 'Failed to connect',
      }));
    }
  }, []);

  const disconnect = useCallback(() => {
    setState({
      provider: null,
      signer: null,
      address: null,
      chainId: null,
      isConnecting: false,
      error: null,
    });
  }, []);

  // Listen for account changes
  useEffect(() => {
    if (typeof window.ethereum === 'undefined') return;

    const handleAccountsChanged = (accounts: string[]) => {
      if (accounts.length === 0) {
        disconnect();
      } else if (state.address !== accounts[0]) {
        connect();
      }
    };

    const handleChainChanged = () => {
      window.location.reload();
    };

    window.ethereum.on('accountsChanged', handleAccountsChanged);
    window.ethereum.on('chainChanged', handleChainChanged);

    return () => {
      window.ethereum.removeListener('accountsChanged', handleAccountsChanged);
      window.ethereum.removeListener('chainChanged', handleChainChanged);
    };
  }, [state.address, connect, disconnect]);

  return {
    ...state,
    connect,
    disconnect,
    isConnected: !!state.address,
  };
}
```

`src/components/WalletConnect.tsx`:

```tsx
import { useWallet } from '../hooks/useWallet';

export function WalletConnect() {
  const { address, isConnecting, connect, disconnect, isConnected, error } = useWallet();

  if (isConnected) {
    return (
      <div className="flex items-center gap-4">
        <span className="text-sm text-gray-600">
          {address?.slice(0, 6)}...{address?.slice(-4)}
        </span>
        <button
          onClick={disconnect}
          className="px-4 py-2 bg-red-500 text-white rounded hover:bg-red-600"
        >
          Disconnect
        </button>
      </div>
    );
  }

  return (
    <div>
      <button
        onClick={connect}
        disabled={isConnecting}
        className="px-4 py-2 bg-blue-500 text-white rounded hover:bg-blue-600 disabled:opacity-50"
      >
        {isConnecting ? 'Connecting...' : 'Connect Wallet'}
      </button>
      {error && <p className="text-red-500 text-sm mt-2">{error}</p>}
    </div>
  );
}
```

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

  const contract = useMemo(() => {
    if (!provider) return null;

    // Use signer if available, otherwise use provider (read-only)
    const signerOrProvider = signer || provider;
    return new Contract(DEPLOB_ADDRESS, DePLOBABI.abi, signerOrProvider);
  }, [signer, provider]);

  return contract;
}

export function useERC20(tokenAddress: string) {
  const { signer, provider } = useWallet();

  const contract = useMemo(() => {
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

  return contract;
}
```

## 10.4 Note Storage

`src/store/noteStore.ts`:

```typescript
import { create } from 'zustand';
import { persist } from 'zustand/middleware';
import { DepositNote } from '../utils/deposit';
import { OrderNote } from '../utils/order';

interface NoteStore {
  // Deposit notes
  depositNotes: DepositNote[];
  addDepositNote: (note: DepositNote) => void;
  removeDepositNote: (commitment: string) => void;

  // Order notes
  orderNotes: OrderNote[];
  addOrderNote: (note: OrderNote) => void;
  removeOrderNote: (commitment: string) => void;

  // Encryption key (derived from wallet signature)
  encryptionKey: string | null;
  setEncryptionKey: (key: string) => void;
}

export const useNoteStore = create<NoteStore>()(
  persist(
    (set) => ({
      depositNotes: [],
      addDepositNote: (note) =>
        set((state) => ({
          depositNotes: [...state.depositNotes, note],
        })),
      removeDepositNote: (commitment) =>
        set((state) => ({
          depositNotes: state.depositNotes.filter(
            (n) => n.commitment !== commitment
          ),
        })),

      orderNotes: [],
      addOrderNote: (note) =>
        set((state) => ({
          orderNotes: [...state.orderNotes, note],
        })),
      removeOrderNote: (commitment) =>
        set((state) => ({
          orderNotes: state.orderNotes.filter(
            (n) => n.orderCommitment !== commitment
          ),
        })),

      encryptionKey: null,
      setEncryptionKey: (key) => set({ encryptionKey: key }),
    }),
    {
      name: 'deplob-notes',
      // In production, encrypt storage with user's key
    }
  )
);
```

## 10.5 Deposit Component

`src/components/Deposit.tsx`:

```tsx
import { useState } from 'react';
import { ethers } from 'ethers';
import { useDePLOB, useERC20 } from '../hooks/useContract';
import { useWallet } from '../hooks/useWallet';
import { useNoteStore } from '../store/noteStore';
import { createDepositNote, DepositNote } from '../utils/deposit';

interface DepositProps {
  tokenAddress: string;
  tokenSymbol: string;
  decimals: number;
}

export function Deposit({ tokenAddress, tokenSymbol, decimals }: DepositProps) {
  const [amount, setAmount] = useState('');
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [txHash, setTxHash] = useState<string | null>(null);

  const deplob = useDePLOB();
  const token = useERC20(tokenAddress);
  const { address } = useWallet();
  const addDepositNote = useNoteStore((s) => s.addDepositNote);

  const handleDeposit = async () => {
    if (!deplob || !token || !address || !amount) return;

    setIsLoading(true);
    setError(null);
    setTxHash(null);

    try {
      const amountWei = ethers.parseUnits(amount, decimals);

      // Check allowance
      const allowance = await token.allowance(address, await deplob.getAddress());
      if (allowance < amountWei) {
        const approveTx = await token.approve(await deplob.getAddress(), amountWei);
        await approveTx.wait();
      }

      // Create deposit note
      const note = await createDepositNote(tokenAddress, amountWei);

      // Generate proof
      const proofResponse = await fetch('/api/prove/deposit', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          nullifierNote: note.nullifierNote,
          secret: note.secret,
          token: tokenAddress,
          amount: amountWei.toString(),
        }),
      });

      if (!proofResponse.ok) {
        throw new Error('Failed to generate proof');
      }

      const { proof } = await proofResponse.json();

      // Submit deposit
      const tx = await deplob.deposit(
        note.commitment,
        tokenAddress,
        amountWei,
        proof
      );

      setTxHash(tx.hash);
      const receipt = await tx.wait();

      // Extract leaf index from event
      const depositEvent = receipt.logs.find(
        (log: any) => log.fragment?.name === 'Deposit'
      );
      if (depositEvent) {
        note.leafIndex = Number(depositEvent.args.leafIndex);
      }

      // Save note
      addDepositNote(note);

      setAmount('');
      alert('Deposit successful! Note saved.');
    } catch (err: any) {
      setError(err.message || 'Deposit failed');
    } finally {
      setIsLoading(false);
    }
  };

  return (
    <div className="p-6 bg-white rounded-lg shadow">
      <h2 className="text-xl font-bold mb-4">Deposit {tokenSymbol}</h2>

      <div className="space-y-4">
        <div>
          <label className="block text-sm font-medium text-gray-700">
            Amount
          </label>
          <input
            type="number"
            value={amount}
            onChange={(e) => setAmount(e.target.value)}
            placeholder="0.0"
            className="mt-1 block w-full px-3 py-2 border border-gray-300 rounded-md"
          />
        </div>

        <button
          onClick={handleDeposit}
          disabled={isLoading || !amount}
          className="w-full px-4 py-2 bg-green-500 text-white rounded hover:bg-green-600 disabled:opacity-50"
        >
          {isLoading ? 'Processing...' : 'Deposit'}
        </button>

        {error && (
          <p className="text-red-500 text-sm">{error}</p>
        )}

        {txHash && (
          <p className="text-green-600 text-sm">
            Transaction: {txHash.slice(0, 10)}...
          </p>
        )}
      </div>
    </div>
  );
}
```

## 10.6 Withdraw Component

`src/components/Withdraw.tsx`:

```tsx
import { useState } from 'react';
import { ethers } from 'ethers';
import { useDePLOB } from '../hooks/useContract';
import { useNoteStore } from '../store/noteStore';
import { generateWithdrawProof } from '../utils/withdraw';
import { MerkleIndexer } from '../utils/merkleIndexer';

export function Withdraw() {
  const [selectedNote, setSelectedNote] = useState<string | null>(null);
  const [recipient, setRecipient] = useState('');
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const deplob = useDePLOB();
  const { depositNotes, removeDepositNote } = useNoteStore();

  const handleWithdraw = async () => {
    if (!deplob || !selectedNote || !recipient) return;

    const note = depositNotes.find((n) => n.commitment === selectedNote);
    if (!note) return;

    setIsLoading(true);
    setError(null);

    try {
      // Get Merkle proof from indexer
      const indexer = new MerkleIndexer(/* provider, contract */);
      await indexer.sync();

      // Generate withdrawal proof
      const { nullifier, root, proof } = await generateWithdrawProof(
        { note, recipient },
        indexer
      );

      // Submit withdrawal
      const tx = await deplob.withdraw(
        nullifier,
        recipient,
        note.token,
        note.amount,
        root,
        proof
      );

      await tx.wait();

      // Remove note
      removeDepositNote(selectedNote);

      alert('Withdrawal successful!');
      setSelectedNote(null);
      setRecipient('');
    } catch (err: any) {
      setError(err.message || 'Withdrawal failed');
    } finally {
      setIsLoading(false);
    }
  };

  return (
    <div className="p-6 bg-white rounded-lg shadow">
      <h2 className="text-xl font-bold mb-4">Withdraw</h2>

      <div className="space-y-4">
        <div>
          <label className="block text-sm font-medium text-gray-700">
            Select Deposit
          </label>
          <select
            value={selectedNote || ''}
            onChange={(e) => setSelectedNote(e.target.value)}
            className="mt-1 block w-full px-3 py-2 border border-gray-300 rounded-md"
          >
            <option value="">Select a deposit...</option>
            {depositNotes.map((note) => (
              <option key={note.commitment} value={note.commitment}>
                {ethers.formatUnits(note.amount, 18)} tokens
              </option>
            ))}
          </select>
        </div>

        <div>
          <label className="block text-sm font-medium text-gray-700">
            Recipient Address
          </label>
          <input
            type="text"
            value={recipient}
            onChange={(e) => setRecipient(e.target.value)}
            placeholder="0x..."
            className="mt-1 block w-full px-3 py-2 border border-gray-300 rounded-md"
          />
          <p className="text-xs text-gray-500 mt-1">
            Use a fresh address for privacy
          </p>
        </div>

        <button
          onClick={handleWithdraw}
          disabled={isLoading || !selectedNote || !recipient}
          className="w-full px-4 py-2 bg-blue-500 text-white rounded hover:bg-blue-600 disabled:opacity-50"
        >
          {isLoading ? 'Processing...' : 'Withdraw'}
        </button>

        {error && <p className="text-red-500 text-sm">{error}</p>}
      </div>
    </div>
  );
}
```

## 10.7 Create Order Component

`src/components/CreateOrder.tsx`:

```tsx
import { useState } from 'react';
import { ethers } from 'ethers';
import { useDePLOB } from '../hooks/useContract';
import { useNoteStore } from '../store/noteStore';
import { createOrder, OrderSide } from '../utils/order';

export function CreateOrder() {
  const [selectedDeposit, setSelectedDeposit] = useState<string | null>(null);
  const [price, setPrice] = useState('');
  const [quantity, setQuantity] = useState('');
  const [side, setSide] = useState<OrderSide>(OrderSide.Buy);
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const deplob = useDePLOB();
  const { depositNotes, addOrderNote } = useNoteStore();

  // TEE public key for encryption (loaded from contract or config)
  const TEE_PUBLIC_KEY = '0x...';

  const handleCreateOrder = async () => {
    if (!deplob || !selectedDeposit || !price || !quantity) return;

    const depositNote = depositNotes.find((n) => n.commitment === selectedDeposit);
    if (!depositNote) return;

    setIsLoading(true);
    setError(null);

    try {
      const orderParams = {
        price: ethers.parseUnits(price, 6), // Assuming 6 decimals for price
        quantity: ethers.parseUnits(quantity, 18),
        side,
        tokenIn: depositNote.token,
        tokenOut: '0x...', // Selected output token
      };

      const { orderNote, encryptedOrder, proof } = await createOrder(
        depositNote,
        orderParams,
        TEE_PUBLIC_KEY
      );

      const tx = await deplob.createOrder(
        orderNote.orderCommitment,
        orderNote.depositNullifier,
        encryptedOrder,
        proof
      );

      await tx.wait();

      addOrderNote(orderNote);
      alert('Order created successfully!');

      setPrice('');
      setQuantity('');
      setSelectedDeposit(null);
    } catch (err: any) {
      setError(err.message || 'Order creation failed');
    } finally {
      setIsLoading(false);
    }
  };

  return (
    <div className="p-6 bg-white rounded-lg shadow">
      <h2 className="text-xl font-bold mb-4">Create Order</h2>

      <div className="space-y-4">
        <div>
          <label className="block text-sm font-medium text-gray-700">
            Select Deposit
          </label>
          <select
            value={selectedDeposit || ''}
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
        </div>

        <div className="flex gap-4">
          <button
            onClick={() => setSide(OrderSide.Buy)}
            className={`flex-1 py-2 rounded ${
              side === OrderSide.Buy
                ? 'bg-green-500 text-white'
                : 'bg-gray-200'
            }`}
          >
            Buy
          </button>
          <button
            onClick={() => setSide(OrderSide.Sell)}
            className={`flex-1 py-2 rounded ${
              side === OrderSide.Sell
                ? 'bg-red-500 text-white'
                : 'bg-gray-200'
            }`}
          >
            Sell
          </button>
        </div>

        <div className="grid grid-cols-2 gap-4">
          <div>
            <label className="block text-sm font-medium text-gray-700">
              Price
            </label>
            <input
              type="number"
              value={price}
              onChange={(e) => setPrice(e.target.value)}
              placeholder="0.00"
              className="mt-1 block w-full px-3 py-2 border border-gray-300 rounded-md"
            />
          </div>
          <div>
            <label className="block text-sm font-medium text-gray-700">
              Quantity
            </label>
            <input
              type="number"
              value={quantity}
              onChange={(e) => setQuantity(e.target.value)}
              placeholder="0.0"
              className="mt-1 block w-full px-3 py-2 border border-gray-300 rounded-md"
            />
          </div>
        </div>

        <button
          onClick={handleCreateOrder}
          disabled={isLoading || !selectedDeposit || !price || !quantity}
          className="w-full px-4 py-2 bg-purple-500 text-white rounded hover:bg-purple-600 disabled:opacity-50"
        >
          {isLoading ? 'Creating...' : 'Create Order'}
        </button>

        {error && <p className="text-red-500 text-sm">{error}</p>}
      </div>
    </div>
  );
}
```

## 10.8 Proof Generation API

Create a backend service to generate proofs (since SP1 proving is resource-intensive).

`backend/src/routes/prove.ts`:

```typescript
import express from 'express';
import { spawn } from 'child_process';
import fs from 'fs/promises';
import path from 'path';

const router = express.Router();

router.post('/deposit', async (req, res) => {
  try {
    const { nullifierNote, secret, token, amount } = req.body;

    // Write inputs to temp file
    const inputPath = `/tmp/deposit_input_${Date.now()}.json`;
    await fs.writeFile(inputPath, JSON.stringify({
      nullifier_note: nullifierNote,
      secret,
      token,
      amount,
    }));

    // Run SP1 prover
    const proverPath = path.join(__dirname, '../../sp1-programs/deposit/script');

    const result = await new Promise<string>((resolve, reject) => {
      const proc = spawn('cargo', ['run', '--release'], {
        cwd: proverPath,
        env: {
          ...process.env,
          DEPOSIT_INPUT: inputPath,
          GENERATE_PROOF: 'true',
        },
      });

      let output = '';
      proc.stdout.on('data', (data) => { output += data; });
      proc.stderr.on('data', (data) => { console.error(data.toString()); });

      proc.on('close', (code) => {
        if (code === 0) resolve(output);
        else reject(new Error(`Prover exited with code ${code}`));
      });
    });

    // Read proof artifacts
    const proof = await fs.readFile(path.join(proverPath, 'deposit_proof.bin'));
    const publicValues = await fs.readFile(
      path.join(proverPath, 'deposit_public_values.bin')
    );

    res.json({
      proof: '0x' + proof.toString('hex'),
      publicValues: '0x' + publicValues.toString('hex'),
    });
  } catch (error: any) {
    res.status(500).json({ error: error.message });
  }
});

// Similar routes for /withdraw, /create-order, /cancel-order

export default router;
```

## 10.9 Main App

`src/App.tsx`:

```tsx
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { WalletConnect } from './components/WalletConnect';
import { Deposit } from './components/Deposit';
import { Withdraw } from './components/Withdraw';
import { CreateOrder } from './components/CreateOrder';

const queryClient = new QueryClient();

function App() {
  return (
    <QueryClientProvider client={queryClient}>
      <div className="min-h-screen bg-gray-100">
        <header className="bg-white shadow">
          <div className="max-w-7xl mx-auto px-4 py-4 flex justify-between items-center">
            <h1 className="text-2xl font-bold text-gray-900">DePLOB</h1>
            <WalletConnect />
          </div>
        </header>

        <main className="max-w-7xl mx-auto px-4 py-8">
          <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-6">
            <Deposit
              tokenAddress="0x..."
              tokenSymbol="WETH"
              decimals={18}
            />
            <Withdraw />
            <CreateOrder />
          </div>
        </main>
      </div>
    </QueryClientProvider>
  );
}

export default App;
```

## 10.10 Run Frontend

```bash
# Development
cd frontend
npm run dev

# Build
npm run build

# Preview build
npm run preview
```

## 10.11 Checklist

- [ ] Wallet connection works
- [ ] Token approval flow works
- [ ] Deposit UI works end-to-end
- [ ] Withdraw UI works end-to-end
- [ ] Order creation UI works
- [ ] Notes stored securely
- [ ] Proof generation API works
- [ ] Error handling in place
- [ ] Loading states displayed
- [ ] Mobile responsive
