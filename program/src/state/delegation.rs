use pinocchio::{pubkey::Pubkey, sysvars::clock};

use super::{Epoch, PodF64, PodU64, StakeHistoryEntry, StakeHistorySysvar};

#[repr(C)]
#[derive(Debug, PartialEq, Clone)]
pub struct Delegation {
    /// to whom the stake is delegated
    pub voter_pubkey: Pubkey,
    /// activated stake amount, set at delegate() time
    pub stake: PodU64,
    /// epoch at which this stake was activated, std::Epoch::MAX if is a bootstrap stake
    pub activation_epoch: Epoch,
    /// epoch the stake was deactivated, std::Epoch::MAX if not deactivated
    pub deactivation_epoch: Epoch,
    /// how much stake we can activate per-epoch as a fraction of currently effective stake
    #[deprecated(
        since = "1.16.7",
        note = "Please use `solana_sdk::stake::state::warmup_cooldown_rate()` instead"
    )]
    pub warmup_cooldown_rate: PodF64,
}

pub const DEFAULT_WARMUP_COOLDOWN_RATE: f64 = 0.25;
pub const NEW_WARMUP_COOLDOWN_RATE: f64 = 0.09;

impl Default for Delegation {
    fn default() -> Self {
        #[allow(deprecated)]
        Self {
            voter_pubkey: Pubkey::default(),
            stake: 0u64.into(),
            activation_epoch: 0u64.into(),
            deactivation_epoch: u64::MAX.into(),
            warmup_cooldown_rate: DEFAULT_WARMUP_COOLDOWN_RATE.into(),
        }
    }
}

impl Delegation {
    pub fn stake_activating_and_deactivating(
        &self,
        target_epoch: clock::Epoch,
        history: &StakeHistorySysvar,
        new_rate_activation_epoch: Option<clock::Epoch>,
    ) -> StakeHistoryEntry {
        // first, calculate an effective and activating stake
        let (effective_stake, activating_stake) =
            self.stake_and_activating(target_epoch, history, new_rate_activation_epoch);

        let deactivation_epoch = self.deactivation_epoch.into();

        // then de-activate some portion if necessary
        if target_epoch < deactivation_epoch {
            // not deactivated
            if activating_stake == 0 {
                StakeHistoryEntry::with_effective(effective_stake)
            } else {
                StakeHistoryEntry::with_effective_and_activating(effective_stake, activating_stake)
            }
        } else if target_epoch == deactivation_epoch {
            // can only deactivate what's activated
            StakeHistoryEntry::with_deactivating(effective_stake)
        } else if let Some((history, mut prev_epoch, mut prev_cluster_stake)) = history
            .get_entry(deactivation_epoch)
            .map(|cluster_stake_at_deactivation_epoch| {
                (
                    history,
                    deactivation_epoch,
                    cluster_stake_at_deactivation_epoch,
                )
            })
        {
            // target_epoch > self.deactivation_epoch

            // loop from my deactivation epoch until the target epoch
            // current effective stake is updated using its previous epoch's cluster stake
            let mut current_epoch;
            let mut current_effective_stake = effective_stake;
            loop {
                current_epoch = prev_epoch + 1;
                // if there is no deactivating stake at prev epoch, we should have been
                // fully undelegated at this moment
                if prev_cluster_stake.deactivating == 0 {
                    break;
                }

                // I'm trying to get to zero, how much of the deactivation in stake
                //   this account is entitled to take
                let weight =
                    current_effective_stake as f64 / prev_cluster_stake.deactivating as f64;
                let warmup_cooldown_rate =
                    warmup_cooldown_rate(current_epoch, new_rate_activation_epoch);

                // portion of newly not-effective cluster stake I'm entitled to at current epoch
                let newly_not_effective_cluster_stake =
                    prev_cluster_stake.effective as f64 * warmup_cooldown_rate;
                let newly_not_effective_stake =
                    ((weight * newly_not_effective_cluster_stake) as u64).max(1);

                current_effective_stake =
                    current_effective_stake.saturating_sub(newly_not_effective_stake);
                if current_effective_stake == 0 {
                    break;
                }

                if current_epoch >= target_epoch {
                    break;
                }
                if let Some(current_cluster_stake) = history.get_entry(current_epoch) {
                    prev_epoch = current_epoch;
                    prev_cluster_stake = current_cluster_stake;
                } else {
                    break;
                }
            }

            // deactivating stake should equal to all of currently remaining effective stake
            StakeHistoryEntry::with_deactivating(current_effective_stake)
        } else {
            // no history or I've dropped out of history, so assume fully deactivated
            StakeHistoryEntry::default()
        }
    }

