// Copyright (C) Gear Technologies Inc.
// SPDX-License-Identifier: GPL-3.0-or-later WITH Classpath-exception-2.0

//! Caller-bound BEEFY key admission and fail-closed activation, including Root dispatch.
use crate::{
    AccountId, BeefyId, BlockNumber, GearEthBridge, Runtime, RuntimeOrigin, Session, SessionKeys,
    System,
};
use frame_support::{
    Blake2_128Concat,
    crypto::ecdsa::ECDSAExt,
    pallet_prelude::ValueQuery,
    traits::{EnsureOriginWithArg, Get},
    weights::Weight,
};

use sp_core::ecdsa;
use sp_runtime::traits::{Hash, Keccak256, OpaqueKeys};
use sp_std::collections::btree_set::BTreeSet;

#[frame_support::storage_alias]
pub(crate) type RegisteredKeys = StorageMap<Session, Blake2_128Concat, AccountId, BeefyId>;

#[frame_support::storage_alias]
pub(crate) type PendingRegistrations = StorageValue<Session, u32, ValueQuery>;

fn clear_pending(validator: &AccountId) {
    if !RegisteredKeys::contains_key(validator) {
        PendingRegistrations::mutate(|count| {
            *count = count
                .checked_sub(1)
                .expect("every existing unproved owner is counted");
        });
    }
}

/// Canonical signing prehash. Bind the signed controller/stash, whole bundle, and actual chain.
pub(crate) fn registration_payload(account: &AccountId, keys: &SessionKeys) -> [u8; 32] {
    let mut preimage = [0u8; b"vara/beefy-session-keys/v1".len() + 32 + 32 + 161];
    let genesis = System::block_hash(0);
    let mut offset = 0;
    for bytes in [
        b"vara/beefy-session-keys/v1".as_slice(),
        genesis.as_ref(),
        account.as_ref(),
    ] {
        preimage[offset..offset + bytes.len()].copy_from_slice(bytes);
        offset += bytes.len();
    }
    // OpaqueKeys key order is the same fixed-field order as the SessionKeys SCALE encoding.
    for key_type in SessionKeys::key_ids() {
        let bytes = keys.get_raw(*key_type);
        preimage[offset..offset + bytes.len()].copy_from_slice(bytes);
        offset += bytes.len();
    }
    Keccak256::hash(&preimage).0
}

fn operational_key(validator: &AccountId, key: &BeefyId) -> bool {
    key != &crate::migrations::placeholder_beefy_key(validator)
        && ecdsa::Public::from(key.clone()).to_eth_address().is_ok()
}

pub struct Registration;
impl pallet_session::KeyRegistration<AccountId, SessionKeys, AccountId> for Registration {
    fn validate(
        account: &AccountId,
        validator: &AccountId,
        keys: &SessionKeys,
        proof: &[u8],
    ) -> bool {
        let Ok(bytes) = <[u8; 65]>::try_from(proof) else {
            return false;
        };

        let public = ecdsa::Public::from(keys.beefy.clone());
        !System::block_hash(0).is_zero()
            && operational_key(validator, &keys.beefy)
            && sp_io::crypto::ecdsa_verify_prehashed(
                &ecdsa::Signature::from_raw(bytes),
                &registration_payload(account, keys),
                &public,
            )
    }

    fn on_genesis(_: &AccountId) {
        PendingRegistrations::mutate(|count| {
            *count = count
                .checked_add(1)
                .expect("genesis owner count cannot overflow");
        });
    }

    fn on_registered(validator: &AccountId, keys: &SessionKeys, had_keys: bool) {
        if had_keys {
            clear_pending(validator);
        }
        RegisteredKeys::insert(validator, &keys.beefy);
    }

    fn on_purged(validator: &AccountId) {
        clear_pending(validator);
        RegisteredKeys::remove(validator);
    }

    fn weight() -> Weight {
        // Conservative admission allowance, not production benchmark evidence.
        Weight::from_parts(500_000_000, 131_072)
            .saturating_add(<Runtime as frame_system::Config>::DbWeight::get().reads_writes(4, 3))
    }
}

pub struct BindingAllowed;
impl Get<bool> for BindingAllowed {
    fn get() -> bool {
        pallet_beefy::GenesisBlock::<Runtime>::get().is_none()
    }
}

