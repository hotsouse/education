use anchor_lang::{InstructionData, ToAccountMetas};
use anchor_spl::associated_token::get_associated_token_address_with_program_id;
use litesvm::LiteSVM;
use solana_keypair::Keypair;
use solana_message::{Instruction, Message};
use solana_signer::Signer;
use solana_transaction::Transaction;
use std::{fs, path::PathBuf};

use solana_level_1_token_starter::{accounts, instruction, ID};

const AMOUNT: u64 = 5_000_000;
const INITIAL: u64 = 20_000_000;

fn program_bytes() -> Vec<u8> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/deploy/solana_level_1_token_starter.so");
    fs::read(&path).expect("Run anchor build before cargo test")
}

fn ix<A: ToAccountMetas, D: InstructionData>(accounts: A, data: D) -> Instruction {
    Instruction { program_id: ID, accounts: accounts.to_account_metas(None), data: data.data() }
}

fn send(svm: &mut LiteSVM, payer: &Keypair, signers: &[&Keypair], instruction: Instruction) -> bool {
    let message = Message::new(&[instruction], Some(&payer.pubkey()));
    let tx = Transaction::new(signers, message, svm.latest_blockhash());
    svm.send_transaction(tx).is_ok()
}

struct Fixture {
    svm: LiteSVM,
    maker: Keypair,
    taker: Keypair,
    attacker: Keypair,
    mint: Keypair,
    maker_ata: anchor_lang::prelude::Pubkey,
    taker_ata: anchor_lang::prelude::Pubkey,
    attacker_ata: anchor_lang::prelude::Pubkey,
    escrow: anchor_lang::prelude::Pubkey,
    vault: anchor_lang::prelude::Pubkey,
}

impl Fixture {
    fn new() -> Self {
        let mut svm = LiteSVM::new();
        svm.add_program(ID, &program_bytes()).expect("load escrow program");
        let maker = Keypair::new();
        let taker = Keypair::new();
        let attacker = Keypair::new();
        let mint = Keypair::new();
        let token_program = anchor_spl::token_2022::ID;
        for signer in [&maker, &taker, &attacker] {
            svm.airdrop(&signer.pubkey(), 2_000_000_000).expect("fund signer");
        }
        let create = ix(accounts::CreateToken {
            payer: maker.pubkey(), authority: maker.pubkey(), mint: mint.pubkey(),
            token_program, system_program: anchor_lang::system_program::ID,
        }, instruction::CreateToken { decimals: 6 });
        assert!(send(&mut svm, &maker, &[&maker, &mint], create));
        let ata = |owner| get_associated_token_address_with_program_id(&owner, &mint.pubkey(), &token_program);
        let maker_ata = ata(maker.pubkey());
        let taker_ata = ata(taker.pubkey());
        let attacker_ata = ata(attacker.pubkey());
        for (owner, token_account) in [
            (maker.pubkey(), maker_ata), (taker.pubkey(), taker_ata), (attacker.pubkey(), attacker_ata),
        ] {
            let create_ata = ix(accounts::CreateTokenAccount {
                payer: maker.pubkey(), owner, mint: mint.pubkey(), token_account,
                token_program, associated_token_program: anchor_spl::associated_token::ID,
                system_program: anchor_lang::system_program::ID,
            }, instruction::CreateTokenAccount {});
            assert!(send(&mut svm, &maker, &[&maker], create_ata), "create ATA");
        }
        let mint_ix = ix(accounts::MintTokens {
            authority: maker.pubkey(), mint: mint.pubkey(), destination: maker_ata, token_program,
        }, instruction::MintTokens { amount: INITIAL });
        assert!(send(&mut svm, &maker, &[&maker], mint_ix), "mint tokens");
        let (escrow, _) = anchor_lang::prelude::Pubkey::find_program_address(
            &[b"escrow", maker.pubkey().as_ref(), &42_u64.to_le_bytes()], &ID);
        let (vault, _) = anchor_lang::prelude::Pubkey::find_program_address(
            &[b"vault", escrow.as_ref()], &ID);
        Self { svm, maker, taker, attacker, mint, maker_ata, taker_ata, attacker_ata, escrow, vault }
    }

