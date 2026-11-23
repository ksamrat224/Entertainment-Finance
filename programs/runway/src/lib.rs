//! Runway is intentionally a devnet-only, non-custodial prototype. Self-reported
//! habit amounts drive game progress only; no instruction moves, mints, or prices
//! a financial asset.

use anchor_lang::prelude::*;
use ephemeral_rollups_sdk::{
    anchor::{commit, delegate, ephemeral, ephemeral_accounts},
    cpi::DelegateConfig,
    ephem::{FoldableIntentBuilder, MagicIntentBundleBuilder},
};
use session_keys::{session_auth_or, Session, SessionError, SessionTokenV2};

declare_id!("Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS");

const PROFILE_SEED: &[u8] = b"profile";
const RUN_SEED: &[u8] = b"run";
const SPONSOR_SEED: &[u8] = b"sponsor";
const LIVE_RUN_SEED: &[u8] = b"live-run";
const RECEIPT_SEED: &[u8] = b"receipt";
const ACHIEVEMENT_SEED: &[u8] = b"achievement";
const MIN_RACE_SECONDS: i64 = 60;
const MAX_CHECKINS: u8 = 8;
const MAX_SELF_REPORTED_CENTS: u64 = 100_000;

#[ephemeral]
#[program]
pub mod runway {
    use super::*;

    pub fn initialize_profile(ctx: Context<InitializeProfile>) -> Result<()> {
        ctx.accounts.profile.set_inner(UserProfile {
            authority: ctx.accounts.authority.key(),
            settled_runs: 0,
            current_streak: 0,
            bump: ctx.bumps.profile,
        });
        Ok(())
    }

    /// Creates only program state and a rent sponsor. It never receives a token
    /// deposit and the sponsor's remaining lamports are later returned to the user.
    pub fn open_run(ctx: Context<OpenRun>, run_id: u64) -> Result<()> {
        let now = Clock::get()?.unix_timestamp;
        ctx.accounts.daily_run.set_inner(DailyRun {
            authority: ctx.accounts.authority.key(),
            run_id,
            opened_at: now,
            started_at: 0,
            finalized_at: 0,
            score: 0,
            checkins: 0,
            phase: RunPhase::Opened,
            settled: false,
            bump: ctx.bumps.daily_run,
        });
        ctx.accounts.sponsor.set_inner(SessionSponsor {
            authority: ctx.accounts.authority.key(),
            daily_run: ctx.accounts.daily_run.key(),
            bump: ctx.bumps.sponsor,
        });
        Ok(())
    }

    /// Base-layer delegation. The client must query the router afterwards and use
    /// its returned ER FQDN rather than assuming a regional endpoint.
    pub fn delegate_daily_run(
        ctx: Context<DelegateDailyRun>,
        validator: Option<Pubkey>,
    ) -> Result<()> {
        let authority = daily_run_authority(&ctx.accounts.daily_run)?;
        require_keys_eq!(authority, ctx.accounts.authority.key(), RunwayError::Unauthorized);
        let authority_key = authority;
        ctx.accounts.delegate_daily_run(
            &ctx.accounts.authority,
            &[RUN_SEED, authority_key.as_ref(), &daily_run_id(&ctx.accounts.daily_run)?.to_le_bytes()],
            DelegateConfig { validator, ..Default::default() },
        )?;
        Ok(())
    }

    /// The sponsor is delegated independently because ER-only Magic Accounts need
    /// a delegated sponsor with lamports available on the ER.
    pub fn delegate_sponsor(ctx: Context<DelegateSponsor>, validator: Option<Pubkey>) -> Result<()> {
        let (authority, daily_run) = sponsor_identity(&ctx.accounts.sponsor)?;
        require_keys_eq!(authority, ctx.accounts.authority.key(), RunwayError::Unauthorized);
        ctx.accounts.delegate_sponsor(
            &ctx.accounts.authority,
            &[SPONSOR_SEED, daily_run.as_ref()],
            DelegateConfig { validator, ..Default::default() },
        )?;
        Ok(())
    }

