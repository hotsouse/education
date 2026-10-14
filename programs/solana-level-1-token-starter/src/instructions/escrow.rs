use anchor_lang::prelude::*;
use anchor_spl::token_interface::{self, CloseAccount, Mint, TokenAccount, TokenInterface, TransferChecked};

#[account]
pub struct Escrow {
    pub maker: Pubkey,
    pub taker: Pubkey,
    pub mint: Pubkey,
    pub refund_account: Pubkey,
    pub destination_account: Pubkey,
    pub vault_key: Pubkey,
    pub amount: u64,
    pub seed: u64,
    pub bump: u8,
    pub status: u8, // 0 = active, 1 = released, 2 = cancelled
}
impl Escrow { pub const SPACE: usize = 8 + 32 * 6 + 8 + 8 + 1 + 1; }

use crate::error::TokenStarterError as EscrowError;


#[derive(Accounts)]
#[instruction(seed: u64)]
pub struct InitializeEscrow<'info> {
    #[account(mut)] pub maker: Signer<'info>,
    /// CHECK: This address is recorded and matched to the release destination's owner.
    pub taker: UncheckedAccount<'info>,
    #[account(mint::token_program = token_program)] pub mint: Box<InterfaceAccount<'info, Mint>>,
    #[account(mut, token::mint = mint, token::authority = maker, token::token_program = token_program)]
    pub maker_source: Box<InterfaceAccount<'info, TokenAccount>>,
    #[account(token::mint = mint, token::authority = maker, token::token_program = token_program)]
    pub refund_account: Box<InterfaceAccount<'info, TokenAccount>>,
    #[account(token::mint = mint, token::authority = taker, token::token_program = token_program)]
    pub destination_account: Box<InterfaceAccount<'info, TokenAccount>>,
    #[account(init, payer = maker, space = Escrow::SPACE, seeds = [b"escrow", maker.key().as_ref(), &seed.to_le_bytes()], bump)]
    pub escrow: Box<Account<'info, Escrow>>,
    #[account(
        init,
        payer = maker,
        seeds = [b"vault", escrow.key().as_ref()],
        bump,
        token::mint = mint,
        token::authority = escrow,
        token::token_program = token_program
    )]
    pub vault: Box<InterfaceAccount<'info, TokenAccount>>,
    pub token_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,
}

pub fn initialize_escrow(ctx: Context<InitializeEscrow>, seed: u64, amount: u64) -> Result<()> {
    require!(amount > 0, EscrowError::InvalidAmount);
    require_keys_neq!(ctx.accounts.maker.key(), ctx.accounts.taker.key(), EscrowError::SameParty);
    let e = &mut ctx.accounts.escrow;
    e.maker = ctx.accounts.maker.key();
    e.taker = ctx.accounts.taker.key();
    e.mint = ctx.accounts.mint.key();
    e.refund_account = ctx.accounts.refund_account.key();
    e.destination_account = ctx.accounts.destination_account.key();
    e.vault_key = ctx.accounts.vault.key();
    e.amount = amount;
    e.seed = seed;
    e.bump = ctx.bumps.escrow;
    e.status = 0;
    token_interface::transfer_checked(
        CpiContext::new(ctx.accounts.token_program.key(), TransferChecked {
            from: ctx.accounts.maker_source.to_account_info(),
            mint: ctx.accounts.mint.to_account_info(),
            to: ctx.accounts.vault.to_account_info(),
            authority: ctx.accounts.maker.to_account_info(),
        }), amount, ctx.accounts.mint.decimals,
    )
}

#[derive(Accounts)]
pub struct FinalizeEscrow<'info> {
    #[account(mut)] pub maker: Signer<'info>,
    #[account(mut, seeds = [b"escrow", maker.key().as_ref(), &escrow.seed.to_le_bytes()], bump = escrow.bump,
        constraint = escrow.maker == maker.key() @ EscrowError::Unauthorized,
        constraint = escrow.status == 0 @ EscrowError::Finalized,
        constraint = escrow.mint == mint.key(),
        constraint = escrow.vault_key == vault.key(),
        constraint = vault.key() == Pubkey::find_program_address(
            &[b"vault", escrow.key().as_ref()],
            &crate::ID
        ).0)]
    pub escrow: Box<Account<'info, Escrow>>,
    #[account(mut, token::mint = mint, token::authority = escrow, token::token_program = token_program)]
    pub vault: Box<InterfaceAccount<'info, TokenAccount>>,
    #[account(mint::token_program = token_program)] pub mint: Box<InterfaceAccount<'info, Mint>>,
    #[account(mut, token::mint = mint, token::token_program = token_program)]
    pub destination: Box<InterfaceAccount<'info, TokenAccount>>,
    pub token_program: Interface<'info, TokenInterface>,
}

fn finalize(ctx: Context<FinalizeEscrow>, release: bool) -> Result<()> {
    let e = &ctx.accounts.escrow;
    let expected = if release { e.destination_account } else { e.refund_account };
    require_keys_eq!(ctx.accounts.destination.key(), expected, EscrowError::WrongDestination);
    require!(ctx.accounts.vault.amount == e.amount, EscrowError::WrongBalance);
    let maker_key = e.maker;
    let seed_bytes = e.seed.to_le_bytes();
    let bump_bytes = [e.bump];
    let signer_seeds: &[&[u8]] = &[b"escrow", maker_key.as_ref(), &seed_bytes, &bump_bytes];
    let signer = &[signer_seeds];
    token_interface::transfer_checked(CpiContext::new_with_signer(
        ctx.accounts.token_program.key(), TransferChecked {
            from: ctx.accounts.vault.to_account_info(),
            mint: ctx.accounts.mint.to_account_info(),
            to: ctx.accounts.destination.to_account_info(),
            authority: ctx.accounts.escrow.to_account_info(),
        }, signer), e.amount, ctx.accounts.mint.decimals)?;
    token_interface::close_account(CpiContext::new_with_signer(
        ctx.accounts.token_program.key(), CloseAccount {
            account: ctx.accounts.vault.to_account_info(),
            destination: ctx.accounts.maker.to_account_info(),
            authority: ctx.accounts.escrow.to_account_info(),
        }, signer))?;
    ctx.accounts.escrow.status = if release { 1 } else { 2 };
    Ok(())
}
pub fn release_escrow(ctx: Context<FinalizeEscrow>) -> Result<()> { finalize(ctx, true) }
pub fn cancel_escrow(ctx: Context<FinalizeEscrow>) -> Result<()> { finalize(ctx, false) }
