use anchor_lang::prelude::*;

pub mod error;
pub mod instructions;

pub use instructions::*;

declare_id!("dgTKnP1RW7MuAo8NDcWCRZ1uJu4Jfzmbi4LQQe8q411");

#[program]
pub mod solana_level_1_token_starter {
    use super::*;

    pub fn create_token(ctx: Context<CreateToken>, decimals: u8) -> Result<()> {
        instructions::create_token::handler(ctx, decimals)
    }

    pub fn create_token_account(ctx: Context<CreateTokenAccount>) -> Result<()> {
        instructions::create_token_account::handler(ctx)
    }

    pub fn mint_tokens(ctx: Context<MintTokens>, amount: u64) -> Result<()> {
        instructions::mint_tokens::handler(ctx, amount)
    }

    pub fn initialize_escrow(ctx: Context<InitializeEscrow>, seed: u64, amount: u64) -> Result<()> {
        instructions::escrow::initialize_escrow(ctx, seed, amount)
    }
    pub fn release_escrow(ctx: Context<FinalizeEscrow>) -> Result<()> {
        instructions::escrow::release_escrow(ctx)
    }
    pub fn cancel_escrow(ctx: Context<FinalizeEscrow>) -> Result<()> {
        instructions::escrow::cancel_escrow(ctx)
    }
    pub fn transfer_tokens(ctx: Context<TransferTokens>, amount: u64) -> Result<()> {
        instructions::transfer_tokens::handler(ctx, amount)
    }
}
