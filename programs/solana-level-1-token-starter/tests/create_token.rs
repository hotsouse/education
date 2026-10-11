use anchor_lang::{InstructionData, ToAccountMetas};
use anchor_lang::solana_program::program_pack::Pack;
use anchor_spl::token_2022::spl_token_2022::state::{Account as TokenState, Mint as MintState};
use litesvm::LiteSVM;
use solana_keypair::Keypair;
use solana_message::{Instruction, Message};
use solana_signer::Signer;
use solana_transaction::Transaction;
use std::{fs, path::PathBuf};
use anchor_spl::token_2022::spl_token_2022::extension::StateWithExtensions;
const DECIMALS: u8 = 6;
const INITIAL_MINT: u64 = 2_500_000;
const TRANSFER: u64 = 700_000;

fn program_bytes() -> Vec<u8> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/deploy/solana_level_1_token_starter.so");
    fs::read(&path).unwrap_or_else(|error| panic!(
        "Run `anchor build --ignore-keys` before `cargo test`. Cannot read {}: {error}",
        path.display()
    ))
}

fn ix<A: ToAccountMetas, D: InstructionData>(accounts: A, data: D) -> Instruction {
    Instruction {
        program_id: solana_level_1_token_starter::ID,
        accounts: accounts.to_account_metas(None),
        data: data.data(),
    }
}

struct Fixture {
    svm: LiteSVM,
    payer: Keypair,
    authority: Keypair,
    mint: Keypair,
}

impl Fixture {
    fn new() -> Self {
        let mut svm = LiteSVM::new();
        svm.add_program(solana_level_1_token_starter::ID, &program_bytes())
            .expect("Anchor program should load into LiteSVM");
        let payer = Keypair::new();
        let authority = Keypair::new();
        let mint = Keypair::new();
        svm.airdrop(&payer.pubkey(), 10_000_000_000).expect("fund payer");
        Self { svm, payer, authority, mint }
    }

    fn send(&mut self, instruction: Instruction, extra_signers: &[&Keypair]) -> Result<(), String> {
        let mut signers: Vec<&Keypair> = vec![&self.payer];
        for signer in extra_signers {
            if signer.pubkey() != self.payer.pubkey() {
                signers.push(signer);
            }
        }
        let message = Message::new(&[instruction], Some(&self.payer.pubkey()));
        let tx = Transaction::new(&signers, message, self.svm.latest_blockhash());
        self.svm.send_transaction(tx).map(|_| ()).map_err(|e| format!("{e:?}"))
    }

    fn create_mint(&mut self, mint: &Keypair, authority: &Keypair) {
        let instruction = ix(
            solana_level_1_token_starter::accounts::CreateToken {
                payer: self.payer.pubkey(),
                authority: authority.pubkey(),
                mint: mint.pubkey(),
                token_program: anchor_spl::token_2022::ID,
                system_program: anchor_lang::system_program::ID,
            },
            solana_level_1_token_starter::instruction::CreateToken { decimals: DECIMALS },
        );
        self.send(instruction, &[mint, authority]).expect("create Token-2022 mint");
    }

    fn create_ata(&mut self, mint: &Keypair, owner: &Keypair) -> anchor_lang::prelude::Pubkey {
        let ata = anchor_spl::associated_token::get_associated_token_address_with_program_id(
            &owner.pubkey(), &mint.pubkey(), &anchor_spl::token_2022::ID,
        );
        let instruction = ix(
            solana_level_1_token_starter::accounts::CreateTokenAccount {
                payer: self.payer.pubkey(),
                owner: owner.pubkey(),
                mint: mint.pubkey(),
                token_account: ata,
                token_program: anchor_spl::token_2022::ID,
                associated_token_program: anchor_spl::associated_token::ID,
                system_program: anchor_lang::system_program::ID,
            },
            solana_level_1_token_starter::instruction::CreateTokenAccount {},
        );
        self.send(instruction, &[]).expect("create Token-2022 associated token account");
        ata
    }

    fn mint_ix(&self, mint: &Keypair, authority: &Keypair, destination: anchor_lang::prelude::Pubkey, amount: u64) -> Instruction {
        ix(
            solana_level_1_token_starter::accounts::MintTokens {
                authority: authority.pubkey(), mint: mint.pubkey(), destination,
                token_program: anchor_spl::token_2022::ID,
            },
            solana_level_1_token_starter::instruction::MintTokens { amount },
        )
    }