    // returned tuple is (effective, activating) stake
    fn stake_and_activating(
        &self,
        target_epoch: clock::Epoch,
        history: &StakeHistorySysvar,
        new_rate_activation_epoch: Option<clock::Epoch>,
    ) -> (u64, u64) {
        let delegated_stake = self.stake.into();
        let activation_epoch = self.activation_epoch.into();
        let deactivation_epoch = self.deactivation_epoch.into();

        if self.is_bootstrap() {
            // fully effective immediately
            (delegated_stake, 0)
        } else if self.activation_epoch == self.deactivation_epoch {
            // activated but instantly deactivated; no stake at all regardless of target_epoch
            // this must be after the bootstrap check and before all-is-activating check
            (0, 0)
        } else if target_epoch == activation_epoch {
            // all is activating
            (0, delegated_stake)
        } else if target_epoch < activation_epoch {
            // not yet enabled
            (0, 0)
        } else if let Some((history, mut prev_epoch, mut prev_cluster_stake)) = history
            .get_entry(activation_epoch)
            .map(|cluster_stake_at_activation_epoch| {
                (history, activation_epoch, cluster_stake_at_activation_epoch)
            })
        {
            // target_epoch > self.activation_epoch

            // loop from my activation epoch until the target epoch summing up my entitlement
            // current effective stake is updated using its previous epoch's cluster stake
            let mut current_epoch;
            let mut current_effective_stake = 0;
            loop {
                current_epoch = prev_epoch + 1;
                // if there is no activating stake at prev epoch, we should have been
                // fully effective at this moment
                if prev_cluster_stake.activating == 0 {
                    break;
                }

                // how much of the growth in stake this account is
                //  entitled to take
                let remaining_activating_stake = delegated_stake - current_effective_stake;
                let weight =
                    remaining_activating_stake as f64 / prev_cluster_stake.activating as f64;
                let warmup_cooldown_rate =
                    warmup_cooldown_rate(current_epoch, new_rate_activation_epoch);

                // portion of newly effective cluster stake I'm entitled to at current epoch
                let newly_effective_cluster_stake =
                    prev_cluster_stake.effective as f64 * warmup_cooldown_rate;
                let newly_effective_stake =
                    ((weight * newly_effective_cluster_stake) as u64).max(1);

                current_effective_stake += newly_effective_stake;
                if current_effective_stake >= delegated_stake {
                    current_effective_stake = delegated_stake;
                    break;
                }

                if current_epoch >= target_epoch || current_epoch >= deactivation_epoch {
                    break;
                }
                if let Some(current_cluster_stake) = history.get_entry(current_epoch) {
                    prev_epoch = current_epoch;
                    prev_cluster_stake = current_cluster_stake;
                } else {
                    break;
                }
            }

            (
                current_effective_stake,
                delegated_stake - current_effective_stake,
            )
        } else {
            // no history or I've dropped out of history, so assume fully effective
            (delegated_stake, 0)
        }
    }

    // Explanation: this is an optimized version of stake_activating_and_deactivating when only effective stake is needed
    pub fn get_effective_stake(
        &self,
        target_epoch: clock::Epoch,
        history: &StakeHistorySysvar,
        new_rate_activation_epoch: Option<clock::Epoch>,
    ) -> u64 {
        let effective_stake =
            self.get_effective_stake_inner(target_epoch, history, new_rate_activation_epoch);

        let deactivation_epoch = self.deactivation_epoch.into();

        if target_epoch <= deactivation_epoch {
            effective_stake
        } else if let Some((history, mut prev_epoch, mut prev_cluster_stake)) = history
            .get_entry(deactivation_epoch)
            .map(|cluster_stake_at_deactivation_epoch| {
                (
                    history,
                    deactivation_epoch,
                    cluster_stake_at_deactivation_epoch,
                )
            })
        {
            // target_epoch > self.deactivation_epoch

            // loop from my deactivation epoch until the target epoch
            // current effective stake is updated using its previous epoch's cluster stake
            let mut current_epoch;
            let mut current_effective_stake = effective_stake;
            loop {
                current_epoch = prev_epoch + 1;
                // if there is no deactivating stake at prev epoch, we should have been
                // fully undelegated at this moment
                if prev_cluster_stake.deactivating == 0 {
                    break;
                }

                // I'm trying to get to zero, how much of the deactivation in stake
                //   this account is entitled to take
                let weight =
                    current_effective_stake as f64 / prev_cluster_stake.deactivating as f64;
                let warmup_cooldown_rate =
                    warmup_cooldown_rate(current_epoch, new_rate_activation_epoch);

                // portion of newly not-effective cluster stake I'm entitled to at current epoch
                let newly_not_effective_cluster_stake =
                    prev_cluster_stake.effective as f64 * warmup_cooldown_rate;
                let newly_not_effective_stake =
                    ((weight * newly_not_effective_cluster_stake) as u64).max(1);

                current_effective_stake =
                    current_effective_stake.saturating_sub(newly_not_effective_stake);
                if current_effective_stake == 0 {
                    break;
                }

                if current_epoch >= target_epoch {
                    break;
                }
                if let Some(current_cluster_stake) = history.get_entry(current_epoch) {
                    prev_epoch = current_epoch;
                    prev_cluster_stake = current_cluster_stake;
                } else {
                    break;
                }
            }

            current_effective_stake
        } else {
            0
        }
    }

