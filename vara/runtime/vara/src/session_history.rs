// Copyright (C) Gear Technologies Inc.
// SPDX-License-Identifier: GPL-3.0-or-later WITH Classpath-exception-2.0

//! Ownership commitments use activated keys, never mutable registrations for future sessions.

use crate::{
    AccountId, Historical, Runtime, Session, SessionKeys, Staking, pallet_session_historical,
};
use frame_support::{Twox64Concat, pallet_prelude::ValueQuery, traits::KeyOwnerProofSystem};
use parity_scale_codec::{DecodeAll, Encode};
use sp_core::crypto::KeyTypeId;
use sp_runtime::{
    StateVersion,
    traits::{BlakeTwo256, Hash, OpaqueKeys},
};
use sp_std::prelude::*;
use sp_trie::{
    LayoutV0, MemoryDB, Recorder, StorageProof, Trie, TrieDBBuilder, TrieMut,
    accessed_nodes_tracker::AccessedNodesTracker, recorder_ext::RecorderExt,
    trie_types::TrieDBMutBuilderV0,
};

pub(crate) type HistoricalRoot = (sp_core::H256, u32);

#[frame_support::storage_alias]
pub(crate) type ActiveSessionKeys =
    StorageValue<Historical, Vec<(AccountId, SessionKeys)>, ValueQuery>;

#[frame_support::storage_alias]
pub(crate) type LegacySessionRoots = StorageMap<Historical, Twox64Concat, u32, HistoricalRoot>;

fn entries(
    keys: &[(AccountId, SessionKeys)],
    era: Option<u32>,
) -> impl Iterator<Item = (Vec<u8>, Vec<u8>)> + '_ {
    era.into_iter().flat_map(move |era| {
        keys.iter()
            .enumerate()
            .flat_map(move |(index, (account, keys))| {
                let index = index as u32;
                SessionKeys::key_ids()
                    .iter()
                    .map(move |kind| ((*kind, keys.get_raw(*kind)).encode(), index.encode()))
                    .chain(core::iter::once_with(move || {
                        (
                            index.encode(),
                            (account, Staking::eras_stakers(era, account)).encode(),
                        )
                    }))
            })
    })
}

pub(crate) fn root(keys: &[(AccountId, SessionKeys)], era: Option<u32>) -> HistoricalRoot {
    (
        sp_io::trie::blake2_256_root(entries(keys, era).collect(), StateVersion::V0),
        if era.is_some() { keys.len() as u32 } else { 0 },
    )
}

/// Delegates validator selection unchanged; commits the exact keys when they become active.
pub struct SessionManager;

impl pallet_session::SessionManager<AccountId> for SessionManager {
    fn new_session(index: u32) -> Option<Vec<AccountId>> {
        <Staking as pallet_session::SessionManager<AccountId>>::new_session(index)
    }

    fn new_session_genesis(index: u32) -> Option<Vec<AccountId>> {
        <Staking as pallet_session::SessionManager<AccountId>>::new_session_genesis(index)
    }

    fn start_session(index: u32) {
        <Staking as pallet_session::SessionManager<AccountId>>::start_session(index);
        // Session has activated this queue, but has not overwritten it with future registrations.
        let keys = pallet_session::QueuedKeys::<Runtime>::get();
        let root = root(&keys, Staking::active_era().map(|era| era.index));
        ActiveSessionKeys::put(keys);
        pallet_session_historical::HistoricalSessions::<Runtime>::insert(index, root);
        pallet_session_historical::StoredRange::<Runtime>::mutate(|range| {
            let end = index.checked_add(1).expect("session index cannot overflow");
            match range {
                Some((_, previous_end)) => *previous_end = (*previous_end).max(end),
                None => *range = Some((index, end)),
            }
        });
        for legacy in LegacySessionRoots::iter_keys() {
            if !pallet_session_historical::HistoricalSessions::<Runtime>::contains_key(legacy) {
                LegacySessionRoots::remove(legacy);
            }
        }
    }

