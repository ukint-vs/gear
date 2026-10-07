// Copyright (C) Gear Technologies Inc.
// SPDX-License-Identifier: GPL-3.0-or-later WITH Classpath-exception-2.0

//! Runtime-specific benchmarks, not an on-chain pallet or a replacement weight schedule.
//!
//! FRAME times only the `}: { ... }` block: deterministic key generation/signing,
//! fixture writes and expected-root computation precede it; assertions follow it.
//! Storage-root recalculation is reported separately by the CLI. Keep verification
//! enabled. Release calibration uses Wasm, --steps 50 --repeat 20
//! on reference hardware, not wall-clock assertions in unit tests.
//! Owner/exposure ranges below are samples, NOT protocol bounds. Migration walks
//! EVERY legacy NextKeys/KeyOwner record, including permissionless standby owners.
//! Session on_initialize already reserves max_block; do not add these measurements
//! to that reservation. Upstream base weights and conservative allowances stay intact.

extern crate alloc;

use crate::{
    AccountId, AuthorityDiscovery, Babe, Beefy, GearEthBridge, Grandpa, ImOnline, Runtime,
    RuntimeOrigin, Session, SessionKeys, Staking, System,
    beefy_activation::{PendingRegistrations, RegisteredKeys, registration_payload},
    migrations::{MigrateSessionKeys, SessionKeysOld, placeholder_beefy_key},
    pallet_session_historical,
    session_history::{self, ActiveSessionKeys, HistoricalRoot, LegacySessionRoots},
};
use frame_benchmarking::benchmarks;
use frame_support::{
    assert_ok,
    traits::{KeyOwnerProofSystem, OnRuntimeUpgrade, OneSessionHandler},
};
use parity_scale_codec::Encode;
use sp_core::{H160, H256, ecdsa};
use sp_runtime::{StateVersion, traits::OpaqueKeys};
use sp_std::{marker::PhantomData, prelude::*};

// Like frame_system_benchmarking::Pallet: a benchmark adapter only. No call index,
// runtime metadata, genesis configuration, or production storage is introduced.
pub struct Pallet<T>(PhantomData<T>);
pub trait Config: frame_system::Config {}
impl Config for Runtime {}

fn account(index: u32) -> AccountId {
    frame_benchmarking::account("beefy-validator", index, 0)
}

fn old_keys(index: u32) -> SessionKeysOld {
    let bytes = sp_io::hashing::blake2_256(&(b"beefy-benchmark", index).encode());
    SessionKeysOld {
        babe: sp_core::sr25519::Public::from_raw(bytes).into(),
        grandpa: sp_core::ed25519::Public::from_raw(bytes).into(),
        im_online: sp_core::sr25519::Public::from_raw(bytes).into(),
        authority_discovery: sp_core::sr25519::Public::from_raw(bytes).into(),
    }
}

fn keys(index: u32) -> (SessionKeys, ecdsa::Public) {
    let public = sp_io::crypto::ecdsa_generate(
        sp_consensus_beefy::KEY_TYPE,
        Some(alloc::format!("//BeefyBenchmark//{index}").into_bytes()),
    );
    (with_beefy(old_keys(index), public.into()), public)
}

fn with_beefy(old: SessionKeysOld, beefy: crate::BeefyId) -> SessionKeys {
    SessionKeys {
        babe: old.babe,
        grandpa: old.grandpa,
        im_online: old.im_online,
        authority_discovery: old.authority_discovery,
        beefy,
    }
}

fn reset_registry() {
    for owner in pallet_session::NextKeys::<Runtime>::iter_keys().collect::<Vec<_>>() {
        pallet_session::NextKeys::<Runtime>::remove(owner);
    }
    for key in pallet_session::KeyOwner::<Runtime>::iter_keys().collect::<Vec<_>>() {
        pallet_session::KeyOwner::<Runtime>::remove(key);
    }
    for owner in RegisteredKeys::iter_keys().collect::<Vec<_>>() {
        RegisteredKeys::remove(owner);
    }
    PendingRegistrations::put(0);
    System::set_block_number(10);
    frame_system::BlockHash::<Runtime>::insert(0, H256::repeat_byte(9));
}

