use pinocchio::{
    program_error::ProgramError,
    sysvars::clock::{self, Clock},
    ProgramResult,
};

use crate::PERPETUAL_NEW_WARMUP_COOLDOWN_RATE_EPOCH;

use super::{Delegation, Meta, Stake, StakeFlags, StakeHistorySysvar, StakeStateV2};

#[derive(Debug, PartialEq)]
// TODO: probaby can be optimized using references and/or change to struct
pub enum MergeKind {
    Inactive(Meta, u64, StakeFlags),
    ActivationEpoch(Meta, Stake, StakeFlags),
    FullyActive(Meta, Stake),
}

impl MergeKind {
    pub(crate) fn meta(&self) -> &Meta {
        match self {
            MergeKind::Inactive(meta, _, _) => meta,
            MergeKind::ActivationEpoch(meta, _, _) => meta,
            MergeKind::FullyActive(meta, _) => meta,
        }
    }

    fn active_stake(&self) -> Option<&Stake> {
        match self {
            Self::Inactive(_, _, _) => None,
            Self::ActivationEpoch(_, stake, _) => Some(stake),
            Self::FullyActive(_, stake) => Some(stake),
        }
    }

    pub(crate) fn get_if_mergeable(
        stake_state: &StakeStateV2,
        stake_lamports: u64,
        clock: &Clock,
        stake_history: &StakeHistorySysvar,
    ) -> Result<Self, ProgramError> {
        match stake_state {
            StakeStateV2::Stake(meta, stake, stake_flags) => {
                // stake must not be in a transient state. Transient here meaning
                // activating or deactivating with non-zero effective stake.
                let status = stake.delegation.stake_activating_and_deactivating(
                    clock.epoch,
                    stake_history,
                    PERPETUAL_NEW_WARMUP_COOLDOWN_RATE_EPOCH,
                );

                match (status.effective, status.activating, status.deactivating) {
                    (0, 0, 0) => Ok(Self::Inactive(
                        meta.clone(),
                        stake_lamports,
                        stake_flags.clone(),
                    )),
                    (0, _, _) => Ok(Self::ActivationEpoch(
                        meta.clone(),
                        stake.clone(),
                        stake_flags.clone(),
                    )),
                    (_, 0, 0) => Ok(Self::FullyActive(meta.clone(), stake.clone())),
                    _ => {
                        // let err = StakeError::MergeTransientStake;
                        // msg!("{}", err);
                        #[cfg(feature = "logging")]
                        pinocchio::msg!("stake account with transient stake cannot be merged");
                        Err(ProgramError::Custom(5))
                    }
                }
            }
            StakeStateV2::Initialized(meta) => Ok(Self::Inactive(
                meta.clone(),
                stake_lamports,
                StakeFlags::empty(),
            )),
            _ => Err(ProgramError::InvalidAccountData),
        }
    }

    pub(crate) fn merge(
        self,
        source: Self,
        clock: &Clock,
    ) -> Result<Option<StakeStateV2>, ProgramError> {
        Self::metas_can_merge(self.meta(), source.meta(), clock)?;
        self.active_stake()
            .zip(source.active_stake())
            .map(|(stake, source)| {
                Self::active_delegations_can_merge(&stake.delegation, &source.delegation)
            })
            .unwrap_or(Ok(()))?;
        let merged_state = match (self, source) {
            (Self::Inactive(_, _, _), Self::Inactive(_, _, _)) => None,
            (Self::Inactive(_, _, _), Self::ActivationEpoch(_, _, _)) => None,
            (
                Self::ActivationEpoch(meta, mut stake, stake_flags),
                Self::Inactive(_, source_lamports, source_stake_flags),
            ) => {
                stake.delegation.stake = u64::from(stake.delegation.stake)
                    .checked_add(source_lamports)
                    .ok_or(ProgramError::InsufficientFunds)?
                    .into();
                Some(StakeStateV2::Stake(
                    meta,
                    stake,
                    stake_flags.union(source_stake_flags),
                ))
            }
            (
                Self::ActivationEpoch(meta, mut stake, stake_flags),
                Self::ActivationEpoch(source_meta, source_stake, source_stake_flags),
            ) => {
                // let source_lamports = checked_add(
                //     source_meta.rent_exempt_reserve,
                //     source_stake.delegation.stake,
                // )?;
                let source_lamports = u64::from(source_meta.rent_exempt_reserve)
                    .checked_add(source_stake.delegation.stake.into())
                    .ok_or(ProgramError::ArithmeticOverflow)?;
                merge_delegation_stake_and_credits_observed(
                    &mut stake,
                    source_lamports,
                    source_stake.credits_observed.into(),
                )?;
                Some(StakeStateV2::Stake(
                    meta,
                    stake,
                    stake_flags.union(source_stake_flags),
                ))
            }
            (Self::FullyActive(meta, mut stake), Self::FullyActive(_, source_stake)) => {
                // Don't stake the source account's `rent_exempt_reserve` to
                // protect against the magic activation loophole. It will
                // instead be moved into the destination account as extra,
                // withdrawable `lamports`
                merge_delegation_stake_and_credits_observed(
                    &mut stake,
                    source_stake.delegation.stake.into(),
                    source_stake.credits_observed.into(),
                )?;
                Some(StakeStateV2::Stake(meta, stake, StakeFlags::empty()))
            }
            // _ => return Err(StakeError::MergeMismatch.into()),
            _ => return Err(ProgramError::Custom(6)),
        };
        Ok(merged_state)
    }

