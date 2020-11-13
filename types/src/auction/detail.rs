use alloc::vec::Vec;
use core::convert::TryInto;

use num_rational::Ratio;

use crate::{
    auction::{
        constants::*, Auction, Bids, DelegatorRewardMap, EraId, MintProvider, RuntimeProvider,
        SeigniorageRecipientsSnapshot, StorageProvider, SystemProvider, UnbondingPurse,
        UnbondingPurses, ValidatorRewardMap,
    },
    bytesrepr::{FromBytes, ToBytes},
    system_contract_errors::auction::{Error, Result},
    CLTyped, PublicKey, URef, U512,
};

fn read_from<P, T>(provider: &mut P, name: &str) -> Result<T>
where
    P: StorageProvider + RuntimeProvider + ?Sized,
    T: FromBytes + CLTyped,
{
    let key = provider.get_key(name).ok_or(Error::MissingKey)?;
    let uref = key.into_uref().ok_or(Error::InvalidKeyVariant)?;
    let value: T = provider.read(uref)?.ok_or(Error::MissingValue)?;
    Ok(value)
}

fn write_to<P, T>(provider: &mut P, name: &str, value: T) -> Result<()>
where
    P: StorageProvider + RuntimeProvider + ?Sized,
    T: ToBytes + CLTyped,
{
    let key = provider.get_key(name).ok_or(Error::MissingKey)?;
    let uref = key.into_uref().ok_or(Error::InvalidKeyVariant)?;
    provider.write(uref, value)?;
    Ok(())
}

pub fn get_bids<P>(provider: &mut P) -> Result<Bids>
where
    P: StorageProvider + RuntimeProvider + ?Sized,
{
    Ok(read_from(provider, BIDS_KEY)?)
}

pub fn set_bids<P>(provider: &mut P, validators: Bids) -> Result<()>
where
    P: StorageProvider + RuntimeProvider + ?Sized,
{
    write_to(provider, BIDS_KEY, validators)
}

pub fn get_unbonding_purses<P>(provider: &mut P) -> Result<UnbondingPurses>
where
    P: StorageProvider + RuntimeProvider + ?Sized,
{
    Ok(read_from(provider, UNBONDING_PURSES_KEY)?)
}

pub fn set_unbonding_purses<P>(provider: &mut P, unbonding_purses: UnbondingPurses) -> Result<()>
where
    P: StorageProvider + RuntimeProvider + ?Sized,
{
    write_to(provider, UNBONDING_PURSES_KEY, unbonding_purses)
}

pub fn get_delegator_reward_map<P>(provider: &mut P) -> Result<DelegatorRewardMap>
where
    P: StorageProvider + RuntimeProvider + ?Sized,
{
    read_from(provider, DELEGATOR_REWARD_MAP_KEY)
}

pub fn set_delegator_reward_map<P>(
    provider: &mut P,
    delegator_reward_map: DelegatorRewardMap,
) -> Result<()>
where
    P: StorageProvider + RuntimeProvider + ?Sized,
{
    write_to(provider, DELEGATOR_REWARD_MAP_KEY, delegator_reward_map)
}

pub fn get_validator_reward_map<P>(provider: &mut P) -> Result<ValidatorRewardMap>
where
    P: StorageProvider + RuntimeProvider + ?Sized,
{
    read_from(provider, VALIDATOR_REWARD_MAP_KEY)
}

pub fn set_validator_reward_map<P>(
    provider: &mut P,
    validator_reward_map: ValidatorRewardMap,
) -> Result<()>
where
    P: StorageProvider + RuntimeProvider + ?Sized,
{
    write_to(provider, VALIDATOR_REWARD_MAP_KEY, validator_reward_map)
}

pub fn get_era_id<P>(provider: &mut P) -> Result<EraId>
where
    P: StorageProvider + RuntimeProvider + ?Sized,
{
    Ok(read_from(provider, ERA_ID_KEY)?)
}

pub fn set_era_id<P>(provider: &mut P, era_id: u64) -> Result<()>
where
    P: StorageProvider + RuntimeProvider + ?Sized,
{
    write_to(provider, ERA_ID_KEY, era_id)
}

pub fn get_seigniorage_recipients_snapshot<P>(
    provider: &mut P,
) -> Result<SeigniorageRecipientsSnapshot>
where
    P: StorageProvider + RuntimeProvider + ?Sized,
{
    Ok(read_from(provider, SEIGNIORAGE_RECIPIENTS_SNAPSHOT_KEY)?)
}

pub fn set_seigniorage_recipients_snapshot<P>(
    provider: &mut P,
    snapshot: SeigniorageRecipientsSnapshot,
) -> Result<()>
where
    P: StorageProvider + RuntimeProvider + ?Sized,
{
    write_to(provider, SEIGNIORAGE_RECIPIENTS_SNAPSHOT_KEY, snapshot)
}

pub fn get_validator_slots<P>(provider: &mut P) -> Result<usize>
where
    P: StorageProvider + RuntimeProvider + ?Sized,
{
    let validator_slots: u32 = read_from(provider, VALIDATOR_SLOTS_KEY)?;
    let validator_slots = validator_slots
        .try_into()
        .map_err(|_| Error::InvalidValidatorSlotsValue)?;
    Ok(validator_slots)
}