    fn end_session(index: u32) {
        pallet_session_historical::onchain::store_session_validator_set_to_offchain::<Runtime>(
            index,
        );
        <Staking as pallet_session::SessionManager<AccountId>>::end_session(index);
    }
}

/// Both new and migration-boundary proofs are checked against immutable session commitments.
pub struct SessionKeyOwnerProof;

impl<D: AsRef<[u8]>> KeyOwnerProofSystem<(KeyTypeId, D)> for SessionKeyOwnerProof {
    type Proof = sp_session::MembershipProof;
    type IdentificationTuple = pallet_session_historical::IdentificationTuple<Runtime>;

    fn prove(key: (KeyTypeId, D)) -> Option<Self::Proof> {
        let session = Session::current_index();
        let (expected_root, validator_count) = Historical::historical_root(session)?;
        let keys = ActiveSessionKeys::get();
        let entries = entries(&keys, Staking::active_era().map(|era| era.index));
        let mut db = MemoryDB::<BlakeTwo256>::default();
        let mut root = Default::default();
        {
            let mut trie = TrieDBMutBuilderV0::<BlakeTwo256>::new(&mut db, &mut root).build();
            for (key, value) in entries {
                trie.insert(&key, &value).ok()?;
            }
        }
        if root != expected_root {
            return None;
        }
        let mut recorder = Recorder::<LayoutV0<BlakeTwo256>>::new();
        {
            let trie = TrieDBBuilder::<LayoutV0<BlakeTwo256>>::new(&db, &root)
                .with_recorder(&mut recorder)
                .build();
            let index = trie.get(&(key.0, key.1.as_ref()).encode()).ok()??;
            trie.get(&index).ok()??;
        }
        Some(sp_session::MembershipProof {
            session,
            validator_count,
            trie_nodes: recorder.into_raw_storage_proof(),
        })
    }

    fn check_proof(key: (KeyTypeId, D), proof: Self::Proof) -> Option<Self::IdentificationTuple> {
        if proof.session > Session::current_index() {
            return None;
        }
        let mut commitment = Historical::historical_root(proof.session)?;
        if let Some(legacy) = LegacySessionRoots::get(proof.session)
            && proof
                .trie_nodes
                .iter()
                .any(|node| BlakeTwo256::hash(node) == legacy.0)
        {
            commitment = legacy;
        }
        let (root, count) = commitment;
        if count != proof.validator_count {
            return None;
        }
        let proof = StorageProof::new_with_duplicate_nodes_check(proof.trie_nodes).ok()?;
        let mut tracker = AccessedNodesTracker::new(proof.len());
        let db = proof.into_memory_db::<BlakeTwo256>();
        let trie = TrieDBBuilder::<LayoutV0<BlakeTwo256>>::new(&db, &root)
            .with_recorder(&mut tracker)
            .build();
        let index = trie.get(&(key.0, key.1.as_ref()).encode()).ok()??;
        let index = u32::decode_all(&mut &index[..]).ok()?;
        if index >= count {
            return None;
        }
        let owner = trie.get(&index.encode()).ok()??;
        let owner = Self::IdentificationTuple::decode_all(&mut &owner[..]).ok()?;
        tracker.ensure_no_unused_nodes().ok()?;
        Some(owner)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        BuildStorage, RuntimeOrigin, System,
        genesis_config_presets::{authority_keys_from_seed, local_testnet_genesis},
    };
    use sp_core::Pair;

    fn proofs(keys: &SessionKeys) -> Vec<(KeyTypeId, Vec<u8>, sp_session::MembershipProof)> {
        SessionKeys::key_ids()
            .iter()
            .map(|kind| {
                let raw = keys.get_raw(*kind).to_vec();
                let proof = SessionKeyOwnerProof::prove((*kind, &raw)).unwrap();
                (*kind, raw, proof)
            })
            .collect()
    }

    fn assert_owner(
        proofs: &[(KeyTypeId, Vec<u8>, sp_session::MembershipProof)],
        owner: &AccountId,
    ) {
        for (kind, raw, proof) in proofs {
            assert_eq!(
                SessionKeyOwnerProof::check_proof((*kind, raw), proof.clone())
                    .unwrap()
                    .0,
                *owner
            );
        }
    }

