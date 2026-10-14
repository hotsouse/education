use anchor_lang::{InstructionData, ToAccountMetas};
use anchor_spl::token_2022::spl_token_2022::{
    extension::StateWithExtensions,
    state::{Account as SplTokenAccount, Mint as SplMint},
};
use litesvm::LiteSVM;
use solana_keypair::Keypair;
use solana_message::{Instruction, Message};
use solana_signer::Signer;
use solana_transaction::Transaction;
use std::{fs, path::PathBuf};

const DECIMALS: u8 = 6;
const INITIAL_SUPPLY: u64 = 10_000_000;
const BURN_AMOUNT: u64 = 2_500_000;

fn program_bytes() -> Vec<u8> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/deploy/solana_level_1_token_starter.so");
    fs::read(&path).unwrap_or_else(|e| panic!("Run `anchor build` first ({}): {e}", path.display()))
}

fn instruction<A: ToAccountMetas, D: InstructionData>(accounts: A, data: D) -> Instruction {
    Instruction {
        program_id: solana_level_1_token_starter::ID,
        accounts: accounts.to_account_metas(None),
        data: data.data(),
    }
}

fn send(svm: &mut LiteSVM, payer: &Keypair, other_signers: &[&Keypair], ix: Instruction) -> bool {
    let message = Message::new(&[ix], Some(&payer.pubkey()));
    let mut signers = vec![payer];
    signers.extend_from_slice(other_signers);
    let tx = Transaction::new(&signers, message, svm.latest_blockhash());
    svm.send_transaction(tx).is_ok()
}

struct Fixture {
    svm: LiteSVM,
    payer: Keypair,
    authority: Keypair,
    mint: Keypair,
    source: anchor_lang::prelude::Pubkey,
}

impl Fixture {
    fn new() -> Self {
        let mut svm = LiteSVM::new();
        svm.add_program(solana_level_1_token_starter::ID, &program_bytes())
            .expect("load program");
        let payer = Keypair::new();
        let authority = Keypair::new();
        let mint = Keypair::new();
        let token_program = anchor_spl::token_2022::ID;
        svm.airdrop(&payer.pubkey(), 10_000_000_000).expect("fund payer");

        let create_mint = instruction(
            solana_level_1_token_starter::accounts::CreateToken {
                payer: payer.pubkey(),
                authority: authority.pubkey(),
                mint: mint.pubkey(),
                token_program,
                system_program: anchor_lang::system_program::ID,
            },
            solana_level_1_token_starter::instruction::CreateToken { decimals: DECIMALS },
        );
        assert!(send(&mut svm, &payer, &[&authority, &mint], create_mint));

        let source = anchor_spl::associated_token::get_associated_token_address_with_program_id(
            &authority.pubkey(), &mint.pubkey(), &token_program,
        );
        let create_account = instruction(
            solana_level_1_token_starter::accounts::CreateTokenAccount {
                payer: payer.pubkey(),
                owner: authority.pubkey(),
                mint: mint.pubkey(),
                token_account: source,
                token_program,
                associated_token_program: anchor_spl::associated_token::ID,
                system_program: anchor_lang::system_program::ID,
            },
            solana_level_1_token_starter::instruction::CreateTokenAccount {},
        );
        assert!(send(&mut svm, &payer, &[], create_account));

        let mint_tokens = instruction(
            solana_level_1_token_starter::accounts::MintTokens {
                authority: authority.pubkey(),
                mint: mint.pubkey(),
                destination: source,
                token_program,
            },
            solana_level_1_token_starter::instruction::MintTokens { amount: INITIAL_SUPPLY },
        );
        assert!(send(&mut svm, &payer, &[&authority], mint_tokens));

        let fixture = Self { svm, payer, authority, mint, source };
        assert_eq!(fixture.state(), (INITIAL_SUPPLY, INITIAL_SUPPLY));
        fixture
    }

    fn state(&self) -> (u64, u64) {
        let mint_data = self.svm.get_account(&self.mint.pubkey()).unwrap().data;
        let token_data = self.svm.get_account(&self.source).unwrap().data;
        let supply = StateWithExtensions::<SplMint>::unpack(&mint_data).unwrap().base.supply;
        let balance = StateWithExtensions::<SplTokenAccount>::unpack(&token_data).unwrap().base.amount;
        (balance, supply)
    }

    fn burn_ix(&self, authority: anchor_lang::prelude::Pubkey, mint: anchor_lang::prelude::Pubkey, token_program: anchor_lang::prelude::Pubkey, amount: u64) -> Instruction {
        instruction(
            solana_level_1_token_starter::accounts::BurnTokens {
                authority,
                mint,
                source: self.source,
                token_program,
            },
            solana_level_1_token_starter::instruction::BurnTokens { amount },
        )
    }