pub fn get_auction_delay<P>(provider: &mut P) -> Result<u64>
where
    P: StorageProvider + RuntimeProvider + ?Sized,
{
    let auction_delay: u64 = read_from(provider, AUCTION_DELAY_KEY)?;
    Ok(auction_delay)
}

/// Iterates over unbonding entries and checks if a locked amount can be paid already if
/// a specific era is reached.
///
/// This function can be called by the system only.
pub(crate) fn process_unbond_requests<P: Auction + ?Sized>(provider: &mut P) -> Result<()> {
    if provider.get_caller() != SYSTEM_ACCOUNT {
        return Err(Error::InvalidCaller);
    }

    // Update `unbonding_purses` data
    let mut unbonding_purses: UnbondingPurses = get_unbonding_purses(provider)?;

    let current_era_id = provider.read_era_id()?;

    for unbonding_list in unbonding_purses.values_mut() {
        let mut new_unbonding_list = Vec::new();
        for unbonding_purse in unbonding_list.iter() {
            // Since `process_unbond_requests` is run before `run_auction`, we should check
            // if current era id is equal or greater than the `era_of_withdrawal` that was
            // calculated on `unbond` attempt.
            if current_era_id >= unbonding_purse.era_of_withdrawal as u64 {
                // Move funds from bid purse to unbonding purse
                provider.transfer_from_purse_to_purse(
                    unbonding_purse.bonding_purse,
                    unbonding_purse.unbonding_purse,
                    unbonding_purse.amount,
                )?;
            } else {
                new_unbonding_list.push(*unbonding_purse);
            }
        }
        *unbonding_list = new_unbonding_list;
    }

    // Prune empty entries
    let unbonding_purses = unbonding_purses
        .into_iter()
        .filter(|(_k, unbonding_purses)| !unbonding_purses.is_empty())
        .collect();

    set_unbonding_purses(provider, unbonding_purses)?;
    Ok(())
}

/// Creates a new purse in unbonding_purses given a validator's key, amount, and a destination
/// unbonding purse. Returns the amount of motes remaining in the validator's bid purse.
pub(crate) fn create_unbonding_purse<P: Auction + ?Sized>(
    provider: &mut P,
    public_key: PublicKey,
    bonding_purse: URef,
    unbonding_purse: URef,
    amount: U512,
) -> Result<U512> {
    if provider.get_balance(bonding_purse)?.unwrap_or_default() < amount {
        return Err(Error::UnbondTooLarge);
    }

    let mut unbonding_purses: UnbondingPurses = get_unbonding_purses(provider)?;
    let era_of_withdrawal = provider.read_era_id()? + DEFAULT_UNBONDING_DELAY;
    let new_unbonding_purse = UnbondingPurse {
        bonding_purse,
        unbonding_purse,
        public_key,
        era_of_withdrawal,
        amount,
    };
    unbonding_purses
        .entry(public_key)
        .or_default()
        .push(new_unbonding_purse);
    set_unbonding_purses(provider, unbonding_purses)?;

    // Remaining motes in the validator's bid purse
    let remaining_bond = provider.get_balance(bonding_purse)?.unwrap_or_default();

    Ok(remaining_bond)
}

/// Update validator reward map.
pub fn update_delegator_rewards<P>(
    provider: &mut P,
    validator_public_key: PublicKey,
    rewards: impl Iterator<Item = (PublicKey, Ratio<U512>)>,
) -> Result<U512>
where
    P: MintProvider + RuntimeProvider + StorageProvider + SystemProvider + ?Sized,
{
    let mut total_delegator_payout = U512::zero();
    let mut outer = get_delegator_reward_map(provider)?;
    let mut inner = outer.remove(&validator_public_key).unwrap_or_default();

    for (delegator_key, delegator_reward) in rewards {
        let delegator_reward_trunc = delegator_reward.to_integer();
        inner
            .entry(delegator_key)
            .and_modify(|sum| *sum += delegator_reward_trunc)
            .or_insert_with(|| delegator_reward_trunc);
        total_delegator_payout += delegator_reward_trunc;
    }

    outer.insert(validator_public_key, inner);
    set_delegator_reward_map(provider, outer)?;
    Ok(total_delegator_payout)
}

/// Update validator reward map.
pub fn update_validator_reward<P>(
    provider: &mut P,
    validator_public_key: PublicKey,
    amount: U512,
) -> Result<()>
where
    P: MintProvider + RuntimeProvider + StorageProvider + SystemProvider + ?Sized,
{
    let mut validator_reward_map = get_validator_reward_map(provider)?;
    validator_reward_map
        .entry(validator_public_key)
        .and_modify(|sum| *sum += amount)
        .or_insert_with(|| amount);
    set_validator_reward_map(provider, validator_reward_map)?;
    Ok(())
}

/// Removes validator entries from either founders or validators, wherever they
/// might be found.
///
/// This function is intended to be called together with the slash function in the Mint
/// contract.
pub(crate) fn quash_bid<P: StorageProvider + RuntimeProvider + ?Sized>(
    provider: &mut P,
    validator_public_keys: &[PublicKey],
) -> Result<()> {
    // Clean up inside `bids`
    let mut validators = get_bids(provider)?;

    let mut modified_validators = 0usize;

    for validator_public_key in validator_public_keys {
        if validators.remove(validator_public_key).is_some() {
            modified_validators += 1;
        }
    }

    if modified_validators > 0 {
        set_bids(provider, validators)?;
    }

    Ok(())
}
