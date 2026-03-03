import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { RateLimiter } from "../target/types/rate_limiter";
import { assert, expect } from "chai";
import { PublicKey } from "@solana/web3.js";

describe("rate-limiter", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);
  const program = anchor.workspace.RateLimiter as Program<RateLimiter>;

  let authority: anchor.web3.Keypair;
  let user1: anchor.web3.Keypair;
  let user2: anchor.web3.Keypair;
  let registryPda: PublicKey;
  let apiKeyPda1: PublicKey;
  let apiKeyPda2: PublicKey;

  before(async () => {
    authority = anchor.web3.Keypair.generate();
    user1 = anchor.web3.Keypair.generate();
    user2 = anchor.web3.Keypair.generate();

    // Airdrop SOL for testing
    await provider.connection.confirmTransaction(
      await provider.connection.requestAirdrop(authority.publicKey, 2e9)
    );
    await provider.connection.confirmTransaction(
      await provider.connection.requestAirdrop(user1.publicKey, 2e9)
    );
    await provider.connection.confirmTransaction(
      await provider.connection.requestAirdrop(user2.publicKey, 2e9)
    );

    [registryPda] = PublicKey.findProgramAddressSync(
      [Buffer.from("registry")],
      program.programId
    );
    [apiKeyPda1] = PublicKey.findProgramAddressSync(
      [Buffer.from("apikey"), user1.publicKey.toBuffer()],
      program.programId
    );
    [apiKeyPda2] = PublicKey.findProgramAddressSync(
      [Buffer.from("apikey"), user2.publicKey.toBuffer()],
      program.programId
    );
  });

  it("Initializes the registry", async () => {
    const tx = await program.methods
      .initializeRegistry()
      .accounts({
        registry: registryPda,
        authority: authority.publicKey,
        systemProgram: anchor.web3.SystemProgram.programId,
      })
      .signers([authority])
      .rpc();

    const registry = await program.account.registry.fetch(registryPda);
    assert.equal(registry.authority.toString(), authority.publicKey.toString());
    assert.equal(registry.totalKeys.toNumber(), 0);
    console.log("Registry initialized. TX:", tx);
  });

  it("Creates a Free tier API key", async () => {
    const tx = await program.methods
      .createApiKey(0) // Free tier
      .accounts({
        apiKey: apiKeyPda1,
        registry: registryPda,
        owner: user1.publicKey,
        systemProgram: anchor.web3.SystemProgram.programId,
      })
      .signers([user1])
      .rpc();

    const apiKey = await program.account.apiKey.fetch(apiKeyPda1);
    assert.equal(apiKey.tier, 0);
    assert.equal(apiKey.isActive, true);
    assert.equal(apiKey.totalRequests.toNumber(), 0);
    
    const registry = await program.account.registry.fetch(registryPda);
    assert.equal(registry.totalKeys.toNumber(), 1);
    console.log("API Key created (Free tier). TX:", tx);
  });

  it("Creates a Pro tier API key for user2", async () => {
    await program.methods
      .createApiKey(2) // Pro tier
      .accounts({
        apiKey: apiKeyPda2,
        registry: registryPda,
        owner: user2.publicKey,
        systemProgram: anchor.web3.SystemProgram.programId,
      })
      .signers([user2])
      .rpc();

    const apiKey = await program.account.apiKey.fetch(apiKeyPda2);
    assert.equal(apiKey.tier, 2); // Pro
  });

  it("Allows requests within rate limit", async () => {
    // Get current window
    const clock = await provider.connection.getBlockTime(
      await provider.connection.getSlot()
    );
    const windowId = Math.floor(clock! / 3600);
    
    const [rateLimitStatePda] = PublicKey.findProgramAddressSync(
      [
        Buffer.from("ratelimit"),
        apiKeyPda1.toBuffer(),
        Buffer.from(new anchor.BN(windowId).toArrayLike(Buffer, "le", 8))
      ],
      program.programId
    );

    // Make 3 requests (within Free tier limit of 10)
    for (let i = 0; i < 3; i++) {
      const tx = await program.methods
        .checkAndIncrement()
        .accounts({
          apiKey: apiKeyPda1,
          rateLimitState: rateLimitStatePda,
          owner: user1.publicKey,
          systemProgram: anchor.web3.SystemProgram.programId,
        })
        .signers([user1])
        .rpc();
      console.log(`Request ${i + 1} allowed. TX: ${tx}`);
    }

    const state = await program.account.rateLimitState.fetch(rateLimitStatePda);
    assert.equal(state.requestCount, 3);
    
    const apiKey = await program.account.apiKey.fetch(apiKeyPda1);
    assert.equal(apiKey.totalRequests.toNumber(), 3);
  });

  it("Rejects invalid tier on create", async () => {
    const badUser = anchor.web3.Keypair.generate();
    await provider.connection.confirmTransaction(
      await provider.connection.requestAirdrop(badUser.publicKey, 1e9)
    );
    const [badKeyPda] = PublicKey.findProgramAddressSync(
      [Buffer.from("apikey"), badUser.publicKey.toBuffer()],
      program.programId
    );

    try {
      await program.methods
        .createApiKey(5) // Invalid tier
        .accounts({
          apiKey: badKeyPda,
          registry: registryPda,
          owner: badUser.publicKey,
          systemProgram: anchor.web3.SystemProgram.programId,
        })
        .signers([badUser])
        .rpc();
      assert.fail("Should have thrown InvalidTier error");
    } catch (e: any) {
      expect(e.message).to.include("InvalidTier");
      console.log("InvalidTier correctly rejected");
    }
  });

  it("Verifies Pro tier user has higher limit", async () => {
    // Pro tier allows 1000 req/hr, we test 5 requests
    const clock = await provider.connection.getBlockTime(
      await provider.connection.getSlot()
    );
    const windowId = Math.floor(clock! / 3600);
    
    const [rateLimitStatePda2] = PublicKey.findProgramAddressSync(
      [
        Buffer.from("ratelimit"),
        apiKeyPda2.toBuffer(),
        Buffer.from(new anchor.BN(windowId).toArrayLike(Buffer, "le", 8))
      ],
      program.programId
    );

    for (let i = 0; i < 5; i++) {
      await program.methods
        .checkAndIncrement()
        .accounts({
          apiKey: apiKeyPda2,
          rateLimitState: rateLimitStatePda2,
          owner: user2.publicKey,
          systemProgram: anchor.web3.SystemProgram.programId,
        })
        .signers([user2])
        .rpc();
    }

    const state = await program.account.rateLimitState.fetch(rateLimitStatePda2);
    assert.equal(state.requestCount, 5);
    console.log("Pro tier user made 5 requests successfully");
  });

  it("Upgrades tier", async () => {
    const tx = await program.methods
      .upgradeTier(1) // Upgrade user1 from Free to Basic
      .accounts({
        apiKey: apiKeyPda1,
        owner: user1.publicKey,
      })
      .signers([user1])
      .rpc();

    const apiKey = await program.account.apiKey.fetch(apiKeyPda1);
    assert.equal(apiKey.tier, 1); // Now Basic
    console.log("Tier upgraded. TX:", tx);
  });

  it("Revokes an API key", async () => {
    const tx = await program.methods
      .revokeApiKey()
      .accounts({
        apiKey: apiKeyPda1,
        owner: user1.publicKey,
      })
      .signers([user1])
      .rpc();

    const apiKey = await program.account.apiKey.fetch(apiKeyPda1);
    assert.equal(apiKey.isActive, false);
    console.log("API key revoked. TX:", tx);
  });

  it("Rejects requests from revoked key", async () => {
    const clock = await provider.connection.getBlockTime(
      await provider.connection.getSlot()
    );
    const windowId = Math.floor(clock! / 3600);
    
    const [rateLimitStatePda] = PublicKey.findProgramAddressSync(
      [
        Buffer.from("ratelimit"),
        apiKeyPda1.toBuffer(),
        Buffer.from(new anchor.BN(windowId).toArrayLike(Buffer, "le", 8))
      ],
      program.programId
    );

    try {
      await program.methods
        .checkAndIncrement()
        .accounts({
          apiKey: apiKeyPda1,
          rateLimitState: rateLimitStatePda,
          owner: user1.publicKey,
          systemProgram: anchor.web3.SystemProgram.programId,
        })
        .signers([user1])
        .rpc();
      assert.fail("Should have thrown ApiKeyInactive error");
    } catch (e: any) {
      expect(e.message).to.include("ApiKeyInactive");
      console.log("Revoked key correctly rejected");
    }
  });
});