    pub(crate) fn metas_can_merge(stake: &Meta, source: &Meta, clock: &Clock) -> ProgramResult {
        // lockups may mismatch so long as both have expired
        let can_merge_lockups = stake.lockup == source.lockup
            || (!stake.lockup.is_in_force(clock, None) && !source.lockup.is_in_force(clock, None));
        // `rent_exempt_reserve` has no bearing on the mergeability of accounts,
        // as the source account will be culled by runtime once the operation
        // succeeds. Considering it here would needlessly prevent merging stake
        // accounts with differing data lengths, which already exist in the wild
        // due to an SDK bug
        if stake.authorized == source.authorized && can_merge_lockups {
            Ok(())
        } else {
            #[cfg(feature = "logging")]
            pinocchio::msg!("Unable to merge due to metadata mismatch");
            // Err(StakeError::MergeMismatch.into())
            Err(ProgramError::Custom(6))
        }
    }

    fn active_delegations_can_merge(stake: &Delegation, source: &Delegation) -> ProgramResult {
        if stake.voter_pubkey != source.voter_pubkey {
            #[cfg(feature = "logging")]
            pinocchio::msg!("Unable to merge due to voter mismatch");
            // Err(StakeError::MergeMismatch.into())
            Err(ProgramError::Custom(6))
        } else if u64::from(stake.deactivation_epoch) == clock::Epoch::MAX
            && u64::from(source.deactivation_epoch) == clock::Epoch::MAX
        {
            Ok(())
        } else {
            #[cfg(feature = "logging")]
            pinocchio::msg!("Unable to merge due to stake deactivation");
            // Err(StakeError::MergeMismatch.into())
            Err(ProgramError::Custom(6))
        }
    }
}

pub fn merge_delegation_stake_and_credits_observed(
    stake: &mut Stake,
    absorbed_lamports: u64,
    absorbed_credits_observed: u64,
) -> ProgramResult {
    if u64::from(stake.credits_observed) != absorbed_credits_observed {
        stake.credits_observed =
            stake_weighted_credits_observed(stake, absorbed_lamports, absorbed_credits_observed)
                .ok_or(ProgramError::ArithmeticOverflow)?
                .into();
    }
    // stake.delegation.stake = checked_add(stake.delegation.stake, absorbed_lamports)?;
    stake.delegation.stake = u64::from(stake.delegation.stake)
        .checked_add(absorbed_lamports)
        .ok_or(ProgramError::InsufficientFunds)?
        .into();
    Ok(())
}

/// Calculate the effective credits observed for two stakes when merging
///
/// When merging two `ActivationEpoch` or `FullyActive` stakes, the credits
/// observed of the merged stake is the weighted average of the two stakes'
/// credits observed.
///
/// This is because we can derive the effective credits_observed by reversing
/// the staking rewards equation, _while keeping the rewards unchanged after
/// merge (i.e. strong requirement)_, like below:
///
/// a(N) => account, r => rewards, s => stake, c => credits:
/// assume:
///   a3 = merge(a1, a2)
/// then:
///   a3.s = a1.s + a2.s
///
/// Next, given:
///   aN.r = aN.c * aN.s (for every N)
/// finally:
///        a3.r = a1.r + a2.r
/// a3.c * a3.s = a1.c * a1.s + a2.c * a2.s
///        a3.c = (a1.c * a1.s + a2.c * a2.s) / (a1.s + a2.s)     // QED
///
/// (For this discussion, we omitted irrelevant variables, including distance
///  calculation against vote_account and point indirection.)
fn stake_weighted_credits_observed(
    stake: &Stake,
    absorbed_lamports: u64,
    absorbed_credits_observed: u64,
) -> Option<u64> {
    let total_stake = u128::from(u64::from(stake.delegation.stake).checked_add(absorbed_lamports)?);
    let stake_weighted_credits = u128::from(u64::from(stake.credits_observed))
        .checked_mul(u128::from(u64::from(stake.delegation.stake)))?;
    let absorbed_weighted_credits =
        u128::from(absorbed_credits_observed).checked_mul(u128::from(absorbed_lamports))?;
    // Discard fractional credits as a merge side-effect friction by taking
    // the ceiling, done by adding `denominator - 1` to the numerator.
    let total_weighted_credits = stake_weighted_credits
        .checked_add(absorbed_weighted_credits)?
        .checked_add(total_stake)?
        .checked_sub(1)?;
    u64::try_from(total_weighted_credits.checked_div(total_stake)?).ok()
}
