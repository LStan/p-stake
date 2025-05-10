use pinocchio::pubkey::Pubkey;

use super::{Epoch, PodF64, PodU64};

#[repr(C)]
#[derive(Debug, PartialEq)]
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