fn valid_set(keys: &[(AccountId, SessionKeys)], authorities: &[BeefyId]) -> bool {
    if keys.is_empty() || keys.len() > 256 || keys.len() != authorities.len() {
        return false;
    }
    let mut unique_keys = BTreeSet::new();
    let mut unique_validators = BTreeSet::new();
    keys.iter()
        .zip(authorities)
        .all(|((validator, keys), authority)| {
            &keys.beefy == authority
                && operational_key(validator, authority)
                && RegisteredKeys::get(validator).as_ref() == Some(authority)
                && unique_validators.insert(validator)
                && unique_keys.insert(authority)
        })
}

fn keyset_commitment(authorities: &[BeefyId]) -> sp_core::H256 {
    binary_merkle_tree::merkle_root::<<Runtime as pallet_mmr::Config>::Hashing, _>(
        authorities.iter().map(|key| {
            ecdsa::Public::from(key.clone())
                .to_eth_address()
                .expect("readiness validates every ECDSA key before computing its commitment")
        }),
    )
}

fn ready() -> bool {
    let Some((genesis, chain_id, queue)) =
        pallet_gear_eth_bridge::DestinationBinding::<Runtime>::get()
    else {
        return false;
    };
    if genesis.is_zero()
        || chain_id.is_zero()
        || queue.is_zero()
        || genesis != System::block_hash(0)
        || GearEthBridge::bridge_domain()
            != GearEthBridge::destination_domain(genesis, chain_id, queue)
    {
        return false;
    }

    let active = crate::session_history::ActiveSessionKeys::get();
    let queued = pallet_session::QueuedKeys::<Runtime>::get();
    let current = pallet_beefy::Authorities::<Runtime>::get();
    let next = pallet_beefy::NextAuthorities::<Runtime>::get();
    let set_id = pallet_beefy::ValidatorSetId::<Runtime>::get();
    let current_leaf = pallet_beefy_mmr::BeefyAuthorities::<Runtime>::get();
    let next_leaf = pallet_beefy_mmr::BeefyNextAuthorities::<Runtime>::get();
    // All legacy/genesis standby owners must rotate or purge before activation.
    // Later permissionless registrations always pass the cryptographic guard.
    if PendingRegistrations::get() != 0 || pallet_staking::ValidatorCount::<Runtime>::get() > 256 {
        return false;
    }
    valid_set(&active, &current)
        && valid_set(&queued, &next)
        && pallet_beefy::SetIdSession::<Runtime>::get(set_id) == Some(Session::current_index())
        && current_leaf.id == set_id
        && current_leaf.len as usize == current.len()
        && current_leaf.keyset_commitment == keyset_commitment(&current)
        && set_id.checked_add(1) == Some(next_leaf.id)
        && next_leaf.len as usize == next.len()
        && next_leaf.keyset_commitment == keyset_commitment(&next)
}

/// Restarts remain supported, but never change the destination or waive readiness checks.
pub struct ActivationOrigin;
impl EnsureOriginWithArg<RuntimeOrigin, BlockNumber> for ActivationOrigin {
    type Success = ();
    fn try_origin(origin: RuntimeOrigin, delay: &BlockNumber) -> Result<(), RuntimeOrigin> {
        if frame_system::ensure_root(origin.clone()).is_ok()
            && *delay > 0
            && System::block_number().checked_add(*delay).is_some()
            && ready()
        {
            Ok(())
        } else {
            Err(origin)
        }
    }

    #[cfg(feature = "runtime-benchmarks")]
    fn try_successful_origin(_: &BlockNumber) -> Result<RuntimeOrigin, ()> {
        Err(())
    }
}

pub struct ActivationWeight;
impl Get<Weight> for ActivationWeight {
    fn get() -> Weight {
        // Reserve half the operational maximum, leaving room for governance wrappers.
        // This conservative allowance is not production benchmark evidence.
        <Runtime as frame_system::Config>::BlockWeights::get()
            .get(frame_support::dispatch::DispatchClass::Operational)
            .max_extrinsic
            .expect("operational calls have a configured maximum extrinsic weight")
            .saturating_div(2)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Beefy, RuntimeCall, Utility, genesis_config_presets};
    use frame_support::{assert_noop, assert_ok, traits::UnfilteredDispatchable};
    use parity_scale_codec::Encode;
    use sp_core::{H160, H256, Pair};
    use sp_runtime::{BuildStorage, DispatchError};

