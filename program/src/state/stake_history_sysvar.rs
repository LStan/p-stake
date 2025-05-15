use pinocchio::sysvars::clock::Epoch;
use pinocchio_pubkey::from_str;

use crate::pinocchio_add::sysvar::get_sysvar_unchecked;

use super::StakeHistoryEntry;

#[derive(Debug, PartialEq)]
pub struct StakeHistorySysvar(pub Epoch);

pub const MAX_ENTRIES: usize = 512; // it should never take as many as 512 epochs to warm up or cool down

// precompute so we can statically allocate buffer
const EPOCH_AND_ENTRY_SERIALIZED_SIZE: u64 = 32;

impl StakeHistorySysvar {
    pub fn get_entry(&self, target_epoch: Epoch) -> Option<StakeHistoryEntry> {
        let current_epoch = self.0;

        // if current epoch is zero this returns None because there is no history yet
        let newest_historical_epoch = current_epoch.checked_sub(1)?;
        let oldest_historical_epoch = current_epoch.saturating_sub(MAX_ENTRIES as u64);

        // target epoch is old enough to have fallen off history; presume fully active/deactive
        if target_epoch < oldest_historical_epoch {
            return None;
        }

        // epoch delta is how many epoch-entries we offset in the stake history vector, which may be zero
        // None means target epoch is current or in the future; this is a user error
        let epoch_delta = newest_historical_epoch.checked_sub(target_epoch)?;

        // offset is the number of bytes to our desired entry, including eight for vector length
        let offset = epoch_delta
            .checked_mul(EPOCH_AND_ENTRY_SERIALIZED_SIZE)?
            .checked_add(core::mem::size_of::<u64>() as u64)?;

        // TODO: change to MaybeUninit
        let mut entry_buf = [0; EPOCH_AND_ENTRY_SERIALIZED_SIZE as usize];
        // SAFETY: the buffer is large enough
        let result = unsafe {
            get_sysvar_unchecked(
                &mut entry_buf,
                &from_str("SysvarStakeHistory1111111111111111111111111"),
                offset,
                EPOCH_AND_ENTRY_SERIALIZED_SIZE,
            )
        };

        match result {
            Ok(()) => {
                // TODO: optimize
                let entry_epoch = u64::from_le_bytes(entry_buf[0..8].try_into().unwrap());
                let effective = u64::from_le_bytes(entry_buf[8..16].try_into().unwrap());
                let activating = u64::from_le_bytes(entry_buf[16..24].try_into().unwrap());
                let deactivating = u64::from_le_bytes(entry_buf[24..32].try_into().unwrap());

                // this would only fail if stake history skipped an epoch or the binary format of the sysvar changed
                assert_eq!(entry_epoch, target_epoch);

                Some(StakeHistoryEntry {
                    effective,
                    activating,
                    deactivating,
                })
            }
            _ => None,
        }
    }
}
