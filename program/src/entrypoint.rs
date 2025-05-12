use pinocchio::{
    account_info::AccountInfo, cpi::set_return_data, no_allocator, nostd_panic_handler,
    program_entrypoint, program_error::ProgramError, pubkey::Pubkey, sysvars::Sysvar,
    ProgramResult,
};

use crate::{instruction, pinocchio_add::epoch_rewards::EpochRewards};

// This is the entrypoint for the program.
program_entrypoint!(process_instruction);
//Do not allocate memory.
no_allocator!();
// Use the no_std panic handler.
nostd_panic_handler!();

#[inline(always)]
fn process_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    // convenience so we can safely use id() everywhere
    if *program_id != crate::ID {
        return Err(ProgramError::IncorrectProgramId);
    }

    let (ix_disc, instruction_data) = instruction_data
        .split_first_chunk::<4>()
        .ok_or(ProgramError::InvalidInstructionData)?;

    // Second variant, test CUs usage
    // let (ix_disc, instruction_data) = instruction_data
    //     .split_at_checked(4)
    //     .ok_or(ProgramError::InvalidInstructionData)?;

    let instruction = &ix_disc[0];

    let epoch_rewards_active = EpochRewards::get()
        .map(|epoch_rewards| epoch_rewards.active)
        .unwrap_or(false);
    // 13 == GetMinimumDelegation
    if epoch_rewards_active && *instruction != 13 {
        return Err(ProgramError::Custom(16));
    }

    match *instruction {
        // 0 - Initialize
        0 => {
            #[cfg(feature = "logging")]
            pinocchio::msg!("Instruction: Initialize");

            instruction::process_initialize(accounts, instruction_data)
        }
        // 1 - Authorize
        1 => {
            #[cfg(feature = "logging")]
            pinocchio::msg!("Instruction: Authorize");

            instruction::process_authorize(accounts, instruction_data)
        }
        // 2 - DelegateStake
        2 => {
            #[cfg(feature = "logging")]
            pinocchio::msg!("Instruction: DelegateStake");

            todo!()
        }
        // 3 - Split
        3 => {
            #[cfg(feature = "logging")]
            pinocchio::msg!("Instruction: Split");

            todo!()
        }
        // 4 - Withdraw
        4 => {
            #[cfg(feature = "logging")]
            pinocchio::msg!("Instruction: Withdraw");

            todo!()
        }
        // 5 - Deactivate
        5 => {
            #[cfg(feature = "logging")]
            pinocchio::msg!("Instruction: Deactivate");

            instruction::process_deactivate(accounts, instruction_data)
        }
        // 6 - SetLockup
        6 => {
            #[cfg(feature = "logging")]
            pinocchio::msg!("Instruction: SetLockup");

            instruction::process_set_lockup(accounts, instruction_data)
        }
        // 7 - Merge
        7 => {
            #[cfg(feature = "logging")]
            pinocchio::msg!("Instruction: Merge");

            todo!()
        }
        // 8 - AuthorizeWithSeed
        8 => {
            #[cfg(feature = "logging")]
            pinocchio::msg!("Instruction: AuthorizeWithSeed");

            instruction::process_authorize_with_seed(accounts, instruction_data)
        }
        // 9 - InitializeChecked
        9 => {
            #[cfg(feature = "logging")]
            pinocchio::msg!("Instruction: InitializeChecked");

            instruction::process_initialize_checked(accounts, instruction_data)
        }
        // 10 - AuthorizeChecked
        10 => {
            #[cfg(feature = "logging")]
            pinocchio::msg!("Instruction: AuthorizeChecked");

            instruction::process_authorize_checked(accounts, instruction_data)
        }
        // 11 - AuthorizeCheckedWithSeed
        11 => {
            #[cfg(feature = "logging")]
            pinocchio::msg!("Instruction: AuthorizeCheckedWithSeed");

            instruction::process_authorize_checked_with_seed(accounts, instruction_data)
        }
        // 12 - SetLockupChecked
        12 => {
            #[cfg(feature = "logging")]
            pinocchio::msg!("Instruction: SetLockupChecked");

            instruction::process_set_lockup_checked(accounts, instruction_data)
        }
        // 13 - GetMinimumDelegation
        13 => {
            #[cfg(feature = "logging")]
            pinocchio::msg!("Instruction: GetMinimumDelegation");

            let minimum_delegation = crate::get_minimum_delegation();
            set_return_data(&minimum_delegation.to_le_bytes());

            Ok(())
        }
        // 14 - DeactivateDelinquent
        14 => {
            #[cfg(feature = "logging")]
            pinocchio::msg!("Instruction: DeactivateDelinquent");

            todo!()
        }
        // 15 - Redelegate deprecated
        15 => Err(ProgramError::InvalidInstructionData),
        // 16 - MoveStake
        16 => {
            #[cfg(feature = "logging")]
            pinocchio::msg!("Instruction: MoveStake");

            todo!()
        }
        // 17 - MoveLamports
        17 => {
            #[cfg(feature = "logging")]
            pinocchio::msg!("Instruction: MoveLamports");

            todo!()
        }
        _ => Err(ProgramError::InvalidInstructionData),
    }
}