fn seed_registration(owner: &AccountId, keys: &SessionKeys, proved: bool) {
    pallet_session::NextKeys::<Runtime>::insert(owner, keys);
    for kind in SessionKeys::key_ids() {
        pallet_session::KeyOwner::<Runtime>::insert((*kind, keys.get_raw(*kind).to_vec()), owner);
    }
    if proved {
        RegisteredKeys::insert(owner, &keys.beefy);
    } else {
        PendingRegistrations::mutate(|n| *n += 1);
    }
}

fn registration_fixture() -> (AccountId, AccountId, SessionKeys, Vec<u8>) {
    reset_registry();
    let owner = account(0);
    let controller: AccountId = frame_benchmarking::account("beefy-controller", 0, 0);
    // Exercise caller/controller -> validator/stash conversion, not a bypass hook.
    pallet_staking::Bonded::<Runtime>::insert(&owner, &controller);
    pallet_staking::Ledger::<Runtime>::insert(
        &controller,
        pallet_staking::StakingLedger::<Runtime>::new(owner.clone(), 1),
    );
    frame_system::Account::<Runtime>::mutate(&controller, |info| {
        info.providers = 1;
        info.consumers = 1;
    });
    let (keys, public) = keys(0);
    let proof = sp_io::crypto::ecdsa_sign_prehashed(
        sp_consensus_beefy::KEY_TYPE,
        &public,
        &registration_payload(&controller, &keys),
    )
    .expect("benchmark key exists")
    .0
    .to_vec();
    (owner, controller, keys, proof)
}

fn assert_registered(owner: &AccountId, keys: &SessionKeys) {
    assert_eq!(RegisteredKeys::get(owner), Some(keys.beefy.clone()));
    assert_eq!(
        pallet_session::NextKeys::<Runtime>::get(owner),
        Some(keys.clone())
    );
    assert_eq!(PendingRegistrations::get(), 0);
    for kind in SessionKeys::key_ids() {
        assert_eq!(
            Session::key_owner(*kind, keys.get_raw(*kind)),
            Some(owner.clone())
        );
    }
}

fn assert_purged(owner: &AccountId, keys: &SessionKeys) {
    assert!(!RegisteredKeys::contains_key(owner));
    assert!(!pallet_session::NextKeys::<Runtime>::contains_key(owner));
    assert_eq!(PendingRegistrations::get(), 0);
    for kind in SessionKeys::key_ids() {
        assert!(Session::key_owner(*kind, keys.get_raw(*kind)).is_none());
    }
}

fn exposure_fixture(entries: &[(AccountId, SessionKeys)], nominators: u32) {
    pallet_staking::ActiveEra::<Runtime>::put(pallet_staking::ActiveEraInfo {
        index: 0,
        start: None,
    });
    pallet_staking::CurrentEra::<Runtime>::put(0);
    for (owner, _) in entries {
        pallet_staking::EraInfo::<Runtime>::set_exposure(
            0,
            owner,
            pallet_staking::Exposure {
                total: nominators as u128 + 1,
                own: 1,
                others: (0..nominators)
                    .map(|i| pallet_staking::IndividualExposure {
                        who: frame_benchmarking::account("beefy-nominator", i, 0),
                        value: 1,
                    })
                    .collect(),
            },
        );
    }
}

fn history_fixture(n: u32, e: u32) -> Vec<(AccountId, SessionKeys)> {
    let entries = (0..n).map(|i| (account(i), keys(i).0)).collect::<Vec<_>>();
    exposure_fixture(&entries, e);
    pallet_session::CurrentIndex::<Runtime>::put(17);
    pallet_session::QueuedKeys::<Runtime>::put(&entries);
    entries
}

