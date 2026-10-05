// Copyright (C) Gear Technologies Inc.
// SPDX-License-Identifier: GPL-3.0-or-later WITH Classpath-exception-2.0

//! BEEFY MMR leaf-extra provider committing the destination-bound bridge lane,
//! parent time, and queue state. It runs before the current timestamp inherent
//! and before the bridge's delayed queue clear.

use crate::{GearEthBridge, Timestamp};
use sp_runtime::traits::{Hash, Keccak256};

pub struct VaraBridgeProvider;

fn encode_snapshot(
    bridge_domain: sp_core::H256,
    source_timestamp_ms: u64,
    snapshot: Option<(u64, sp_core::H256)>,
) -> [u8; 86] {
    let (initialized, queue_id, root) = match snapshot {
        Some((queue_id, root)) => (1, queue_id, root),
        None => (0, 0, sp_core::H256::zero()),
    };
    let mut encoded = [0u8; 86];
    encoded[0] = 2;
    encoded[1..5].copy_from_slice(b"vara");
    encoded[5..37].copy_from_slice(bridge_domain.as_bytes());
    encoded[37..45].copy_from_slice(&source_timestamp_ms.to_le_bytes());
    encoded[45] = initialized;
    encoded[46..54].copy_from_slice(&queue_id.to_le_bytes());
    encoded[54..].copy_from_slice(root.as_bytes());
    encoded
}

impl sp_consensus_beefy::mmr::BeefyDataProvider<[u8; 32]> for VaraBridgeProvider {
    fn extra_data() -> [u8; 32] {
        Keccak256::hash(&encode_snapshot(
            GearEthBridge::bridge_domain(),
            Timestamp::get(),
            GearEthBridge::bridge_snapshot(),
        ))
        .0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use frame_support::traits::OnRuntimeUpgrade;
    use sp_consensus_beefy::mmr::BeefyDataProvider;
    use std::str::FromStr;

    #[test]
    fn initialized_commitment_matches_wire_fixture() {
        let [
            source_genesis,
            source_domain,
            destination_chain_id,
            destination_queue,
            bridge_domain,
            timestamp,
            initialized,
            queue_id,
            root,
            preimage,
            commitment,
            uninitialized_preimage,
            uninitialized_commitment,
        ]: [String; 13] =
            serde_json_wasm::from_str(include_str!("../tests/fixtures/bridge_commitment.json"))
                .expect("fixture is valid JSON");

        let source_genesis =
            sp_core::H256::from_str(&source_genesis).expect("raw source genesis is valid hex");
        let source_domain =
            sp_core::H256::from_str(&source_domain).expect("source domain is valid hex");
        let destination_chain_id = sp_core::H256::from_str(&destination_chain_id)
            .expect("destination chain id is 32-byte big-endian hex");
        let destination_queue =
            sp_core::H160::from_str(&destination_queue).expect("destination queue is valid hex");
        let bridge_domain =
            sp_core::H256::from_str(&bridge_domain).expect("bridge domain is valid hex");
        let source_timestamp_ms = timestamp.parse().expect("timestamp is valid decimal");
        let initialized: u8 = initialized.parse().expect("initialized is valid decimal");
        let queue_id = u64::from_str_radix(
            queue_id.strip_prefix("0x").expect("queue id has 0x prefix"),
            16,
        )
        .expect("queue id is valid hex");
        let root = sp_core::H256::from_str(&root).expect("root is valid hex");
        let preimage = sp_core::Bytes::from_str(&preimage).expect("preimage is valid hex");
        let commitment = sp_core::H256::from_str(&commitment).expect("commitment is valid hex");
        let uninitialized_preimage = sp_core::Bytes::from_str(&uninitialized_preimage)
            .expect("uninitialized preimage is valid hex");
        let uninitialized_commitment = sp_core::H256::from_str(&uninitialized_commitment)
            .expect("uninitialized commitment is valid hex");

        assert_ne!(source_genesis, source_domain);
        assert_ne!(source_genesis, bridge_domain);
        assert_eq!(initialized, 1);
        assert_eq!(
            Keccak256::hash(
                &[
                    &b"vara/gear-eth-bridge-domain/v2"[..],
                    source_domain.as_bytes(),
                    destination_chain_id.as_bytes(),
                    destination_queue.as_bytes(),
                ]
                .concat()
            ),
            bridge_domain,
        );

        let encoded = encode_snapshot(bridge_domain, source_timestamp_ms, Some((queue_id, root)));
        assert_eq!(encoded.len(), 86);
        assert_eq!(encoded.as_slice(), &preimage[..]);
        assert_eq!(Keccak256::hash(&encoded), commitment);

        let uninitialized = encode_snapshot(bridge_domain, source_timestamp_ms, None);
        assert_eq!(uninitialized.as_slice(), &uninitialized_preimage[..]);
        assert_eq!(Keccak256::hash(&uninitialized), uninitialized_commitment);
    }

    #[test]
    fn reset_preserves_bridge_domain_and_uninitialized_snapshot_is_distinct() {
        let bridge_domain = sp_core::H256::from_str(
            "0x9aac6d72e183672d20696112082accb15719870152120212f541e3e233837944",
        )
        .expect("bridge domain is valid hex");
        let timestamp = 1_800_000_000_000_u64;

        sp_io::TestExternalities::default().execute_with(|| {
            frame_system::BlockHash::<crate::Runtime>::insert(0, sp_core::H256::repeat_byte(0x99));
            pallet_timestamp::Now::<crate::Runtime>::put(timestamp);
            pallet_gear_eth_bridge::BridgeDomain::<crate::Runtime>::put(bridge_domain);
            pallet_gear_eth_bridge::migrations::reset::ResetMigration::<crate::Runtime>::
                on_runtime_upgrade();

            assert!(GearEthBridge::bridge_snapshot().is_none());
            assert_eq!(GearEthBridge::bridge_domain(), bridge_domain);
            assert_eq!(
                VaraBridgeProvider::extra_data(),
                Keccak256::hash(&encode_snapshot(bridge_domain, timestamp, None)).0,
            );
            assert_ne!(
                encode_snapshot(bridge_domain, timestamp, None),
                encode_snapshot(bridge_domain, timestamp, Some((0, sp_core::H256::zero()))),
            );
        });
    }
}