    fn ext() -> sp_io::TestExternalities {
        let mut ext: sp_io::TestExternalities = genesis_config_presets::local_testnet_genesis()
            .build_storage()
            .unwrap()
            .into();
        ext.execute_with(|| {
            frame_system::BlockHash::<Runtime>::insert(0, H256::repeat_byte(9));
            System::set_block_number(10);
        });
        ext
    }

    fn register(seed: &str) {
        let (stash, ..) = genesis_config_presets::authority_keys_from_seed(seed);
        let account = pallet_staking::Bonded::<Runtime>::get(&stash).unwrap();
        let keys = pallet_session::NextKeys::<Runtime>::get(&stash).unwrap();
        assert_eq!(
            registration_payload(&account, &keys),
            Keccak256::hash(
                &(
                    b"vara/beefy-session-keys/v1",
                    System::block_hash(0),
                    &account,
                    &keys
                )
                    .encode(),
            )
            .0
        );
        let pair = ecdsa::Pair::from_string(&format!("//{seed}"), None).unwrap();
        let proof = pair
            .sign_prehashed(&registration_payload(&account, &keys))
            .0
            .to_vec();
        assert_ok!(Session::set_keys(
            RuntimeOrigin::signed(account),
            keys,
            proof
        ));
    }

    fn bind() {
        assert_ok!(GearEthBridge::bind_destination(
            RuntimeOrigin::root(),
            H256::from_low_u64_be(1),
            H160::repeat_byte(3)
        ));
    }

    fn prepare() {
        bind();
        register("Alice");
        register("Bob");
    }

    #[test]
    fn offline_helper_proof_matches_the_runtime_canonical_payload() {
        ext().execute_with(|| {
            use parity_scale_codec::Decode;
            let account = AccountId::new([6; 32]);
            let mut encoded = [0u8; 161];
            encoded[128..].copy_from_slice(&[
                2, 85, 44, 99, 11, 100, 181, 75, 245, 2, 16, 201, 226, 83, 211, 139, 212, 148, 156,
                114, 226, 40, 115, 80, 15, 98, 133, 194, 190, 222, 49, 42, 132,
            ]);
            let keys = SessionKeys::decode(&mut encoded.as_slice()).unwrap();
            let proof = [
                120, 71, 213, 95, 129, 59, 103, 153, 112, 125, 239, 198, 42, 244, 13, 178, 77, 99,
                132, 57, 47, 27, 207, 48, 187, 88, 193, 214, 129, 28, 93, 246, 18, 175, 206, 230,
                165, 55, 131, 36, 63, 52, 89, 248, 199, 169, 216, 198, 231, 151, 252, 191, 210, 83,
                146, 115, 226, 176, 187, 4, 111, 192, 159, 5, 0,
            ];
            assert_eq!(
                registration_payload(&account, &keys),
                [
                    11, 33, 165, 37, 248, 157, 188, 240, 178, 220, 91, 40, 39, 79, 88, 165, 172,
                    212, 71, 228, 157, 248, 200, 32, 145, 231, 101, 206, 1, 206, 34, 62
                ]
            );
            assert!(<Registration as pallet_session::KeyRegistration<
                AccountId,
                SessionKeys,
                AccountId,
            >>::validate(&account, &account, &keys, &proof));
        });
    }

