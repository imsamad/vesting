use anchor_lang::prelude::*;
use anchor_spl::associated_token::AssociatedToken;
use anchor_spl::token_interface::{
    transfer_checked, Mint, TokenAccount, TokenInterface, TransferChecked,
};

declare_id!("3aJgLn73D9ApLEcmC7kGJ8QJJyz2VyKEoY1x9fNmuFhV");

#[program]
pub mod token_vesting {

    use super::*;

    pub fn create_vesting_account(
        ctx: Context<CreateVestingAccount>,
        company_name: String,
    ) -> Result<()> {
        ctx.accounts.vesting_account.set_inner(VestingAccount {
            bump: ctx.bumps.vesting_account,
            treasury_bump: ctx.bumps.treasury_token_account,
            mint: ctx.accounts.mint.key(),
            owner: ctx.accounts.signer.key(),
            treasury_token_account: ctx.accounts.treasury_token_account.key(),
            company_name,
        });
        Ok(())
    }

    pub fn create_employee_account(
        ctx: Context<CreateEmployeeAccount>,
        start_time: i64,
        cliff_time: i64,
        end_time: i64,
        total_amount: i64,
    ) -> Result<()> {
        ctx.accounts.employee_account.set_inner(EmployeeAccount {
            // beneficiary: ctx.accounts.beneficiary.to_account_info().key,
            bump: ctx.bumps.employee_account,
            vesting_account: ctx.accounts.vesting_account.key(),
            start_time,
            cliff_time,
            end_time,
            total_amount,
            total_withdrawn: 0,
            beneficiary: ctx.accounts.beneficiary.key(),
        });

        Ok(())
    }

    pub fn claim_token(ctx: Context<ClaimToken>, _company_name: String) -> Result<()> {
        let employee_account = &mut ctx.accounts.employee_account;

        let now = Clock::get()?.unix_timestamp;

        if now < employee_account.cliff_time {
            return Err(MyError::ClaimNotAvailable.into());
        }
        let time_since_start = now.saturating_sub(employee_account.start_time);

        let total_vesting_time = employee_account
            .start_time
            .saturating_sub(employee_account.start_time);

        if total_vesting_time == 0 {
            return Err(MyError::InvalidVestingPeriod.into());
        }

        let vested_amount = if now >= employee_account.end_time {
            employee_account.total_amount
        } else {
            (employee_account.total_amount * time_since_start) / total_vesting_time

            // return match employee_account
            //     .total_amount
            //     .checked_mul(time_since_start as u64)
            // {
            //     Some(product) => product / (total_vesting_time as u64),
            //     None => {
            //         return Err(MyError::CalculationOverflow.into());
            //     }
            // };
        };

        let claimable_amount = vested_amount.saturating_sub(employee_account.total_amount);

        if claimable_amount == 0 {
            return Err(MyError::NothingToClaim.into());
        }

        let transfer_accounts = TransferChecked {
            from: ctx.accounts.treasury_token_account.to_account_info(),
            to: ctx.accounts.employee_token_account.to_account_info(),
            authority: ctx.accounts.beneficiary.to_account_info(),
            mint: ctx.accounts.mint.to_account_info(),
        };
        let signer_seeds: &[&[&[u8]]] = &[&[
            b"vesting_treasury",
            ctx.accounts.vesting_account.company_name.as_ref(),
            &[ctx.accounts.vesting_account.treasury_bump],
        ]];

        let cpi_ctx = CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            transfer_accounts,
        )
        .with_signer(signer_seeds);

        transfer_checked(cpi_ctx, claimable_amount as u64, ctx.accounts.mint.decimals)?;
        employee_account.total_amount += claimable_amount;

        Ok(())
    }
}

#[error_code]
pub enum MyError {
    #[msg("Cliff time not reach")]
    ClaimNotAvailable,
    #[msg("Vesting period not valid")]
    InvalidVestingPeriod,
    #[msg("Vesting calculation overflowed")]
    CalculationOverflow,
    #[msg("You have nothing to claim")]
    NothingToClaim,
}

#[derive(Accounts)]
#[instruction(company_name: String)]
pub struct CreateVestingAccount<'info> {
    #[account(mut)]
    pub signer: Signer<'info>,

    #[account(
        init,
        space = 8 + VestingAccount::INIT_SPACE,
        payer = signer,
        seeds = [b"vesting_account",company_name.as_bytes()],
        bump
    )]
    pub vesting_account: Account<'info, VestingAccount>,

    pub mint: InterfaceAccount<'info, Mint>,

    #[account(
        init,
        token::mint = mint,
        token::authority = treasury_token_account,
        payer = signer,
        seeds = [b"treasury_token", company_name.as_bytes()],
        bump
    )]
    pub treasury_token_account: InterfaceAccount<'info, TokenAccount>,

    pub token_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct CreateEmployeeAccount<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,

    // way to send wallet account
    pub beneficiary: SystemAccount<'info>,

    #[account(
        has_one = owner,
    )]
    pub vesting_account: Account<'info, VestingAccount>,

    #[account(
        init,
        seeds = [b"employee_account", beneficiary.key().as_ref(), vesting_account.key().as_ref()],
        bump,
        space = 8 + EmployeeAccount::INIT_SPACE,
        payer = owner,
    )]
    pub employee_account: Account<'info, EmployeeAccount>,

    pub system_program: Program<'info, System>,
}

#[account]
#[derive(InitSpace)]
pub struct VestingAccount {
    pub owner: Pubkey,
    pub mint: Pubkey,
    pub treasury_token_account: Pubkey,
    #[max_len(50)]
    pub company_name: String,
    pub treasury_bump: u8,
    pub bump: u8,
}

#[account]
#[derive(InitSpace)]
pub struct EmployeeAccount {
    pub beneficiary: Pubkey,
    pub cliff_time: i64,
    pub start_time: i64,
    pub end_time: i64,
    pub total_amount: i64,
    pub total_withdrawn: i64,

    pub vesting_account: Pubkey,
    pub bump: u8,
}

#[derive(Accounts)]
#[instruction(company_name: String)]
pub struct ClaimToken<'info> {
    #[account(mut)]
    pub beneficiary: Signer<'info>,

    #[account(
        mut,
        has_one = beneficiary,
        has_one = vesting_account,
        seeds = [b"employee_account", beneficiary.key().as_ref(), vesting_account.key().as_ref()],
        bump = employee_account.bump,
    )]
    pub employee_account: Account<'info, EmployeeAccount>,

    pub mint: InterfaceAccount<'info, Mint>,

    #[account(
        seeds = [b"vesting_account",company_name.as_bytes()],
        bump = vesting_account.bump,
        has_one = treasury_token_account,
    )]
    pub vesting_account: Account<'info, VestingAccount>,

    #[account(mut)]
    pub treasury_token_account: InterfaceAccount<'info, TokenAccount>,

    #[account(
        init_if_needed,
        payer = beneficiary,
        associated_token::mint = mint,
        associated_token::authority = beneficiary,
        associated_token::token_program = token_program,
    )]
    pub employee_token_account: InterfaceAccount<'info, TokenAccount>,

    pub token_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,
    pub associated_token_program: Program<'info, AssociatedToken>,
}