fn legacy_root(entries: &[(AccountId, SessionKeysOld)]) -> HistoricalRoot {
    let values = entries
        .iter()
        .enumerate()
        .flat_map(|(i, (owner, keys))| {
            let index = i as u32;
            SessionKeysOld::key_ids()
                .iter()
                .map(move |kind| ((*kind, keys.get_raw(*kind)).encode(), index.encode()))
                .chain(core::iter::once_with(move || {
                    (
                        index.encode(),
                        (owner, Staking::eras_stakers(0, owner)).encode(),
                    )
                }))
        })
        .collect();
    (
        sp_io::trie::blake2_256_root(values, StateVersion::V0),
        entries.len() as u32,
    )
}

benchmarks! {
    register_keys {
        let (owner, controller, keys, proof) = registration_fixture();
        let submitted = keys.clone();
    }: {
        Session::set_keys(RuntimeOrigin::signed(controller), submitted, proof)?;
    } verify {
        assert_registered(&owner, &keys);
    }

    rotate_keys {
        let (owner, controller, keys, proof) = registration_fixture();
        let submitted = keys.clone();
        let previous = with_beefy(old_keys(1), placeholder_beefy_key(&owner));
        seed_registration(&owner, &previous, false);
    }: {
        Session::set_keys(RuntimeOrigin::signed(controller), submitted, proof)?;
    } verify {
        assert_registered(&owner, &keys);
        for kind in SessionKeys::key_ids() {
            assert!(Session::key_owner(*kind, previous.get_raw(*kind)).is_none());
        }
    }

    rotate_proved_keys {
        let (owner, controller, keys, proof) = registration_fixture();
        let submitted = keys.clone();
        let previous = self::keys(1).0;
        seed_registration(&owner, &previous, true);
    }: {
        Session::set_keys(RuntimeOrigin::signed(controller), submitted, proof)?;
    } verify {
        assert_registered(&owner, &keys);
        for kind in SessionKeys::key_ids() {
            assert!(Session::key_owner(*kind, previous.get_raw(*kind)).is_none());
        }
    }

    reject_registration {
        let (owner, controller, keys, _) = registration_fixture();
        let proof = sp_io::crypto::ecdsa_sign_prehashed(sp_consensus_beefy::KEY_TYPE, &self::keys(1).1, &registration_payload(&controller, &keys)).expect("benchmark key exists").0.to_vec();
        let result;
    }: {
        result = Session::set_keys(RuntimeOrigin::signed(controller), keys, proof);
    } verify {
        assert_eq!(result, Err(pallet_session::Error::<Runtime>::InvalidProof.into()));
        assert!(!pallet_session::NextKeys::<Runtime>::contains_key(&owner));
        assert!(!RegisteredKeys::contains_key(&owner));
        assert_eq!(PendingRegistrations::get(), 0);
    }

    purge_keys {
        let (owner, controller, keys, _) = registration_fixture();
        seed_registration(&owner, &keys, true);
    }: {
        Session::purge_keys(RuntimeOrigin::signed(controller))?;
    } verify {
        assert_purged(&owner, &keys);
    }

    purge_legacy_keys {
        let (owner, controller, keys, _) = registration_fixture();
        seed_registration(&owner, &keys, false);
    }: {
        Session::purge_keys(RuntimeOrigin::signed(controller))?;
    } verify {
        assert_purged(&owner, &keys);
    }

    activate {
        let n in 1 .. 256;
        reset_registry();
        pallet_beefy::GenesisBlock::<Runtime>::kill();
        pallet_gear_eth_bridge::DestinationBinding::<Runtime>::kill();
        pallet_gear_eth_bridge::BridgeDomain::<Runtime>::kill();
        assert_ok!(GearEthBridge::bind_destination(RuntimeOrigin::root(), H256::repeat_byte(1), H160::repeat_byte(3)));
        let entries = (0..n).map(|i| (account(i), keys(i).0)).collect::<Vec<_>>();
        for (owner, keys) in &entries {
            seed_registration(owner, keys, true);
        }
        ActiveSessionKeys::put(&entries);
        pallet_session::QueuedKeys::<Runtime>::put(&entries);
        pallet_session::CurrentIndex::<Runtime>::put(0);
        pallet_staking::ValidatorCount::<Runtime>::put(n);
        pallet_beefy::Authorities::<Runtime>::kill();
        <Beefy as OneSessionHandler<AccountId>>::on_genesis_session(entries.iter().map(|(owner, keys)| (owner, keys.beefy.clone())));
    }: {
        Beefy::set_new_genesis(RuntimeOrigin::root(), 1)?;
    } verify {
        assert_eq!(pallet_beefy::GenesisBlock::<Runtime>::get(), Some(11));
        assert_eq!(pallet_beefy::Authorities::<Runtime>::get().len(), n as usize);
        assert_eq!(PendingRegistrations::get(), 0);
    }

    historical_root {
        let n in 1 .. 256;
        // More than one 256-nominator page; no claim this bounds full exposure.
        let e in 0 .. 1024;
        let entries = history_fixture(n, e);
        let expected = session_history::root(&entries, Some(0));
        let actual;
    }: {
        actual = session_history::root(&entries, Some(0));
    } verify {
        assert_eq!(actual, expected);
        assert_eq!(actual.1, n);
        assert_eq!(Staking::eras_stakers(0, &entries[0].0).others.len(), e as usize);
    }

    session_start {
        let n in 1 .. 256;
        let e in 0 .. 1024;
        let entries = history_fixture(n, e);
        let expected = session_history::root(&entries, Some(0));
        pallet_session_historical::StoredRange::<Runtime>::put((16, 18));
        // The two migration-boundary roots are pruned only with their sessions.
        LegacySessionRoots::insert(15, expected);
        LegacySessionRoots::insert(16, expected);
        pallet_session_historical::HistoricalSessions::<Runtime>::insert(16, expected);
    }: {
        <session_history::SessionManager as pallet_session::SessionManager<AccountId>>::start_session(17);
    } verify {
        assert_eq!(ActiveSessionKeys::get(), entries);
        assert_eq!(pallet_session_historical::HistoricalSessions::<Runtime>::get(17), Some(expected));
        assert_eq!(pallet_session_historical::StoredRange::<Runtime>::get(), Some((16, 18)));
        assert!(!LegacySessionRoots::contains_key(15));
        assert!(LegacySessionRoots::contains_key(16));
        let key = entries[0].1.beefy.clone();
        let proof = session_history::SessionKeyOwnerProof::prove((sp_consensus_beefy::KEY_TYPE, key.clone())).expect("committed owner");
        assert_eq!(session_history::SessionKeyOwnerProof::check_proof((sp_consensus_beefy::KEY_TYPE, key), proof), Some((entries[0].0.clone(), Staking::eras_stakers(0, &entries[0].0))));
    }

    migrate_session_keys {
        // All registered owners, not just the <=256 active bridge authorities.
        let n in 1 .. 10000;
        let e in 0 .. 1024;
        reset_registry();
        frame_system::LastRuntimeUpgrade::<Runtime>::put(frame_system::LastRuntimeUpgradeInfo {
            spec_version: 11000.into(), spec_name: crate::VERSION.spec_name.clone(),
        });
        pallet_beefy::GenesisBlock::<Runtime>::kill();
        let legacy = (0..n).map(|i| (account(i), old_keys(i))).collect::<Vec<_>>();
        for (owner, keys) in &legacy {
            frame_support::storage::unhashed::put(&pallet_session::NextKeys::<Runtime>::hashed_key_for(owner), keys);
            for kind in SessionKeysOld::key_ids() {
                pallet_session::KeyOwner::<Runtime>::insert((*kind, keys.get_raw(*kind).to_vec()), owner);
            }
        }
        let validators = n.min(256) as usize;
        let active = &legacy[..validators];
        let queued = &legacy[legacy.len() - validators..];
        frame_support::storage::unhashed::put(&pallet_session::QueuedKeys::<Runtime>::hashed_key(), &queued.to_vec());
        // Seed actual four-key active handlers, preserving potentially different queued keys.
        for (pallet, item) in [(b"Babe".as_slice(), b"Authorities".as_slice()), (b"Grandpa", b"Authorities"), (b"ImOnline", b"Keys"), (b"AuthorityDiscovery", b"Keys")] {
            sp_io::storage::clear(&frame_support::storage::storage_prefix(pallet, item));
        }
        <(Babe, Grandpa, ImOnline, AuthorityDiscovery) as pallet_session::SessionHandler<AccountId>>::on_genesis_session(active);
        pallet_session::Validators::<Runtime>::put(active.iter().map(|(owner, _)| owner.clone()).collect::<Vec<_>>());
        pallet_session::CurrentIndex::<Runtime>::put(17);
        let current = active.iter().map(|(owner, keys)| (owner.clone(), with_beefy(keys.clone(), placeholder_beefy_key(owner)))).collect::<Vec<_>>();
        let future = queued.iter().map(|(owner, keys)| (owner.clone(), with_beefy(keys.clone(), placeholder_beefy_key(owner)))).collect::<Vec<_>>();
        exposure_fixture(&current, e);
        exposure_fixture(&future, e);
        let old_current = legacy_root(active);
        let old_future = legacy_root(queued);
        pallet_session_historical::HistoricalSessions::<Runtime>::insert(17, old_current);
        pallet_session_historical::HistoricalSessions::<Runtime>::insert(18, old_future);
        pallet_session_historical::StoredRange::<Runtime>::put((17, 19));
        let expected_current = session_history::root(&current, Some(0));
        let expected_future = session_history::root(&future, Some(0));
        let consumed;
    }: {
        consumed = MigrateSessionKeys::on_runtime_upgrade();
    } verify {
        let storage_floor = <Runtime as frame_system::Config>::DbWeight::get().reads_writes(u64::from(n) * 6, u64::from(n) * 6);
        assert!(consumed.ref_time() >= storage_floor.ref_time(), "migration weight must scale with all registered owners");
        assert_eq!(PendingRegistrations::get(), n);
        assert_eq!(pallet_session::NextKeys::<Runtime>::iter().count(), n as usize);
        assert_eq!(pallet_session::KeyOwner::<Runtime>::iter().count(), n as usize * 5);
        assert_eq!(ActiveSessionKeys::get(), current);
        assert_eq!(pallet_session::QueuedKeys::<Runtime>::get(), future);
        assert_eq!(LegacySessionRoots::get(17), Some(old_current));
        assert_eq!(LegacySessionRoots::get(18), Some(old_future));
        assert_eq!(pallet_session_historical::HistoricalSessions::<Runtime>::get(17), Some(expected_current));
        assert_eq!(pallet_session_historical::HistoricalSessions::<Runtime>::get(18), Some(expected_future));
        assert_eq!(pallet_beefy::GenesisBlock::<Runtime>::get(), None);
        for (owner, old) in &legacy {
            let modern = pallet_session::NextKeys::<Runtime>::get(owner).unwrap();
            assert_eq!(modern.beefy, placeholder_beefy_key(owner));
            for kind in SessionKeysOld::key_ids() {
                assert_eq!(modern.get_raw(*kind), old.get_raw(*kind));
                assert_eq!(Session::key_owner(*kind, old.get_raw(*kind)), Some(owner.clone()));
            }
            assert_eq!(Session::key_owner(sp_consensus_beefy::KEY_TYPE, modern.get_raw(sp_consensus_beefy::KEY_TYPE)), Some(owner.clone()));
        }
    }

    impl_benchmark_test_suite!(Pallet, test_externalities(), Runtime);
}

#[cfg(test)]
fn test_externalities() -> sp_io::TestExternalities {
    let mut ext = sp_io::TestExternalities::default();
    ext.register_extension(sp_keystore::KeystoreExt::new(
        sp_keystore::testing::MemoryKeystore::new(),
    ));
    ext
}