    #[test]
    fn proven_permissionless_standbys_do_not_block_activation_or_new_admission() {
        ext().execute_with(|| {
            prepare();
            assert_eq!(PendingRegistrations::get(), 0);
            for index in 0..257 {
                if index == 255 {
                    assert_eq!(
                        pallet_session::NextKeys::<Runtime>::iter_keys().count(),
                        257
                    );
                    assert!(ready());
                    assert_ok!(Beefy::set_new_genesis(RuntimeOrigin::root(), 1));
                }
                let seed = format!("Standby{index}");
                let (stash, _, babe, grandpa, im_online, authority_discovery, beefy) =
                    genesis_config_presets::authority_keys_from_seed(&seed);
                let keys = SessionKeys {
                    babe,
                    grandpa,
                    im_online,
                    authority_discovery,
                    beefy,
                };
                assert_ok!(crate::Balances::force_set_balance(
                    RuntimeOrigin::root(),
                    stash.clone().into(),
                    1_000_000_000_000_000_000,
                ));
                assert_ok!(crate::Staking::bond(
                    RuntimeOrigin::signed(stash.clone()),
                    100_000_000_000_000,
                    pallet_staking::RewardDestination::Staked,
                ));
                let pair = ecdsa::Pair::from_string(&format!("//{seed}"), None).unwrap();
                let proof = pair
                    .sign_prehashed(&registration_payload(&stash, &keys))
                    .0
                    .to_vec();
                assert_ok!(Session::set_keys(
                    RuntimeOrigin::signed(stash.clone()),
                    keys.clone(),
                    proof
                ));
                assert_eq!(RegisteredKeys::get(&stash), Some(keys.beefy));
                assert_eq!(PendingRegistrations::get(), 0);
            }
            assert_eq!(
                pallet_session::NextKeys::<Runtime>::iter_keys().count(),
                259
            );
            assert!(ready());
            pallet_staking::ValidatorCount::<Runtime>::put(257);
            assert!(!ready());
        });
    }

    #[test]
    fn fresh_genesis_standbys_and_repeated_registration_keep_pending_count_exact() {
        let mut genesis = genesis_config_presets::local_testnet_genesis();
        let (standby, _, babe, grandpa, im_online, authority_discovery, beefy) =
            genesis_config_presets::authority_keys_from_seed("Charlie");
        genesis.session.non_authority_keys.push((
            standby.clone(),
            standby.clone(),
            SessionKeys {
                babe,
                grandpa,
                im_online,
                authority_discovery,
                beefy,
            },
        ));
        let mut ext: sp_io::TestExternalities = genesis.build_storage().unwrap().into();
        ext.execute_with(|| {
            frame_system::BlockHash::<Runtime>::insert(0, H256::repeat_byte(9));
            System::set_block_number(10);
            assert_eq!(PendingRegistrations::get(), 3);
            prepare();
            assert_eq!(PendingRegistrations::get(), 1);
            assert!(!ready());
            register("Alice");
            assert_eq!(PendingRegistrations::get(), 1);
            assert_ok!(Session::purge_keys(RuntimeOrigin::signed(standby)));
            assert_eq!(PendingRegistrations::get(), 0);
            assert!(ready());
            let (bob, ..) = genesis_config_presets::authority_keys_from_seed("Bob");
            let signer = pallet_staking::Bonded::<Runtime>::get(&bob).unwrap();
            assert_ok!(Session::purge_keys(RuntimeOrigin::signed(signer)));
            assert_eq!(PendingRegistrations::get(), 0);
        });
    }

    #[test]
    fn activation_is_fail_closed_even_for_root_bypass_and_then_supports_restart() {
        ext().execute_with(|| {
            assert!(pallet_beefy::GenesisBlock::<Runtime>::get().is_none());
            let call =
                RuntimeCall::Beefy(pallet_beefy::Call::set_new_genesis { delay_in_blocks: 1 });
            assert!(call.dispatch_bypass_filter(RuntimeOrigin::root()).is_err());
            bind();
            assert_noop!(
                Beefy::set_new_genesis(RuntimeOrigin::root(), 1),
                DispatchError::BadOrigin
            );
            register("Alice");
            assert_noop!(
                Beefy::set_new_genesis(RuntimeOrigin::root(), 1),
                DispatchError::BadOrigin
            );
            register("Bob");
            assert_noop!(
                Beefy::set_new_genesis(RuntimeOrigin::root(), 0),
                DispatchError::BadOrigin
            );
            assert_noop!(
                Beefy::set_new_genesis(RuntimeOrigin::root(), u32::MAX),
                DispatchError::BadOrigin
            );
            assert_noop!(
                Beefy::set_new_genesis(RuntimeOrigin::signed(AccountId::new([5; 32])), 1),
                DispatchError::BadOrigin
            );
            assert_ok!(Utility::batch_all(
                RuntimeOrigin::root(),
                sp_std::vec![RuntimeCall::Beefy(pallet_beefy::Call::set_new_genesis {
                    delay_in_blocks: 2
                })]
            ));
            assert_eq!(pallet_beefy::GenesisBlock::<Runtime>::get(), Some(12));
            let domain = GearEthBridge::bridge_domain();
            System::set_block_number(12);
            assert_ok!(Beefy::set_new_genesis(RuntimeOrigin::root(), 3));
            assert_eq!(pallet_beefy::GenesisBlock::<Runtime>::get(), Some(15));
            assert_eq!(GearEthBridge::bridge_domain(), domain);
            assert_noop!(
                GearEthBridge::bind_destination(
                    RuntimeOrigin::root(),
                    H256::from_low_u64_be(2),
                    H160::repeat_byte(4)
                ),
                pallet_gear_eth_bridge::Error::<Runtime>::InvalidDestinationBinding
            );
        });
    }