    fn transfer_ix(&self, mint: &Keypair, authority: &Keypair, source: anchor_lang::prelude::Pubkey, destination: anchor_lang::prelude::Pubkey, amount: u64) -> Instruction {
        ix(
            solana_level_1_token_starter::accounts::TransferTokens {
                authority: authority.pubkey(), mint: mint.pubkey(), source, destination,
                token_program: anchor_spl::token_2022::ID,
            },
            solana_level_1_token_starter::instruction::TransferTokens { amount },
        )
    }

    fn mint_state(&self, mint: &Keypair) -> MintState {
        let account = self.svm.get_account(&mint.pubkey()).expect("mint exists");
        assert_eq!(account.owner, anchor_spl::token_2022::ID, "mint belongs to Token-2022");
        MintState::unpack(&account.data).expect("valid mint state")
    }

    fn token_state(&self, address: anchor_lang::prelude::Pubkey) -> TokenState {
        let account = self.svm.get_account(&address).expect("token account exists");
        assert_eq!(account.owner, anchor_spl::token_2022::ID, "ATA belongs to Token-2022");
        StateWithExtensions::<TokenState>::unpack(&account.data)
    .expect("valid Token-2022 token account state")
    .base
    }
}

#[test]
fn creates_token_2022_mint_with_correct_state() {
    let mut f = Fixture::new();
    let mint = Keypair::new();
    let authority = Keypair::new();
    f.create_mint(&mint, &authority);
    let state = f.mint_state(&mint);
    assert_eq!(state.decimals, DECIMALS);
    assert_eq!(state.mint_authority, anchor_lang::solana_program::program_option::COption::Some(authority.pubkey()));
    assert_eq!(state.freeze_authority, anchor_lang::solana_program::program_option::COption::Some(authority.pubkey()));
    assert_eq!(state.supply, 0);
    assert!(state.is_initialized);
}

#[test]
fn creates_token_2022_associated_account_with_correct_owner_mint_and_program() {
    let mut f = Fixture::new();
    let mint = Keypair::new();
    let authority = Keypair::new();
    let owner = Keypair::new();
    f.create_mint(&mint, &authority);
    let ata = f.create_ata(&mint, &owner);
    let state = f.token_state(ata);
    assert_eq!(state.owner, owner.pubkey());
    assert_eq!(state.mint, mint.pubkey());
    assert_eq!(state.amount, 0);
    assert_eq!(f.mint_state(&mint).supply, 0);
}

#[test]
fn minting_increases_destination_balance_and_total_supply() {
    let mut f = Fixture::new();
    let mint = Keypair::new();
    let authority = Keypair::new();
    let owner = Keypair::new();
    f.create_mint(&mint, &authority);
    let ata = f.create_ata(&mint, &owner);
    let before_balance = f.token_state(ata).amount;
    let before_supply = f.mint_state(&mint).supply;
    let instruction = f.mint_ix(&mint, &authority, ata, INITIAL_MINT);
    f.send(instruction, &[&authority]).expect("mint_tokens should succeed");
    assert_eq!(f.token_state(ata).amount, before_balance + INITIAL_MINT);
    assert_eq!(f.mint_state(&mint).supply, before_supply + INITIAL_MINT);
}

#[test]
fn transfer_checked_updates_both_balances_without_changing_supply() {
    let mut f = Fixture::new();
    let mint = Keypair::new();
    let authority = Keypair::new();
    let recipient = Keypair::new();
    f.create_mint(&mint, &authority);
    let source = f.create_ata(&mint, &authority);
    let destination = f.create_ata(&mint, &recipient);
    let mint_ix = f.mint_ix(&mint, &authority, source, INITIAL_MINT);
    f.send(mint_ix, &[&authority]).expect("fund source");
    let source_before = f.token_state(source).amount;
    let destination_before = f.token_state(destination).amount;
    let supply_before = f.mint_state(&mint).supply;
    let transfer_ix = f.transfer_ix(&mint, &authority, source, destination, TRANSFER);
    f.send(transfer_ix, &[&authority]).expect("transfer_checked should succeed");
    assert_eq!(f.token_state(source).amount, source_before - TRANSFER);
    assert_eq!(f.token_state(destination).amount, destination_before + TRANSFER);
    assert_eq!(f.mint_state(&mint).supply, supply_before);
}