    /// ER-only creation. `LiveRun` contains transient detail and is not a durable
    /// entitlement, receipt, or balance.
    pub fn start_live_run(ctx: Context<StartLiveRun>) -> Result<()> {
        let daily_run_key = ctx.accounts.daily_run.key();
        let run = &mut ctx.accounts.daily_run;
        require!(run.phase == RunPhase::Opened, RunwayError::InvalidPhase);
        let now = Clock::get()?.unix_timestamp;
        run.started_at = now;
        run.phase = RunPhase::Racing;

        let started_at = run.started_at;
        drop(run);
        ctx.accounts.create_ephemeral_live_run(LiveRun::SPACE as u32)?;
        let live = LiveRun {
            daily_run: daily_run_key,
            started_at,
            last_checkin_at: now,
            self_reported_total_cents: 0,
            event_count: 0,
            score: 0,
        };
        live.try_serialize(&mut &mut ctx.accounts.live_run.try_borrow_mut_data()?[..])?;
        Ok(())
    }

    /// A scoped session key may call this low-risk progression instruction, but it
    /// cannot settle, close a sponsor, or change any authority. Amounts are capped
    /// and have no financial effect.
    #[session_auth_or(
        ctx.accounts.daily_run.authority == ctx.accounts.authority.key(),
        SessionError::InvalidToken
    )]
    pub fn record_checkin(ctx: Context<RecordCheckin>, amount_cents: u64) -> Result<()> {
        require!(amount_cents > 0 && amount_cents <= MAX_SELF_REPORTED_CENTS, RunwayError::InvalidAmount);
        let now = Clock::get()?.unix_timestamp;
        let run = &mut ctx.accounts.daily_run;
        require!(run.phase == RunPhase::Racing, RunwayError::InvalidPhase);
        require!(run.checkins < MAX_CHECKINS, RunwayError::CheckinLimitReached);

        let live_info = ctx.accounts.live_run.to_account_info();
        let mut live_data = live_info.try_borrow_mut_data()?;
        let mut live = LiveRun::try_deserialize(&mut &live_data[..])?;
        require_keys_eq!(live.daily_run, run.key(), RunwayError::LiveRunMismatch);
        require!(now >= live.last_checkin_at, RunwayError::ClockMovedBackwards);

        live.event_count = live.event_count.checked_add(1).ok_or(RunwayError::MathOverflow)?;
        live.self_reported_total_cents = live
            .self_reported_total_cents
            .checked_add(amount_cents)
            .ok_or(RunwayError::MathOverflow)?;
        live.score = live.score.checked_add(score_for(amount_cents)?).ok_or(RunwayError::MathOverflow)?;
        live.last_checkin_at = now;
        live.try_serialize(&mut &mut live_data[..])?;

        run.checkins = run.checkins.checked_add(1).ok_or(RunwayError::MathOverflow)?;
        run.score = live.score;
        Ok(())
    }

    /// Wallet-authorized terminal ER step. It copies only the outcome summary into
    /// `DailyRun`, closes transient state, and returns its ER rent to the sponsor.
    pub fn finish_live_run(ctx: Context<FinishLiveRun>) -> Result<()> {
        let now = Clock::get()?.unix_timestamp;
        let run = &mut ctx.accounts.daily_run;
        require_keys_eq!(run.authority, ctx.accounts.authority.key(), RunwayError::Unauthorized);
        require!(run.phase == RunPhase::Racing, RunwayError::InvalidPhase);
        require!(now >= run.started_at.checked_add(MIN_RACE_SECONDS).ok_or(RunwayError::MathOverflow)?, RunwayError::RaceTooShort);

        let live_data = ctx.accounts.live_run.try_borrow_data()?;
        let live = LiveRun::try_deserialize(&mut &live_data[..])?;
        require_keys_eq!(live.daily_run, run.key(), RunwayError::LiveRunMismatch);
        run.score = live.score;
        run.finalized_at = now;
        run.phase = RunPhase::Finalized;
        drop(live_data);
        ctx.accounts.close_ephemeral_live_run()?;
        Ok(())
    }

    /// Runs on the ER after `finish_live_run`; the client observes the base-layer
    /// result before exposing the run as ready to settle.
    pub fn commit_and_undelegate_daily_run(ctx: Context<CommitAndUndelegateDailyRun>) -> Result<()> {
        MagicIntentBundleBuilder::new(
            ctx.accounts.authority.to_account_info(),
            ctx.accounts.magic_context.to_account_info(),
            ctx.accounts.magic_program.to_account_info(),
        )
        .commit_and_undelegate(&[ctx.accounts.daily_run.to_account_info()])
        .build_and_invoke()?;
        Ok(())
    }

    pub fn commit_and_undelegate_sponsor(ctx: Context<CommitAndUndelegateSponsor>) -> Result<()> {
        MagicIntentBundleBuilder::new(
            ctx.accounts.authority.to_account_info(),
            ctx.accounts.magic_context.to_account_info(),
            ctx.accounts.magic_program.to_account_info(),
        )
        .commit_and_undelegate(&[ctx.accounts.sponsor.to_account_info()])
        .build_and_invoke()?;
        Ok(())
    }

    /// Base-layer, wallet-only settlement. No market price, token transfer, mint,
    /// or payout participates in this instruction.
    pub fn settle_allocation(ctx: Context<SettleAllocation>, allocation: Allocation) -> Result<()> {
        require!(allocation.is_valid(), RunwayError::InvalidAllocation);
        let run = &mut ctx.accounts.daily_run;
        require_keys_eq!(run.authority, ctx.accounts.authority.key(), RunwayError::Unauthorized);
        require!(run.phase == RunPhase::Finalized && !run.settled, RunwayError::InvalidPhase);
        run.settled = true;
        run.phase = RunPhase::Settled;

        ctx.accounts.receipt.set_inner(AllocationReceipt {
            authority: ctx.accounts.authority.key(),
            daily_run: run.key(),
            score: run.score,
            allocation,
            settled_at: Clock::get()?.unix_timestamp,
        });
        let profile = &mut ctx.accounts.profile;
        profile.settled_runs = profile.settled_runs.checked_add(1).ok_or(RunwayError::MathOverflow)?;
        profile.current_streak = profile.current_streak.checked_add(1).ok_or(RunwayError::MathOverflow)?;
        let achievement = &mut ctx.accounts.achievement;
        achievement.authority = ctx.accounts.authority.key();
        achievement.completed_runs = achievement.completed_runs.checked_add(1).ok_or(RunwayError::MathOverflow)?;
        Ok(())
    }

    /// Only after base settlement and sponsor undelegation. This is the explicit
    /// refund/cleanup path for the temporary sponsor account.
    pub fn close_sponsor(_ctx: Context<CloseSponsor>) -> Result<()> { Ok(()) }
}