    #[test]
    fn registered_keys_never_replace_active_ownership_before_queue_activation() {
        for beefy_only in [true, false] {
            sp_io::TestExternalities::new(local_testnet_genesis().build_storage().unwrap())
                .execute_with(|| {
                    System::set_block_number(1);
                    frame_system::BlockHash::<Runtime>::insert(0, sp_core::H256::repeat_byte(7));
                    let (owner, ..) = authority_keys_from_seed("Alice");
                    let controller = pallet_staking::Bonded::<Runtime>::get(&owner).unwrap();
                    let old = pallet_session::NextKeys::<Runtime>::get(&owner).unwrap();
                    let original = proofs(&old);
                    let old_root = Historical::historical_root(Session::current_index()).unwrap();
                    let mut replacement = old.clone();
                    let pair = sp_core::ecdsa::Pair::from_seed(&[91; 32]);
                    replacement.beefy = pair.public().into();
                    if !beefy_only {
                        let (_, _, babe, grandpa, online, discovery, _) =
                            authority_keys_from_seed("Charlie");
                        replacement.babe = babe;
                        replacement.grandpa = grandpa;
                        replacement.im_online = online;
                        replacement.authority_discovery = discovery;
                    }
                    let signature = pair.sign_prehashed(
                        &crate::beefy_activation::registration_payload(&controller, &replacement),
                    );
                    Session::set_keys(
                        RuntimeOrigin::signed(controller.clone()),
                        replacement.clone(),
                        signature.0.to_vec(),
                    )
                    .unwrap();
                    assert_eq!(
                        Historical::historical_root(Session::current_index()),
                        Some(old_root)
                    );
                    assert_eq!(proofs(&old), original);
                    assert!(
                        SessionKeyOwnerProof::prove((
                            sp_consensus_beefy::KEY_TYPE,
                            &replacement.beefy
                        ))
                        .is_none()
                    );
                    assert_owner(&original, &owner);
                    Session::rotate_session();
                    assert_eq!(proofs(&old)[0].2.session, 1);
                    assert_owner(&original, &owner);
                    assert!(
                        SessionKeyOwnerProof::prove((
                            sp_consensus_beefy::KEY_TYPE,
                            &replacement.beefy
                        ))
                        .is_none()
                    );
                    System::set_block_number(2);
                    Session::rotate_session();
                    let activated = proofs(&replacement);
                    assert_owner(&activated, &owner);
                    assert_owner(&original, &owner);
                    assert!(
                        SessionKeyOwnerProof::prove((sp_consensus_beefy::KEY_TYPE, &old.beefy))
                            .is_none()
                    );
                    assert_eq!(Staking::active_era().unwrap().index, 0);
                    // Removing a future registration must not erase the active or already queued owner.
                    Session::purge_keys(RuntimeOrigin::signed(controller)).unwrap();
                    assert!(pallet_session::NextKeys::<Runtime>::get(&owner).is_none());
                    assert_owner(&proofs(&replacement), &owner);
                    System::set_block_number(3);
                    Session::rotate_session();
                    assert_owner(&proofs(&replacement), &owner);
                    System::set_block_number(4);
                    Session::rotate_session();
                    assert!(
                        SessionKeyOwnerProof::prove((
                            sp_consensus_beefy::KEY_TYPE,
                            &replacement.beefy
                        ))
                        .is_none()
                    );
                    assert_owner(&activated, &owner);
                    assert_owner(&original, &owner);
                    let mut future = activated[0].2.clone();
                    future.session = Session::current_index() + 1;
                    assert!(
                        SessionKeyOwnerProof::check_proof(
                            (activated[0].0, &activated[0].1),
                            future
                        )
                        .is_none()
                    );
                    Historical::prune_up_to(3);
                    for (kind, raw, proof) in original.iter().chain(&activated) {
                        assert!(
                            SessionKeyOwnerProof::check_proof((*kind, raw), proof.clone())
                                .is_none()
                        );
                    }
                });
        }
    }
}