#[test]
fn rejects_zero_mint_and_zero_transfer_without_mutating_state() {
    let mut f = Fixture::new();
    let mint = Keypair::new();
    let authority = Keypair::new();
    let recipient = Keypair::new();
    f.create_mint(&mint, &authority);
    let source = f.create_ata(&mint, &authority);
    let destination = f.create_ata(&mint, &recipient);
    let zero_mint = f.mint_ix(&mint, &authority, source, 0);
    assert!(f.send(zero_mint, &[&authority]).is_err());
    let zero_transfer = f.transfer_ix(&mint, &authority, source, destination, 0);
    assert!(f.send(zero_transfer, &[&authority]).is_err());
    assert_eq!(f.token_state(source).amount, 0);
    assert_eq!(f.token_state(destination).amount, 0);
    assert_eq!(f.mint_state(&mint).supply, 0);
}

#[test]
fn rejects_wrong_authority_for_mint_and_transfer() {
    let mut f = Fixture::new();
    let mint = Keypair::new();
    let authority = Keypair::new();
    let attacker = Keypair::new();
    let recipient = Keypair::new();
    f.create_mint(&mint, &authority);
    let source = f.create_ata(&mint, &authority);
    let destination = f.create_ata(&mint, &recipient);
    let funded = f.mint_ix(&mint, &authority, source, INITIAL_MINT);
    f.send(funded, &[&authority]).expect("fund source");
    let unauthorized_mint = f.mint_ix(&mint, &attacker, source, TRANSFER);
    assert!(f.send(unauthorized_mint, &[&attacker]).is_err());
    let unauthorized_transfer = f.transfer_ix(&mint, &attacker, source, destination, TRANSFER);
    assert!(f.send(unauthorized_transfer, &[&attacker]).is_err());
    assert_eq!(f.token_state(source).amount, INITIAL_MINT);
    assert_eq!(f.token_state(destination).amount, 0);
    assert_eq!(f.mint_state(&mint).supply, INITIAL_MINT);
}

#[test]
fn rejects_wrong_mint_for_minting_and_transfers() {
    let mut f = Fixture::new();
    let mint_a = Keypair::new();
    let mint_b = Keypair::new();
    let authority = Keypair::new();
    let recipient = Keypair::new();
    f.create_mint(&mint_a, &authority);
    f.create_mint(&mint_b, &authority);
    let source_a = f.create_ata(&mint_a, &authority);
    let destination_b = f.create_ata(&mint_b, &recipient);
    let wrong_mint_destination = f.mint_ix(&mint_a, &authority, destination_b, TRANSFER);
    assert!(f.send(wrong_mint_destination, &[&authority]).is_err());
    let funded = f.mint_ix(&mint_a, &authority, source_a, INITIAL_MINT);
    f.send(funded, &[&authority]).expect("fund source");
    let wrong_mint_transfer = f.transfer_ix(&mint_a, &authority, source_a, destination_b, TRANSFER);
    assert!(f.send(wrong_mint_transfer, &[&authority]).is_err());
    assert_eq!(f.token_state(source_a).amount, INITIAL_MINT);
    assert_eq!(f.token_state(destination_b).amount, 0);
    assert_eq!(f.mint_state(&mint_a).supply, INITIAL_MINT);
    assert_eq!(f.mint_state(&mint_b).supply, 0);
}

#[test]
fn rejects_same_source_and_destination() {
    let mut f = Fixture::new();
    let mint = Keypair::new();
    let authority = Keypair::new();
    f.create_mint(&mint, &authority);
    let source = f.create_ata(&mint, &authority);
    let funded = f.mint_ix(&mint, &authority, source, INITIAL_MINT);
    f.send(funded, &[&authority]).expect("fund source");
    let same_account = f.transfer_ix(&mint, &authority, source, source, TRANSFER);
    assert!(f.send(same_account, &[&authority]).is_err());
    assert_eq!(f.token_state(source).amount, INITIAL_MINT);
    assert_eq!(f.mint_state(&mint).supply, INITIAL_MINT);
}