#[derive(Accounts)]
pub struct InitializeProfile<'info> {
    #[account(mut)] pub authority: Signer<'info>,
    #[account(init, payer = authority, space = 8 + UserProfile::SPACE, seeds = [PROFILE_SEED, authority.key().as_ref()], bump)]
    pub profile: Account<'info, UserProfile>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(run_id: u64)]
pub struct OpenRun<'info> {
    #[account(mut)] pub authority: Signer<'info>,
    #[account(mut, seeds = [PROFILE_SEED, authority.key().as_ref()], bump = profile.bump, has_one = authority)]
    pub profile: Account<'info, UserProfile>,
    #[account(init, payer = authority, space = 8 + DailyRun::SPACE, seeds = [RUN_SEED, authority.key().as_ref(), &run_id.to_le_bytes()], bump)]
    pub daily_run: Account<'info, DailyRun>,
    #[account(init, payer = authority, space = 8 + SessionSponsor::SPACE, seeds = [SPONSOR_SEED, daily_run.key().as_ref()], bump)]
    pub sponsor: Account<'info, SessionSponsor>,
    pub system_program: Program<'info, System>,
}

#[delegate]
#[derive(Accounts)]
pub struct DelegateDailyRun<'info> {
    #[account(mut)] pub authority: Signer<'info>,
    /// CHECK: Data is read manually before the delegation CPI changes ownership.
    #[account(mut, del)] pub daily_run: UncheckedAccount<'info>,
}

#[delegate]
#[derive(Accounts)]
pub struct DelegateSponsor<'info> {
    #[account(mut)] pub authority: Signer<'info>,
    /// CHECK: Data is read manually before the delegation CPI changes ownership.
    #[account(mut, del)] pub sponsor: UncheckedAccount<'info>,
}