    fn get_effective_stake_inner(
        &self,
        target_epoch: clock::Epoch,
        history: &StakeHistorySysvar,
        new_rate_activation_epoch: Option<clock::Epoch>,
    ) -> u64 {
        let delegated_stake = self.stake.into();
        let activation_epoch = self.activation_epoch.into();
        let deactivation_epoch = self.deactivation_epoch.into();

        if self.is_bootstrap() {
            // fully effective immediately
            delegated_stake
        } else if activation_epoch == deactivation_epoch {
            // activated but instantly deactivated; no stake at all regardless of target_epoch
            // this must be after the bootstrap check and before all-is-activating check
            0
        } else if target_epoch == activation_epoch {
            // all is activating
            0
        } else if target_epoch < activation_epoch {
            // not yet enabled
            0
        } else if let Some((history, mut prev_epoch, mut prev_cluster_stake)) = history
            .get_entry(activation_epoch)
            .map(|cluster_stake_at_activation_epoch| {
                (history, activation_epoch, cluster_stake_at_activation_epoch)
            })
        {
            // target_epoch > self.activation_epoch

            // loop from my activation epoch until the target epoch summing up my entitlement
            // current effective stake is updated using its previous epoch's cluster stake
            let mut current_epoch;
            let mut current_effective_stake = 0;
            loop {
                current_epoch = prev_epoch + 1;
                // if there is no activating stake at prev epoch, we should have been
                // fully effective at this moment
                if prev_cluster_stake.activating == 0 {
                    break;
                }

                // how much of the growth in stake this account is
                //  entitled to take
                let remaining_activating_stake = delegated_stake - current_effective_stake;
                let weight =
                    remaining_activating_stake as f64 / prev_cluster_stake.activating as f64;
                let warmup_cooldown_rate =
                    warmup_cooldown_rate(current_epoch, new_rate_activation_epoch);

                // portion of newly effective cluster stake I'm entitled to at current epoch
                let newly_effective_cluster_stake =
                    prev_cluster_stake.effective as f64 * warmup_cooldown_rate;
                let newly_effective_stake =
                    ((weight * newly_effective_cluster_stake) as u64).max(1);

                current_effective_stake += newly_effective_stake;
                if current_effective_stake >= delegated_stake {
                    current_effective_stake = delegated_stake;
                    break;
                }

                if current_epoch >= target_epoch || current_epoch >= deactivation_epoch {
                    break;
                }
                if let Some(current_cluster_stake) = history.get_entry(current_epoch) {
                    prev_epoch = current_epoch;
                    prev_cluster_stake = current_cluster_stake;
                } else {
                    break;
                }
            }

            current_effective_stake
        } else {
            // no history or I've dropped out of history, so assume fully effective
            delegated_stake
        }
    }

    #[inline]
    fn is_bootstrap(&self) -> bool {
        u64::from(self.activation_epoch) == u64::MAX
    }
}

fn warmup_cooldown_rate(
    current_epoch: clock::Epoch,
    new_rate_activation_epoch: Option<clock::Epoch>,
) -> f64 {
    if current_epoch < new_rate_activation_epoch.unwrap_or(u64::MAX) {
        DEFAULT_WARMUP_COOLDOWN_RATE
    } else {
        NEW_WARMUP_COOLDOWN_RATE
    }
}