    fn initialize(&mut self, amount: u64) -> bool {
        let ix = ix(accounts::InitializeEscrow {
            maker: self.maker.pubkey(), taker: self.taker.pubkey(), mint: self.mint.pubkey(),
            maker_source: self.maker_ata, refund_account: self.maker_ata,
            destination_account: self.taker_ata, escrow: self.escrow, vault: self.vault,
            token_program: anchor_spl::token_2022::ID,
            system_program: anchor_lang::system_program::ID,
        }, instruction::InitializeEscrow { seed: 42, amount });
        send(&mut self.svm, &self.maker, &[&self.maker], ix)
    }

    fn finalize(&mut self, release: bool, destination: anchor_lang::prelude::Pubkey, attacker_signs: bool) -> bool {
        let signer = if attacker_signs { &self.attacker } else { &self.maker };
        let accounts = accounts::FinalizeEscrow {
            maker: signer.pubkey(), escrow: self.escrow, vault: self.vault,
            mint: self.mint.pubkey(), destination,
            token_program: anchor_spl::token_2022::ID,
        };
        let data = if release { instruction::ReleaseEscrow {}.data() } else { instruction::CancelEscrow {}.data() };
        let ix = Instruction { program_id: ID, accounts: accounts.to_account_metas(None), data };
        send(&mut self.svm, signer, &[signer], ix)
    }

    fn balance(&self, address: anchor_lang::prelude::Pubkey) -> u64 {
        use anchor_spl::token_2022::spl_token_2022::{extension::StateWithExtensions, state::Account};
        let data = self.svm.get_account(&address).expect("token account exists").data;
        StateWithExtensions::<Account>::unpack(&data).expect("valid token account").base.amount
    }
}

#[test]
fn release_transfers_to_taker_and_closes_vault() {
    let mut f = Fixture::new();
    assert!(f.initialize(AMOUNT));
    assert_eq!(f.balance(f.vault), AMOUNT);
    assert_eq!(f.balance(f.maker_ata), INITIAL - AMOUNT);
    assert!(f.finalize(true, f.taker_ata, false));
    assert_eq!(f.balance(f.taker_ata), AMOUNT);
    assert!(f.svm.get_account(&f.vault).is_none(), "vault must be closed");
    assert!(!f.finalize(true, f.taker_ata, false), "double release rejected");
    assert!(!f.finalize(false, f.maker_ata, false), "cancel after release rejected");
}

#[test]
fn cancel_refunds_maker_and_closes_vault() {
    let mut f = Fixture::new();
    assert!(f.initialize(AMOUNT));
    assert!(f.finalize(false, f.maker_ata, false));
    assert_eq!(f.balance(f.maker_ata), INITIAL);
    assert_eq!(f.balance(f.taker_ata), 0);
    assert!(f.svm.get_account(&f.vault).is_none());
    assert!(!f.finalize(false, f.maker_ata, false), "double cancel rejected");
    assert!(!f.finalize(true, f.taker_ata, false), "release after cancel rejected");
}

#[test]
fn invalid_amount_and_duplicate_seed_are_rejected() {
    let mut f = Fixture::new();
    assert!(!f.initialize(0));
    assert!(f.initialize(AMOUNT));
    assert!(!f.initialize(AMOUNT), "duplicate escrow PDA rejected");
}

#[test]
fn unauthorized_signer_and_wrong_destination_are_rejected() {
    let mut f = Fixture::new();
    assert!(f.initialize(AMOUNT));
    assert!(!f.finalize(true, f.attacker_ata, false), "attacker destination rejected");
    assert!(!f.finalize(true, f.taker_ata, true), "wrong signer rejected");
    assert!(!f.finalize(false, f.taker_ata, false), "refund destination mismatch rejected");
    assert_eq!(f.balance(f.vault), AMOUNT);
    assert!(f.finalize(true, f.taker_ata, false), "valid release still works");
}