#[ephemeral_accounts]
#[derive(Accounts)]
pub struct StartLiveRun<'info> {
    #[account(mut)] pub authority: Signer<'info>,
    #[account(mut, seeds = [RUN_SEED, authority.key().as_ref(), &daily_run.run_id.to_le_bytes()], bump = daily_run.bump, has_one = authority)]
    pub daily_run: Account<'info, DailyRun>,
    #[account(mut, sponsor, seeds = [SPONSOR_SEED, daily_run.key().as_ref()], bump = sponsor.bump, has_one = authority)]
    pub sponsor: Account<'info, SessionSponsor>,
    /// CHECK: Created, serialized, and closed only by generated ER helpers.
    #[account(mut, eph, seeds = [LIVE_RUN_SEED, daily_run.key().as_ref()], bump)]
    pub live_run: UncheckedAccount<'info>,
}

#[derive(Accounts, Session)]
pub struct RecordCheckin<'info> {
    pub authority: Signer<'info>,
    #[account(mut, seeds = [RUN_SEED, daily_run.authority.as_ref(), &daily_run.run_id.to_le_bytes()], bump = daily_run.bump)]
    pub daily_run: Account<'info, DailyRun>,
    #[account(mut, seeds = [LIVE_RUN_SEED, daily_run.key().as_ref()], bump)]
    pub live_run: Account<'info, LiveRun>,
    #[session(signer = authority, authority = daily_run.authority)]
    pub session_token: Option<Account<'info, SessionTokenV2>>,
}

#[ephemeral_accounts]
#[derive(Accounts)]
pub struct FinishLiveRun<'info> {
    #[account(mut)] pub authority: Signer<'info>,
    #[account(mut, seeds = [RUN_SEED, authority.key().as_ref(), &daily_run.run_id.to_le_bytes()], bump = daily_run.bump, has_one = authority)]
    pub daily_run: Account<'info, DailyRun>,
    #[account(mut, sponsor, seeds = [SPONSOR_SEED, daily_run.key().as_ref()], bump = sponsor.bump, has_one = authority)]
    pub sponsor: Account<'info, SessionSponsor>,
    /// CHECK: ER-only state closed by generated helper.
    #[account(mut, eph, seeds = [LIVE_RUN_SEED, daily_run.key().as_ref()], bump)]
    pub live_run: UncheckedAccount<'info>,
}

#[commit]
#[derive(Accounts)]
pub struct CommitAndUndelegateDailyRun<'info> {
    #[account(mut)] pub authority: Signer<'info>,
    #[account(mut, seeds = [RUN_SEED, authority.key().as_ref(), &daily_run.run_id.to_le_bytes()], bump = daily_run.bump, has_one = authority)]
    pub daily_run: Account<'info, DailyRun>,
}

#[commit]
#[derive(Accounts)]
pub struct CommitAndUndelegateSponsor<'info> {
    #[account(mut)] pub authority: Signer<'info>,
    #[account(mut, seeds = [SPONSOR_SEED, sponsor.daily_run.as_ref()], bump = sponsor.bump, has_one = authority)]
    pub sponsor: Account<'info, SessionSponsor>,
}

#[derive(Accounts)]
pub struct SettleAllocation<'info> {
    #[account(mut)] pub authority: Signer<'info>,
    #[account(mut, seeds = [PROFILE_SEED, authority.key().as_ref()], bump = profile.bump, has_one = authority)]
    pub profile: Account<'info, UserProfile>,
    #[account(mut, seeds = [RUN_SEED, authority.key().as_ref(), &daily_run.run_id.to_le_bytes()], bump = daily_run.bump, has_one = authority)]
    pub daily_run: Account<'info, DailyRun>,
    #[account(init, payer = authority, space = 8 + AllocationReceipt::SPACE, seeds = [RECEIPT_SEED, daily_run.key().as_ref()], bump)]
    pub receipt: Account<'info, AllocationReceipt>,
    #[account(init_if_needed, payer = authority, space = 8 + Achievement::SPACE, seeds = [ACHIEVEMENT_SEED, authority.key().as_ref()], bump)]
    pub achievement: Account<'info, Achievement>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct CloseSponsor<'info> {
    #[account(mut)] pub authority: Signer<'info>,
    #[account(seeds = [RUN_SEED, authority.key().as_ref(), &daily_run.run_id.to_le_bytes()], bump = daily_run.bump, has_one = authority, constraint = daily_run.settled @ RunwayError::InvalidPhase)]
    pub daily_run: Account<'info, DailyRun>,
    #[account(mut, close = authority, seeds = [SPONSOR_SEED, daily_run.key().as_ref()], bump = sponsor.bump, has_one = authority)]
    pub sponsor: Account<'info, SessionSponsor>,
}