    #[test]
    fn destination_uses_actual_genesis_is_nonzero_and_cannot_be_rebound() {
        ext().execute_with(|| {
            let chain = H256::from_low_u64_be(1);
            let queue = H160::repeat_byte(3);
            assert_noop!(
                GearEthBridge::bind_destination(
                    RuntimeOrigin::signed(AccountId::new([5; 32])),
                    chain,
                    queue
                ),
                DispatchError::BadOrigin
            );
            assert_noop!(
                GearEthBridge::bind_destination(RuntimeOrigin::root(), H256::zero(), queue),
                pallet_gear_eth_bridge::Error::<Runtime>::InvalidDestinationBinding
            );
            assert_noop!(
                GearEthBridge::bind_destination(RuntimeOrigin::root(), chain, H160::zero()),
                pallet_gear_eth_bridge::Error::<Runtime>::InvalidDestinationBinding
            );
            frame_system::BlockHash::<Runtime>::insert(0, H256::zero());
            assert_noop!(
                GearEthBridge::bind_destination(RuntimeOrigin::root(), chain, queue),
                pallet_gear_eth_bridge::Error::<Runtime>::InvalidDestinationBinding
            );
            frame_system::BlockHash::<Runtime>::insert(0, H256::repeat_byte(9));
            bind();
            let expected = Keccak256::hash(
                &[
                    b"vara/gear-eth-bridge-domain/v2".as_slice(),
                    H256::repeat_byte(9).as_bytes(),
                    chain.as_bytes(),
                    queue.as_bytes(),
                ]
                .concat(),
            );
            assert_eq!(GearEthBridge::bridge_domain(), expected);
            assert_noop!(
                GearEthBridge::bind_destination(RuntimeOrigin::root(), chain, queue),
                pallet_gear_eth_bridge::Error::<Runtime>::InvalidDestinationBinding
            );
            pallet_gear_eth_bridge::BridgeDomain::<Runtime>::put(H256::repeat_byte(1));
            assert_noop!(
                Beefy::set_new_genesis(RuntimeOrigin::root(), 1),
                DispatchError::BadOrigin
            );
        });
    }

    #[test]
    fn registration_proof_binds_account_chain_and_entire_bundle() {
        ext().execute_with(|| {
            let (stash, ..) = genesis_config_presets::authority_keys_from_seed("Alice");
            let account = pallet_staking::Bonded::<Runtime>::get(&stash).unwrap();
            let keys = pallet_session::NextKeys::<Runtime>::get(&stash).unwrap();
            let pair = ecdsa::Pair::from_string("//Alice", None).unwrap();
            let proof = pair
                .sign_prehashed(&registration_payload(&account, &keys))
                .0
                .to_vec();
            assert_noop!(
                Session::set_keys(
                    RuntimeOrigin::signed(account.clone()),
                    keys.clone(),
                    sp_std::vec![]
                ),
                pallet_session::Error::<Runtime>::InvalidProof
            );
            let (bob, ..) = genesis_config_presets::authority_keys_from_seed("Bob");
            let bob = pallet_staking::Bonded::<Runtime>::get(&bob).unwrap();
            assert_noop!(
                Session::set_keys(RuntimeOrigin::signed(bob), keys.clone(), proof.clone()),
                pallet_session::Error::<Runtime>::InvalidProof
            );
            let mut changed = keys.clone();
            changed.grandpa = genesis_config_presets::authority_keys_from_seed("Charlie").3;
            assert_noop!(
                Session::set_keys(
                    RuntimeOrigin::signed(account.clone()),
                    changed,
                    proof.clone()
                ),
                pallet_session::Error::<Runtime>::InvalidProof
            );
            frame_system::BlockHash::<Runtime>::insert(0, H256::repeat_byte(8));
            assert_noop!(
                Session::set_keys(
                    RuntimeOrigin::signed(account.clone()),
                    keys.clone(),
                    proof.clone()
                ),
                pallet_session::Error::<Runtime>::InvalidProof
            );
            frame_system::BlockHash::<Runtime>::insert(0, H256::repeat_byte(9));
            assert_ok!(Session::set_keys(
                RuntimeOrigin::signed(account),
                keys.clone(),
                proof
            ));
            assert_eq!(RegisteredKeys::get(&stash), Some(keys.beefy));
        });
    }