    fn assert_failed_unchanged(&mut self, ix: Instruction, sign_authority: bool, external_signer: Option<&Keypair>) {
        let before = self.state();
        let mint_before = self.svm.get_account(&self.mint.pubkey()).unwrap();
        let source_before = self.svm.get_account(&self.source).unwrap();
        let mut signers: Vec<&Keypair> = Vec::new();
        if sign_authority { signers.push(&self.authority); }
        if let Some(signer) = external_signer { signers.push(signer); }
        assert!(!send(&mut self.svm, &self.payer, &signers, ix), "burn must fail");
        assert_eq!(self.state(), before, "balance and supply must not change");
        let mint_after = self.svm.get_account(&self.mint.pubkey()).unwrap();
        let source_after = self.svm.get_account(&self.source).unwrap();
        assert_eq!(mint_after.data, mint_before.data, "mint data changed on failure");
        assert_eq!(source_after.data, source_before.data, "token account data changed on failure");
        assert_eq!(mint_after.lamports, mint_before.lamports);
        assert_eq!(source_after.lamports, source_before.lamports);
    }
}

#[test]
fn successful_burn_reduces_balance_and_supply_equally() {
    let mut f = Fixture::new();
    let before = f.state();
    let ix = f.burn_ix(f.authority.pubkey(), f.mint.pubkey(), anchor_spl::token_2022::ID, BURN_AMOUNT);
    let message = Message::new(&[ix], Some(&f.payer.pubkey()));
    let tx = Transaction::new(&[&f.payer, &f.authority], message, f.svm.latest_blockhash());
    let result = f.svm.send_transaction(tx);
    assert!(result.is_ok(), "successful burn failed: {result:#?}");
    let after = f.state();
    assert_eq!(after.0, before.0 - BURN_AMOUNT);
    assert_eq!(after.1, before.1 - BURN_AMOUNT);
    assert_eq!(before.0 - after.0, before.1 - after.1);
}

#[test]
fn zero_amount_is_rejected_without_mutation() {
    let mut f = Fixture::new();
    let ix = f.burn_ix(f.authority.pubkey(), f.mint.pubkey(), anchor_spl::token_2022::ID, 0);
    // Program-level require! emits AmountMustBePositive (Anchor error 6000).
    f.assert_failed_unchanged(ix, true, None);
}

#[test]
fn wrong_authority_is_rejected_without_mutation() {
    let mut f = Fixture::new();
    let attacker = Keypair::new();
    let ix = f.burn_ix(attacker.pubkey(), f.mint.pubkey(), anchor_spl::token_2022::ID, BURN_AMOUNT);
    f.assert_failed_unchanged(ix, false, Some(&attacker));
}

#[test]
fn missing_authority_signature_is_rejected_without_mutation() {
    let mut f = Fixture::new();
    let mut ix = f.burn_ix(f.authority.pubkey(), f.mint.pubkey(), anchor_spl::token_2022::ID, BURN_AMOUNT);
    // Mark the authority as a non-signer to exercise Anchor Signer validation.
    ix.accounts[0].is_signer = false;
    f.assert_failed_unchanged(ix, false, None);
}

#[test]
fn wrong_mint_is_rejected_without_mutation() {
    let mut f = Fixture::new();
    let another_mint = Keypair::new();
    let ix_create = instruction(
        solana_level_1_token_starter::accounts::CreateToken {
            payer: f.payer.pubkey(), authority: f.authority.pubkey(), mint: another_mint.pubkey(),
            token_program: anchor_spl::token_2022::ID,
            system_program: anchor_lang::system_program::ID,
        },
        solana_level_1_token_starter::instruction::CreateToken { decimals: DECIMALS },
    );
    assert!(send(&mut f.svm, &f.payer, &[&f.authority, &another_mint], ix_create));
    let ix = f.burn_ix(f.authority.pubkey(), another_mint.pubkey(), anchor_spl::token_2022::ID, BURN_AMOUNT);
    f.assert_failed_unchanged(ix, true, None);
}

#[test]
fn insufficient_balance_is_rejected_without_mutation() {
    let mut f = Fixture::new();
    let ix = f.burn_ix(f.authority.pubkey(), f.mint.pubkey(), anchor_spl::token_2022::ID, INITIAL_SUPPLY + 1);
    f.assert_failed_unchanged(ix, true, None);
}

#[test]
fn wrong_token_program_is_rejected_without_mutation() {
    let mut f = Fixture::new();
    let ix = f.burn_ix(f.authority.pubkey(), f.mint.pubkey(), anchor_spl::token::ID, BURN_AMOUNT);
    f.assert_failed_unchanged(ix, true, None);
}