#[account]
pub struct UserProfile { pub authority: Pubkey, pub settled_runs: u32, pub current_streak: u32, pub bump: u8 }
impl UserProfile { pub const SPACE: usize = 32 + 4 + 4 + 1; }

#[account]
pub struct SessionSponsor { pub authority: Pubkey, pub daily_run: Pubkey, pub bump: u8 }
impl SessionSponsor { pub const SPACE: usize = 32 + 32 + 1; }

#[account]
pub struct DailyRun {
    pub authority: Pubkey, pub run_id: u64, pub opened_at: i64, pub started_at: i64,
    pub finalized_at: i64, pub score: u64, pub checkins: u8, pub phase: RunPhase,
    pub settled: bool, pub bump: u8,
}
impl DailyRun { pub const SPACE: usize = 32 + 8 + 8 + 8 + 8 + 8 + 1 + 1 + 1 + 1; }

#[account]
pub struct LiveRun { pub daily_run: Pubkey, pub started_at: i64, pub last_checkin_at: i64, pub self_reported_total_cents: u64, pub event_count: u8, pub score: u64 }
impl LiveRun { pub const SPACE: usize = 32 + 8 + 8 + 8 + 1 + 8; }

#[account]
pub struct AllocationReceipt { pub authority: Pubkey, pub daily_run: Pubkey, pub score: u64, pub allocation: Allocation, pub settled_at: i64 }
impl AllocationReceipt { pub const SPACE: usize = 32 + 32 + 8 + Allocation::SPACE + 8; }

#[account]
pub struct Achievement { pub authority: Pubkey, pub completed_runs: u32 }
impl Achievement { pub const SPACE: usize = 32 + 4; }

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Default)]
pub struct Allocation { pub stability_bps: u16, pub growth_bps: u16, pub discovery_bps: u16 }
impl Allocation {
    pub const SPACE: usize = 2 + 2 + 2;
    pub fn is_valid(&self) -> bool { u32::from(self.stability_bps) + u32::from(self.growth_bps) + u32::from(self.discovery_bps) == 10_000 }
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq)]
pub enum RunPhase { Opened, Racing, Finalized, Settled }

#[error_code]
pub enum RunwayError {
    #[msg("The signer is not authorized for this run.")] Unauthorized,
    #[msg("The run is not in the required lifecycle phase.")] InvalidPhase,
    #[msg("Self-reported amounts must be within the demo limit.")] InvalidAmount,
    #[msg("The run has reached its check-in limit.")] CheckinLimitReached,
    #[msg("The live run is not associated with this daily run.")] LiveRunMismatch,
    #[msg("The race must run for at least 60 seconds.")] RaceTooShort,
    #[msg("Arithmetic overflow.")] MathOverflow,
    #[msg("Clock moved backwards.")] ClockMovedBackwards,
    #[msg("Allocation weights must equal 100%.")] InvalidAllocation,
}

fn daily_run_authority(account: &UncheckedAccount<'_>) -> Result<Pubkey> {
    let data = account.try_borrow_data()?;
    Ok(DailyRun::try_deserialize(&mut &data[..])?.authority)
}
fn daily_run_id(account: &UncheckedAccount<'_>) -> Result<u64> {
    let data = account.try_borrow_data()?;
    Ok(DailyRun::try_deserialize(&mut &data[..])?.run_id)
}
fn sponsor_identity(account: &UncheckedAccount<'_>) -> Result<(Pubkey, Pubkey)> {
    let data = account.try_borrow_data()?;
    let sponsor = SessionSponsor::try_deserialize(&mut &data[..])?;
    Ok((sponsor.authority, sponsor.daily_run))
}
fn score_for(amount_cents: u64) -> Result<u64> { amount_cents.checked_div(100).ok_or(RunwayError::MathOverflow)?.checked_add(10).ok_or(RunwayError::MathOverflow.into()) }

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn allocation_requires_exactly_one_hundred_percent() {
        assert!(Allocation { stability_bps: 5_000, growth_bps: 3_000, discovery_bps: 2_000 }.is_valid());
        assert!(!Allocation { stability_bps: 5_000, growth_bps: 3_001, discovery_bps: 2_000 }.is_valid());
    }
    #[test] fn score_is_bounded_integer_math() { assert_eq!(score_for(12_345).unwrap(), 133); }
}