    #[test]
    fn activation_rejects_empty_invalid_placeholder_duplicate_and_uninitialized_sets() {
        ext().execute_with(|| {
            prepare();
            let active = crate::session_history::ActiveSessionKeys::get();
            let current = pallet_beefy::Authorities::<Runtime>::get();
            let queued = pallet_session::QueuedKeys::<Runtime>::get();
            assert!(ready());
            crate::session_history::ActiveSessionKeys::put(Vec::<(AccountId, SessionKeys)>::new());
            assert!(!ready());
            crate::session_history::ActiveSessionKeys::put(&active);
            pallet_beefy::Authorities::<Runtime>::kill();
            assert!(!ready());
            pallet_beefy::Authorities::<Runtime>::put(&current);
            let mut bad = active.clone();
            bad[0].1.beefy = crate::migrations::placeholder_beefy_key(&bad[0].0);
            assert!(!operational_key(&bad[0].0, &bad[0].1.beefy));
            bad[0].1.beefy = ecdsa::Public::from_raw([0; 33]).into();
            assert!(!operational_key(&bad[0].0, &bad[0].1.beefy));
            bad = active.clone();
            bad[1].1.beefy = bad[0].1.beefy.clone();
            let keys: Vec<_> = bad.iter().map(|(_, keys)| keys.beefy.clone()).collect();
            assert!(!valid_set(&bad, &keys));
            bad = active.clone();
            bad[1] = bad[0].clone();
            let keys: Vec<_> = bad.iter().map(|(_, keys)| keys.beefy.clone()).collect();
            assert!(!valid_set(&bad, &keys));
            pallet_session::QueuedKeys::<Runtime>::put(Vec::<(AccountId, SessionKeys)>::new());
            assert!(!ready());
            pallet_session::QueuedKeys::<Runtime>::put(&queued);
            let descriptor = pallet_beefy_mmr::BeefyNextAuthorities::<Runtime>::get();
            let mut incorrect = descriptor.clone();
            incorrect.keyset_commitment = H256::repeat_byte(1);
            pallet_beefy_mmr::BeefyNextAuthorities::<Runtime>::put(incorrect);
            assert!(!ready());
            pallet_beefy_mmr::BeefyNextAuthorities::<Runtime>::put(descriptor);
            let unknown = AccountId::new([7; 32]);
            let mut stale = active[0].1.clone();
            stale.beefy = crate::migrations::placeholder_beefy_key(&unknown);
            pallet_session::NextKeys::<Runtime>::insert(&unknown, stale);
            PendingRegistrations::put(1);
            assert!(!ready());
            pallet_session::NextKeys::<Runtime>::remove(&unknown);
            PendingRegistrations::put(0);
            let (stash, ..) = genesis_config_presets::authority_keys_from_seed("Bob");
            let account = pallet_staking::Bonded::<Runtime>::get(&stash).unwrap();
            assert_ok!(Session::purge_keys(RuntimeOrigin::signed(account)));
            assert!(!RegisteredKeys::contains_key(&stash));
            pallet_beefy_mmr::BeefyNextAuthorities::<Runtime>::kill();
            assert_noop!(
                Beefy::set_new_genesis(RuntimeOrigin::root(), 1),
                DispatchError::BadOrigin
            );
        });
    }
}
