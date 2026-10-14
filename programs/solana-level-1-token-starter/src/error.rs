use anchor_lang::prelude::*;

#[error_code]
pub enum TokenStarterError {
#[msg("Escrow amount must be positive")] InvalidAmount,
    #[msg("Escrow is already finalized")] Finalized,
    #[msg("Only the maker may cancel this escrow")] Unauthorized,
    #[msg("Token destination does not match the committed account")] WrongDestination,
    #[msg("Escrow vault balance is not equal to committed amount")] WrongBalance,
    #[msg("Maker and taker must be different")] SameParty,

    #[msg("Amount must be greater than zero")]
    AmountMustBePositive,
    #[msg("Source and destination token accounts must be different")]
    SourceEqualsDestination,
}
